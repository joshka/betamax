//! WebP images and animations.
//!
//! Lossless output uses `image-webp`, a pure-Rust encoder, with a small RIFF wrapper for animation
//! timing. Each animation frame after the first stores only the rectangle that changed, replacing
//! those pixels without blending, so pixels and transparency stay exact. Lossy animations use
//! libwebp's animation encoder, statically linked through `webp-animation`.
//! Container layout: <https://developers.google.com/speed/webp/docs/riff_container>.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

use image_webp::{ColorType, WebPEncoder};
use miette::{miette, IntoDiagnostic};
use webp_animation::{Encoder, EncoderOptions, EncodingConfig, EncodingType, LossyEncodingConfig};

use super::{
    ensure_parent_dir, Frame, MediaProgress, MediaProgressKind, MediaProgressReporter,
    NoMediaProgress,
};
use crate::Result;

const MAX_FRAME_DURATION_MS: u128 = 0xff_ffff;
/// libwebp quality/speed trade-off for lossy frames, from 0 (fast) to 6 (small).
const ENCODER_METHOD: usize = 4;

/// Write a static lossless WebP image, preserving RGBA pixels and creating parent directories.
///
/// No external encoder is required. An existing output file is overwritten.
///
/// # Errors
///
/// Returns an error for invalid frame buffers, dimensions outside 1–16384 pixels on either axis,
/// or encoding and file I/O failures.
pub fn write_webp(path: &Path, frame: &Frame) -> Result<()> {
    let encoded = encode_frame(frame)?;
    ensure_parent_dir(path)?;
    std::fs::write(path, encoded)
        .into_diagnostic()
        .map_err(|error| error.wrap_err(format!("failed to write {}", path.display())))?;
    Ok(())
}

/// How animated WebP frames are compressed.
///
/// Terminal recordings are mostly flat text, which lossless WebP stores well, but inline images,
/// gradients, and plots make lossless frames large. Lossy mode trades exact pixels for size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WebpCompression {
    /// Exact pixels. Best for text-only recordings.
    #[default]
    Lossless,
    /// Lossy VP8 frames. Much smaller when frames contain images.
    ///
    /// libwebp treats small changes between frames as unchanged, so below a quality of about 85,
    /// dim text that is erased or redrawn can leave faint ghosts until a larger change repaints
    /// that area.
    Lossy {
        /// Quality from 0 (smallest) to 100 (best).
        quality: u8,
    },
}

/// Write an infinitely looping lossless WebP animation from captured frames and their hold times.
///
/// See [`write_webp_animation_with_options`] for timing, output behavior, and errors.
pub fn write_webp_animation(path: &Path, frames: &[(Frame, Duration)]) -> Result<()> {
    write_webp_animation_with_options(
        path,
        frames,
        WebpCompression::Lossless,
        &mut NoMediaProgress,
    )
}

/// Write a lossless WebP animation, reporting progress after each encoded frame.
///
/// See [`write_webp_animation_with_options`] for timing, output behavior, and errors.
pub fn write_webp_animation_with_progress(
    path: &Path,
    frames: &[(Frame, Duration)],
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    write_webp_animation_with_options(path, frames, WebpCompression::Lossless, progress)
}

/// Write an infinitely looping WebP animation, reporting progress after each encoded frame.
///
/// Cumulative timestamps are rounded to the nearest millisecond to avoid per-frame rounding
/// drift, with a minimum hold of 1 ms per frame. Viewers may impose longer minimum delays,
/// especially for holds of 10 ms or less. No external encoder or frame-rate resampling is used.
/// Parent directories are created and existing files are overwritten.
///
/// Single-frame animations retain their hold time. Lossless animations preserve RGBA pixels,
/// including transparency. After the first frame, each frame stores only the rectangle that
/// changed, so a keystroke costs a small sub-frame rather than a full canvas.
///
/// Lossy animations go through libwebp, which also stores changed rectangles and merges identical
/// frames. A single frame is written losslessly instead, because libwebp would turn it into a
/// still image without its hold.
///
/// # Errors
///
/// Returns an error for empty input, invalid buffers, inconsistent or unsupported dimensions,
/// holds exceeding 16,777,215 ms, quality above 100, files exceeding the WebP 4 GiB limit, or
/// encoding/I/O failures. An error during encoding can leave a partial file at the output path.
pub fn write_webp_animation_with_options(
    path: &Path,
    frames: &[(Frame, Duration)],
    compression: WebpCompression,
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    let Some((first, _)) = frames.first() else {
        return Err(miette!("cannot write WebP with no frames").into());
    };
    validate_dimensions(first)?;
    let mut elapsed = Duration::ZERO;
    let mut previous_ms = 0;
    let mut delays = Vec::with_capacity(frames.len());
    for (frame, delay) in frames {
        if (frame.width, frame.height) != (first.width, first.height) {
            return Err(miette!("all WebP frames must have the same dimensions").into());
        }
        elapsed = elapsed
            .checked_add(*delay)
            .ok_or_else(|| miette!("WebP duration overflow"))?;
        let end_ms = ((elapsed.as_nanos() + 500_000) / 1_000_000).max(previous_ms + 1);
        let delay_ms = end_ms - previous_ms;
        if delay_ms > MAX_FRAME_DURATION_MS {
            return Err(miette!("WebP frame hold exceeds 16,777,215 ms").into());
        }
        delays.push(delay_ms as u32);
        previous_ms = end_ms;
    }
    match compression {
        WebpCompression::Lossless => write_lossless_animation(path, frames, &delays, progress),
        WebpCompression::Lossy { .. } if frames.len() == 1 => {
            write_lossless_animation(path, frames, &delays, progress)
        }
        WebpCompression::Lossy { quality } => {
            write_lossy_animation(path, frames, &delays, quality, progress)
        }
    }
}

