//! Lossless WebP images and full-canvas animations.
//!
//! `image-webp` owns pixel compression. The small RIFF wrapper below adds animation timing without
//! native dependencies or a GIF intermediate. Full-canvas replacement frames preserve transparency
//! without compositing trails; frame-difference compression is deliberately left to future work.
//! Container layout: <https://developers.google.com/speed/webp/docs/riff_container>.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

use image_webp::{ColorType, WebPEncoder};
use miette::{miette, IntoDiagnostic};

use super::{
    ensure_parent_dir, Frame, MediaProgress, MediaProgressKind, MediaProgressReporter,
    NoMediaProgress,
};
use crate::Result;

const MAX_FRAME_DURATION_MS: u128 = 0xff_ffff;

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

/// Write an infinitely looping lossless WebP animation from captured frames and their hold times.
///
/// RGBA pixels, including transparency, are preserved. Cumulative timestamps are rounded to the
/// nearest millisecond to avoid per-frame rounding drift, with a minimum hold of 1 ms per frame.
/// Viewers may impose longer minimum delays, especially for holds of 10 ms or less. Single-frame
/// animations retain their hold time. No external encoder or frame-rate resampling is used.
/// Parent directories are created and existing files are overwritten.
///
/// # Errors
///
/// Returns an error for empty input, invalid buffers, inconsistent or unsupported dimensions,
/// holds exceeding 16,777,215 ms, files exceeding the WebP 4 GiB limit, or encoding/I/O failures.
/// An error during encoding can leave a partial file at the output path.
pub fn write_webp_animation(path: &Path, frames: &[(Frame, Duration)]) -> Result<()> {
    write_webp_animation_with_progress(path, frames, &mut NoMediaProgress)
}

/// Write a lossless WebP animation, reporting progress after each encoded frame.
///
/// See [`write_webp_animation`] for timing, output behavior, and errors.
pub fn write_webp_animation_with_progress(
    path: &Path,
    frames: &[(Frame, Duration)],
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

    ensure_parent_dir(path)?;
    let file = File::create(path).into_diagnostic()?;
    let mut writer = BufWriter::new(file);
    writer.write_all(b"RIFF\0\0\0\0WEBP").into_diagnostic()?;
    // VP8X: alpha + animation flags, reserved bytes, then 1-based canvas dimensions.
    let mut header = vec![0x12, 0, 0, 0];
    header.extend_from_slice(&u24(first.width - 1));
    header.extend_from_slice(&u24(first.height - 1));
    write_chunk(&mut writer, b"VP8X", &header)?;
    // Transparent background, infinite loop count.
    write_chunk(&mut writer, b"ANIM", &[0; 6])?;
    for (index, ((frame, _), delay)) in frames.iter().zip(delays).enumerate() {
        let encoded = encode_frame(frame)?;
        // The metadata-free image-webp encoder emits a simple RIFF/WEBP with one VP8L chunk.
        // Check that contract before embedding its padded chunk in an animation frame.
        if encoded.get(12..16) != Some(b"VP8L") {
            return Err(miette!("WebP encoder did not produce a lossless VP8L chunk").into());
        }
        let mut payload = vec![0; 6]; // Full canvas at x=0, y=0.
        payload.extend_from_slice(&u24(frame.width - 1));
        payload.extend_from_slice(&u24(frame.height - 1));
        payload.extend_from_slice(&u24(delay));
        payload.push(2); // Replace pixels (no alpha blending), no disposal.
        payload.extend_from_slice(&encoded[12..]);
        write_chunk(&mut writer, b"ANMF", &payload)?;
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

fn encode_frame(frame: &Frame) -> Result<Vec<u8>> {
    validate_dimensions(frame)?;
    // Frame::rgba assumes a usable row stride; validate it before accessing the buffer.
    if frame.stride < frame.width as usize * 4 {
        return Err(miette!("WebP frame stride is smaller than width * 4").into());
    }
    let rgba = frame.rgba()?;
    let mut encoded = Vec::new();
    WebPEncoder::new(&mut encoded)
        .encode(&rgba, frame.width, frame.height, ColorType::Rgba8)
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