fn write_lossless_animation(
    path: &Path,
    frames: &[(Frame, Duration)],
    delays: &[u32],
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    let (width, height) = (frames[0].0.width, frames[0].0.height);
    ensure_parent_dir(path)?;
    let file = File::create(path).into_diagnostic()?;
    let mut writer = BufWriter::new(file);
    writer.write_all(b"RIFF\0\0\0\0WEBP").into_diagnostic()?;
    // VP8X: alpha + animation flags, reserved bytes, then 1-based canvas dimensions.
    let mut header = vec![0x12, 0, 0, 0];
    header.extend_from_slice(&u24(width - 1));
    header.extend_from_slice(&u24(height - 1));
    write_chunk(&mut writer, b"VP8X", &header)?;
    // Transparent background, infinite loop count.
    write_chunk(&mut writer, b"ANIM", &[0; 6])?;
    let mut previous: Option<Vec<u8>> = None;
    for (index, ((frame, _), &delay)) in frames.iter().zip(delays).enumerate() {
        // Frame::rgba assumes a usable row stride; validate it before accessing the buffer.
        if frame.stride < frame.width as usize * 4 {
            return Err(miette!("WebP frame stride is smaller than width * 4").into());
        }
        let rgba = frame.rgba()?;
        let rect = previous
            .as_deref()
            .map_or(Rect::full(width, height), |previous| {
                Rect::changed(width, height, previous, &rgba)
            });
        let encoded = encode_rgba(&rect.crop(width, &rgba), rect.width, rect.height)?;
        // The metadata-free image-webp encoder emits a simple RIFF/WEBP with one VP8L chunk.
        // Check that contract before embedding its padded chunk in an animation frame.
        if encoded.get(12..16) != Some(b"VP8L") {
            return Err(miette!("WebP encoder did not produce a lossless VP8L chunk").into());
        }
        // Offsets are stored halved, so rectangles start on even coordinates.
        let mut payload = Vec::with_capacity(16 + encoded.len());
        payload.extend_from_slice(&u24(rect.left / 2));
        payload.extend_from_slice(&u24(rect.top / 2));
        payload.extend_from_slice(&u24(rect.width - 1));
        payload.extend_from_slice(&u24(rect.height - 1));
        payload.extend_from_slice(&u24(delay));
        // Replace pixels (no alpha blending), no disposal: outside the rectangle the canvas keeps
        // the previous frame, which is exactly this frame's pixels there.
        payload.push(2);
        payload.extend_from_slice(&encoded[12..]);
        write_chunk(&mut writer, b"ANMF", &payload)?;
        previous = Some(rgba);
        progress.report(MediaProgress {
            kind: MediaProgressKind::Webp,
            position: index + 1,
            total: frames.len(),
        });
    }
    let length = writer.stream_position().into_diagnostic()?;
    if length > u64::from(u32::MAX) - 1 {
        return Err(miette!("WebP output exceeds the 4 GiB container limit").into());
    }
    writer.seek(SeekFrom::Start(4)).into_diagnostic()?;
    writer
        .write_all(&((length - 8) as u32).to_le_bytes())
        .into_diagnostic()?;
    writer.flush().into_diagnostic()?;
    Ok(())
}

fn write_lossy_animation(
    path: &Path,
    frames: &[(Frame, Duration)],
    delays: &[u32],
    quality: u8,
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    let (width, height) = (frames[0].0.width, frames[0].0.height);
    let options = EncoderOptions {
        encoding_config: Some(lossy_config(quality)?),
        ..EncoderOptions::default()
    };
    let mut encoder = Encoder::new_with_options((width, height), options)
        .map_err(|error| miette!("failed to start WebP encoder: {error}"))?;
    let mut timestamp_ms: i32 = 0;
    for (index, ((frame, _), &delay)) in frames.iter().zip(delays).enumerate() {
        if frame.stride < frame.width as usize * 4 {
            return Err(miette!("WebP frame stride is smaller than width * 4").into());
        }
        encoder
            .add_frame(&frame.rgba()?, timestamp_ms)
            .map_err(|error| miette!("failed to encode WebP frame {index}: {error}"))?;
        timestamp_ms = i32::try_from(delay)
            .ok()
            .and_then(|delay| timestamp_ms.checked_add(delay))
            .ok_or_else(|| miette!("WebP animation is too long"))?;
        progress.report(MediaProgress {
            kind: MediaProgressKind::Webp,
            position: index + 1,
            total: frames.len(),
        });
    }
    let encoded = encoder
        .finalize(timestamp_ms)
        .map_err(|error| miette!("failed to finish WebP animation: {error}"))?;
    ensure_parent_dir(path)?;
    std::fs::write(path, &*encoded)
        .into_diagnostic()
        .map_err(|error| error.wrap_err(format!("failed to write {}", path.display())))?;
    Ok(())
}

/// Lossy settings tuned for terminal text: sharp chroma edges and segment-adaptive quantization.
fn lossy_config(quality: u8) -> Result<EncodingConfig> {
    if quality > 100 {
        return Err(miette!("WebP quality must be between 0 and 100").into());
    }
    Ok(EncodingConfig {
        encoding_type: EncodingType::Lossy(LossyEncodingConfig {
            segments: 4,
            use_sharp_yuv: true,
            ..LossyEncodingConfig::default()
        }),
        quality: f32::from(quality),
        method: ENCODER_METHOD,
    })
}

/// Pixel rectangle of an animation frame, with even offsets as the WebP container requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

impl Rect {
    fn full(width: u32, height: u32) -> Self {
        Self {
            left: 0,
            top: 0,
            width,
            height,
        }
    }

    /// Bounding box of pixels that differ, widened to even offsets.
    ///
    /// Identical frames still need a rectangle to carry their hold, so they get the 1×1 pixel at
    /// the origin, which replaces a pixel with itself.
    fn changed(width: u32, height: u32, previous: &[u8], current: &[u8]) -> Self {
        let row_bytes = width as usize * 4;
        let rows = previous
            .chunks_exact(row_bytes)
            .zip(current.chunks_exact(row_bytes));
        let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
        for (y, (before, after)) in rows.enumerate() {
            if before == after {
                continue;
            }
            let y = y as u32;
            let differs = |(a, b): (&[u8; 4], &[u8; 4])| a != b;
            let pixels = || {
                before
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(after.as_chunks::<4>().0)
            };
            let first = pixels().position(differs);
            let last = pixels().rposition(differs);
            if let (Some(first), Some(last)) = (first, last) {
                left = left.min(first as u32);
                right = right.max(last as u32 + 1);
                top = top.min(y);
                bottom = y + 1;
            }
        }
        if right == 0 {
            return Self {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            };
        }
        let (left, top) = (left & !1, top & !1);
        Self {
            left,
            top,
            width: right - left,
            height: bottom - top,
        }
    }

    /// Copy this rectangle's RGBA pixels out of a full frame `frame_width` pixels wide.
    fn crop(&self, frame_width: u32, rgba: &[u8]) -> Vec<u8> {
        let row_bytes = frame_width as usize * 4;
        let start = self.left as usize * 4;
        let end = start + self.width as usize * 4;
        rgba.chunks_exact(row_bytes)
            .skip(self.top as usize)
            .take(self.height as usize)
            .flat_map(|row| &row[start..end])
            .copied()
            .collect()
    }
}

fn encode_frame(frame: &Frame) -> Result<Vec<u8>> {
    validate_dimensions(frame)?;
    // Frame::rgba assumes a usable row stride; validate it before accessing the buffer.
    if frame.stride < frame.width as usize * 4 {
        return Err(miette!("WebP frame stride is smaller than width * 4").into());
    }
    encode_rgba(&frame.rgba()?, frame.width, frame.height)
}

fn encode_rgba(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    WebPEncoder::new(&mut encoded)
        .encode(rgba, width, height, ColorType::Rgba8)
        .into_diagnostic()?;
    Ok(encoded)
}

fn validate_dimensions(frame: &Frame) -> Result<()> {
    if !(1..=16384).contains(&frame.width) || !(1..=16384).contains(&frame.height) {
        return Err(miette!("WebP dimensions must be between 1 and 16384 pixels").into());
    }
    Ok(())
}

fn u24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

fn write_chunk(writer: &mut impl Write, kind: &[u8; 4], payload: &[u8]) -> Result<()> {
    let size = u32::try_from(payload.len()).into_diagnostic()?;
    writer.write_all(kind).into_diagnostic()?;
    writer.write_all(&size.to_le_bytes()).into_diagnostic()?;
    writer.write_all(payload).into_diagnostic()?;
    if size % 2 != 0 {
        writer.write_all(&[0]).into_diagnostic()?;
    }
    Ok(())
}
