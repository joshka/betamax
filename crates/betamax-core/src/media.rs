//! Media file writers and frame conversion.
//!
//! The renderer produces raw frames. This module owns the format-specific side effects for PNG,
//! GIF, WebP, JSON, and video outputs. Video is intentionally implemented through `ffmpeg` for now
//! so the Rust code only has to provide a deterministic PNG frame sequence and clear error
//! reporting.
//!
//! Most users should let [`crate::Runner`] write media outputs from a tape. Use this module when
//! embedding Betamax's renderer directly or when tests need to inspect/write raw frames.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gif::{DisposalMethod, Encoder as GifEncoder, Frame as GifFrame, Repeat};
use miette::{miette, IntoDiagnostic};
use serde::Serialize;

use crate::Result;

mod webp;

pub use webp::{write_webp, write_webp_animation, write_webp_animation_with_progress};

/// Entries in a GIF color table.
const GIF_PALETTE_SIZE: usize = 256;
/// Palette index reserved for transparent pixels when any frame has them.
const GIF_TRANSPARENT_INDEX: u8 = 255;
/// Most frequent colors given exact palette entries when an animation needs quantizing.
const GIF_EXACT_COLORS: usize = 128;
/// Approximate cap on NeuQuant training pixels, so long animations train in bounded memory.
const GIF_MAX_TRAINING_PIXELS: usize = 1 << 20;
/// Minimum NeuQuant training pixels, so its learning loop always has samples to step through.
const GIF_MIN_TRAINING_PIXELS: usize = 4096;
/// Number of bytes in one RGBA or BGRA pixel.
const BYTES_PER_PIXEL: usize = 4;
/// Width used for numbered PNG sequence frame names.
const FRAME_INDEX_WIDTH: usize = 5;
/// NeuQuant sampling factor used when a GIF has more colors than its palette holds.
///
/// The value favors reasonable encode speed for CLI-generated demos. A future benchmark pass can
/// tune this if GIF quality or encode time becomes a release concern.
const GIF_QUANTIZATION_SPEED: i32 = 10;
/// Milliseconds represented by one GIF delay unit.
const GIF_DELAY_UNIT_MS: u128 = 10;
/// Minimum non-zero delay GIF viewers reliably honor.
const MIN_GIF_DELAY_UNITS: u128 = 1;
/// Minimum frame rate passed to ffmpeg.
const MIN_VIDEO_FRAMERATE: f64 = 1.0;
/// Frame-rate precision used in generated ffmpeg arguments.
const VIDEO_FRAMERATE_PRECISION: usize = 3;
/// VP9 constant-rate-factor used for WebM output.
///
/// Lower values are larger/higher quality. `30` is ffmpeg's common quality-oriented VP9 example
/// value and keeps WebM output useful without making example assets unnecessarily large.
const WEBM_CRF: &str = "30";
/// Disambiguate concurrent encodes when the system clock returns the same timestamp.
static VIDEO_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Media writer stage that can report deterministic frame progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaProgressKind {
    /// Animated GIF frame encoding.
    Gif,
    /// Lossless WebP animation frame encoding.
    Webp,
    /// Numbered PNG frame sequence writing.
    PngSequence,
    /// Temporary PNG frame writing before handing video encoding to `ffmpeg`.
    VideoFrames,
}

/// Determinate progress for media writers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaProgress {
    /// Writer stage being reported.
    pub kind: MediaProgressKind,
    /// Completed units.
    pub position: usize,
    /// Total units expected for this stage.
    pub total: usize,
}

/// Receives media progress updates.
pub trait MediaProgressReporter: Send {
    /// Report the latest completed frame count for a media-writing stage.
    fn report(&mut self, progress: MediaProgress);
}

impl<T> MediaProgressReporter for Box<T>
where
    T: MediaProgressReporter + ?Sized,
{
    fn report(&mut self, progress: MediaProgress) {
        self.as_mut().report(progress);
    }
}

#[derive(Debug, Default)]
pub(crate) struct NoMediaProgress;

impl MediaProgressReporter for NoMediaProgress {
    fn report(&mut self, _progress: MediaProgress) {}
}

/// Pixel byte order for a [`Frame`].
///
/// Betamax normalizes both supported formats to tightly packed RGBA when writing image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// Red, green, blue, alpha byte order.
    Rgba8,
    /// Blue, green, red, alpha byte order, commonly returned by GPU or platform frame buffers.
    Bgra8,
}

/// Raw raster frame.
///
/// Frames are always 8-bit RGBA-like buffers with a declared byte order and row stride. They are
/// intentionally simple so renderers and media writers can exchange pixels without depending on a
/// GUI or image-processing abstraction.
///
/// The type itself does not enforce buffer invariants. Methods that read pixels validate that
/// `stride >= width * 4`, that `stride * height` does not overflow, and that [`Frame::pixels`]
/// contains enough bytes before accessing row data.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Bytes between the start of adjacent rows.
    ///
    /// This may be wider than `width * 4` when a source renderer returns padded rows. It must not
    /// be smaller than `width * 4` for pixel-reading methods such as [`Frame::rgba`].
    pub stride: usize,
    /// Pixel byte order for the buffer.
    pub format: PixelFormat,
    /// Raw pixel bytes.
    ///
    /// The buffer must contain at least `stride * height` bytes. Extra trailing bytes are ignored.
    pub pixels: Vec<u8>,
}

impl Frame {
    /// Return a tightly packed RGBA copy of the frame.
    ///
    /// Writers use this as the common handoff format. The method validates that the buffer is large
    /// enough for the declared geometry and stride before reading each row.
    ///
    /// # Examples
    ///
    /// ```
    /// use betamax_core::media::{Frame, PixelFormat};
    ///
    /// # fn main() -> betamax_core::Result<()> {
    /// let frame = Frame {
    ///     width: 1,
    ///     height: 1,
    ///     stride: 4,
    ///     format: PixelFormat::Bgra8,
    ///     pixels: vec![1, 2, 3, 4],
    /// };
    ///
    /// assert_eq!(frame.rgba()?, vec![3, 2, 1, 4]);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the declared frame geometry overflows, when the row stride is too
    /// small, or when `pixels` does not contain enough bytes for `stride * height`.
    pub fn rgba(&self) -> Result<Vec<u8>> {
        let width = usize::try_from(self.width).into_diagnostic()?;
        let height = usize::try_from(self.height).into_diagnostic()?;
        let row_len = width
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or_else(|| miette!("frame row is too wide"))?;
        let expected_len = self
            .stride
            .checked_mul(height)
            .ok_or_else(|| miette!("frame is too large"))?;
        if self.pixels.len() < expected_len {
            return Err(miette!(
                "frame buffer is too small: expected at least {expected_len} bytes, got {}",
                self.pixels.len()
            )
            .into());
        }

        let mut rgba = Vec::with_capacity(row_len * height);
        for row in self.pixels.chunks(self.stride).take(height) {
            let row = &row[..row_len];
            match self.format {
                PixelFormat::Rgba8 => rgba.extend_from_slice(row),
                PixelFormat::Bgra8 => {
                    for pixel in row.as_chunks::<4>().0 {
                        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                    }
                }
            }
        }
        Ok(rgba)
    }
}

/// Write a single PNG image.
///
/// The frame is validated and normalized through [`Frame::rgba`] before encoding. Parent
/// directories are created automatically, but an existing file is overwritten by `File::create`.
///
/// # Errors
///
/// Returns an error if the parent directory cannot be created, the file cannot be opened, the
/// frame cannot be converted to RGBA, or PNG encoding fails.
pub fn write_png(path: &Path, frame: &Frame) -> Result<()> {
    let rgba = frame.rgba()?;
    ensure_parent_dir(path)?;
    let file = File::create(path)
        .into_diagnostic()
        .map_err(|error| error.wrap_err(format!("failed to create {}", path.display())))?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    Ok(encoder
        .write_header()
        .into_diagnostic()?
        .write_image_data(&rgba)
        .into_diagnostic()?)
}

/// Write a directory of numbered PNG frames.
///
/// The directory is created if needed and frames are named `00000.png`, `00001.png`, and so on.
///
/// # Errors
///
/// Returns an error if no frames are provided, the output directory cannot be created, or any frame
/// cannot be written as PNG.
pub fn write_png_sequence(path: &Path, frames: &[(Frame, Duration)]) -> Result<()> {
    write_png_sequence_with_progress(path, frames, &mut NoMediaProgress)
}

/// Write a directory of numbered PNG frames, reporting progress after each frame.
///
/// See [`write_png_sequence`] for output behavior and error cases.
pub fn write_png_sequence_with_progress(
    path: &Path,
    frames: &[(Frame, Duration)],
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    if frames.is_empty() {
        return Err(miette!("cannot write PNG sequence with no frames").into());
    }
    fs::create_dir_all(path)
        .into_diagnostic()
        .map_err(|error| error.wrap_err(format!("failed to create {}", path.display())))?;
    let total = frames.len();
    for (index, (frame, _)) in frames.iter().enumerate() {
        write_png(
            &path.join(format!("{index:0width$}.png", width = FRAME_INDEX_WIDTH)),
            frame,
        )?;
        progress.report(MediaProgress {
            kind: MediaProgressKind::PngSequence,
            position: index + 1,
            total,
        });
    }
    Ok(())
}

/// Serialize a value as pretty JSON.
///
/// Pretty output is intentional because JSON state files are meant to be inspected and committed in
/// snapshot tests. Callers that need compact JSON can serialize [`crate::ghostty::TerminalState`]
/// directly.
///
/// # Examples
///
/// ```no_run
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Snapshot {
///     text: &'static str,
/// }
///
/// # fn main() -> betamax_core::Result<()> {
/// betamax_core::media::write_json(
///     std::path::Path::new("/tmp/betamax-state.json"),
///     &Snapshot { text: "ready" },
/// )?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns an error if the parent directory cannot be created, the file cannot be opened, or JSON
/// serialization fails.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    ensure_parent_dir(path)?;
    let file = File::create(path)
        .into_diagnostic()
        .map_err(|error| error.wrap_err(format!("failed to create {}", path.display())))?;
    Ok(serde_json::to_writer_pretty(BufWriter::new(file), value).into_diagnostic()?)
}

/// Write an animated GIF with an infinite loop.
///
/// GIF frame delays are stored in hundredths of a second, so sub-10 ms delays are rounded up to the
/// smallest representable non-zero delay.
///
/// # Errors
///
/// Returns an error if no frames are provided, frame dimensions exceed GIF limits, frames have
/// inconsistent dimensions, a frame cannot be converted to RGBA, or file creation/encoding fails.
pub fn write_gif(path: &Path, frames: &[(Frame, Duration)]) -> Result<()> {
    write_gif_with_progress(path, frames, &mut NoMediaProgress)
}

/// Write an animated GIF with an infinite loop, reporting progress after each frame.
///
/// See [`write_gif`] for output behavior and error cases.
pub fn write_gif_with_progress(
    path: &Path,
    frames: &[(Frame, Duration)],
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    let Some((first, _)) = frames.first() else {
        return Err(miette!("cannot write GIF with no frames").into());
    };

    let width = u16::try_from(first.width).map_err(|_| miette!("GIF width is too large"))?;
    let height = u16::try_from(first.height).map_err(|_| miette!("GIF height is too large"))?;
    ensure_parent_dir(path)?;
    let file = File::create(path)
        .into_diagnostic()
        .map_err(|error| error.wrap_err(format!("failed to create {}", path.display())))?;
    let mut counts = HashMap::new();
    let mut any_transparent = false;
    for (frame, _) in frames {
        if frame.width != first.width || frame.height != first.height {
            return Err(miette!("all GIF frames must have the same dimensions").into());
        }
        let mut rgba = frame.rgba()?;
        any_transparent |= gif_binary_alpha(&mut rgba);
        GifPalette::count_colors(&mut counts, &rgba);
    }
    let mut palette = GifPalette::new(counts, any_transparent);

    let mut encoder =
        GifEncoder::new(BufWriter::new(file), width, height, palette.rgb()).into_diagnostic()?;
    encoder.set_repeat(Repeat::Infinite).into_diagnostic()?;

    let total = frames.len();
    for (index, (frame, delay)) in frames.iter().enumerate() {
        let mut rgba = frame.rgba()?;
        let transparent = gif_binary_alpha(&mut rgba);
        let mut gif_frame = GifFrame {
            width,
            height,
            buffer: palette.indices(&rgba).into(),
            delay: gif_delay(*delay),
            ..GifFrame::default()
        };
        if transparent {
            gif_frame.transparent = Some(GIF_TRANSPARENT_INDEX);
            // Clear before the next frame; otherwise earlier pixels show through its clear areas.
            gif_frame.dispose = DisposalMethod::Background;
        }
        encoder.write_frame(&gif_frame).into_diagnostic()?;
        progress.report(MediaProgress {
            kind: MediaProgressKind::Gif,
            position: index + 1,
            total,
        });
    }

    Ok(())
}

/// One palette shared by every frame of a GIF.
///
/// Quantizing each frame separately lets the same source color land on a different palette entry
/// whenever the rest of the frame changes, so static content such as window dots visibly flickers.
/// Building the palette from all frames keeps every source color mapped to the same entry.
struct GifPalette {
    /// Flat RGB palette, with the transparent slot last when reserved.
    rgb: Vec<u8>,
    /// Number of opaque entries in `rgb`.
    opaque_colors: usize,
    /// Palette index for each opaque source color.
    lookup: HashMap<[u8; 3], u8>,
}

impl GifPalette {
    /// Add one frame's opaque pixels to per-color pixel counts.
    ///
    /// Expects straight-alpha RGBA whose alpha is already binary.
    fn count_colors(counts: &mut HashMap<[u8; 3], usize>, rgba: &[u8]) {
        for pixel in rgba.as_chunks::<BYTES_PER_PIXEL>().0 {
            if pixel[3] == u8::MAX {
                *counts.entry([pixel[0], pixel[1], pixel[2]]).or_default() += 1;
            }
        }
    }

    /// Build a palette from the pixel counts of every frame.
    ///
    /// When the colors fit, every one gets an exact entry. Otherwise the most frequent colors
    /// (backgrounds, fills, theme colors, window decorations) keep exact entries and NeuQuant
    /// approximates the rest, such as antialiasing and gradients, in the remaining slots.
    /// `transparent` reserves [`GIF_TRANSPARENT_INDEX`] for clear pixels.
    fn new(counts: HashMap<[u8; 3], usize>, transparent: bool) -> Self {
        let opaque_colors = GIF_PALETTE_SIZE - usize::from(transparent);
        let mut colors: Vec<_> = counts.into_iter().collect();
        // Most frequent first; ties by color so output is deterministic.
        colors.sort_unstable_by(|(a, a_count), (b, b_count)| b_count.cmp(a_count).then(a.cmp(b)));

        let exact = if colors.len() <= opaque_colors {
            colors.len()
        } else {
            GIF_EXACT_COLORS
        };
        let mut rgb: Vec<u8> = colors[..exact]
            .iter()
            .flat_map(|(color, _)| *color)
            .collect();
        let rest = &colors[exact..];
        if !rest.is_empty() {
            // Train on the remaining colors in proportion to their pixel counts, capped so long
            // animations stay bounded. Every color contributes at least one sample.
            let total: usize = rest.iter().map(|(_, count)| count).sum();
            let scale = total.div_ceil(GIF_MAX_TRAINING_PIXELS);
            let mut training = Vec::new();
            for (color, count) in rest {
                for _ in 0..count.div_ceil(scale) {
                    training.extend_from_slice(&[color[0], color[1], color[2], u8::MAX]);
                }
            }
            while training.len() < GIF_MIN_TRAINING_PIXELS * BYTES_PER_PIXEL {
                training.extend_from_within(..);
            }
            let quantizer = color_quant::NeuQuant::new(
                GIF_QUANTIZATION_SPEED,
                opaque_colors - exact,
                &training,
            );
            rgb.extend(quantizer.color_map_rgb());
        }
        let lookup = colors[..exact]
            .iter()
            .zip(0..=u8::MAX)
            .map(|((color, _), index)| (*color, index))
            .collect();
        let opaque_colors = rgb.len() / 3;
        rgb.resize(GIF_PALETTE_SIZE * 3, 0);
        Self {
            rgb,
            opaque_colors,
            lookup,
        }
    }

    /// Flat RGB palette for the GIF's global color table.
    fn rgb(&self) -> &[u8] {
        &self.rgb
    }

    /// Map one frame's pixels to palette indices.
    fn indices(&mut self, rgba: &[u8]) -> Vec<u8> {
        rgba.as_chunks::<BYTES_PER_PIXEL>()
            .0
            .iter()
            .map(|pixel| {
                if pixel[3] < u8::MAX {
                    return GIF_TRANSPARENT_INDEX;
                }
                let color = [pixel[0], pixel[1], pixel[2]];
                *self
                    .lookup
                    .entry(color)
                    .or_insert_with(|| nearest(&self.rgb[..self.opaque_colors * 3], color))
            })
            .collect()
    }
}

/// Index of the palette entry closest to `color` by squared RGB distance.
fn nearest(rgb: &[u8], color: [u8; 3]) -> u8 {
    let distance = |entry: &[u8; 3]| -> u32 {
        entry
            .iter()
            .zip(color)
            .map(|(a, b)| u32::from(a.abs_diff(b)).pow(2))
            .sum()
    };
    let (index, _) = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .min_by_key(|(_, entry)| distance(entry))
        .expect("palette has at least one entry");
    u8::try_from(index).expect("palette has at most 256 entries")
}

/// Reduce straight alpha to GIF's single transparent index, returning whether any pixel is clear.
///
/// The encoder would otherwise make every non-zero alpha opaque, turning soft shadows into solid
/// black. Partially transparent pixels are dropped instead, so GIF keeps only opaque content.
fn gif_binary_alpha(rgba: &mut [u8]) -> bool {
    let mut transparent = false;
    for pixel in rgba.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
        if pixel[3] < u8::MAX {
            pixel.copy_from_slice(&[0, 0, 0, 0]);
            transparent = true;
        }
    }
    transparent
}

/// Write an MP4 video through `ffmpeg`.
///
/// Video support is process-backed for now. The Rust side writes deterministic temporary PNG frames
/// and delegates encoding details to the user's installed `ffmpeg`. Captured durations, including
/// the last frame's hold, determine playback timing. Transitions and total duration are rounded to
/// the nearest output frame interval (at least one frame overall); very short states may be
/// dropped. `framerate` selects that output cadence, not the duration of each captured image.
///
/// # Errors
///
/// Returns an error if the frame list is empty, the rate is non-finite, `ffmpeg` is not on `PATH`,
/// temporary PNG or timeline creation fails, ffmpeg exits unsuccessfully, or cleanup of the
/// temporary frame directory fails after encoding.
pub fn write_mp4(path: &Path, frames: &[(Frame, Duration)], framerate: f64) -> Result<()> {
    write_video(path, frames, framerate, VideoFormat::Mp4)
}

/// Write an MP4 video through `ffmpeg`, reporting progress while writing temporary frames.
///
/// See [`write_mp4`] for output behavior and error cases.
pub fn write_mp4_with_progress(
    path: &Path,
    frames: &[(Frame, Duration)],
    framerate: f64,
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    write_video_with_progress(path, frames, framerate, VideoFormat::Mp4, progress)
}

/// Write a WebM video through `ffmpeg`.
///
/// See [`write_mp4`] for the process-backed encoding tradeoff. WebM uses VP9-specific ffmpeg
/// arguments selected by the private video-format helper.
///
/// # Errors
///
/// Returns an error if the frame list is empty, the rate is non-finite, `ffmpeg` is not on `PATH`,
/// temporary PNG or timeline creation fails, ffmpeg exits unsuccessfully, or cleanup of the
/// temporary frame directory fails after encoding.
pub fn write_webm(path: &Path, frames: &[(Frame, Duration)], framerate: f64) -> Result<()> {
    write_video(path, frames, framerate, VideoFormat::Webm)
}

/// Write a WebM video through `ffmpeg`, reporting progress while writing temporary frames.
///
/// See [`write_webm`] for output behavior and error cases.
pub fn write_webm_with_progress(
    path: &Path,
    frames: &[(Frame, Duration)],
    framerate: f64,
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    write_video_with_progress(path, frames, framerate, VideoFormat::Webm, progress)
}

/// Video container and codec preset selected from the requested output extension.
#[derive(Debug, Clone, Copy)]
enum VideoFormat {
    /// H.264-compatible MP4 output using ffmpeg's default video encoder.
    Mp4,
    /// VP9 WebM output with a quality-oriented constant-rate-factor preset.
    Webm,
}

impl VideoFormat {
    /// File extension used in error messages and temporary directory names.
    fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
        }
    }

    /// Format-specific ffmpeg arguments.
    ///
    /// Input frame sequence arguments are shared by both formats and are assembled by
    /// [`write_video_inner`]. VP9 keeps an alpha plane only when a frame needs one; H.264 has
    /// none, so MP4 flattens any transparency.
    fn args(self, alpha: bool) -> &'static [&'static str] {
        match self {
            Self::Mp4 => &["-pix_fmt", "yuv420p", "-movflags", "+faststart"],
            Self::Webm if alpha => &[
                "-c:v",
                "libvpx-vp9",
                "-pix_fmt",
                "yuva420p",
                "-b:v",
                "0",
                "-crf",
                WEBM_CRF,
            ],
            Self::Webm => &[
                "-c:v",
                "libvpx-vp9",
                "-pix_fmt",
                "yuv420p",
                "-b:v",
                "0",
                "-crf",
                WEBM_CRF,
            ],
        }
    }
}

/// Write a video by materializing temporary PNG frames and invoking ffmpeg.
///
/// This is intentionally not streamed to stdin yet. The temporary sequence is slower and uses more
/// disk, but it is simple to debug and works with both MP4 and WebM encoders.
///
/// Cleanup is attempted after encoding. If encoding succeeds but temporary directory removal fails,
/// the cleanup error is returned because leftover frame directories can become large during
/// repeated example generation.
fn write_video(
    path: &Path,
    frames: &[(Frame, Duration)],
    framerate: f64,
    format: VideoFormat,
) -> Result<()> {
    write_video_with_progress(path, frames, framerate, format, &mut NoMediaProgress)
}

fn write_video_with_progress(
    path: &Path,
    frames: &[(Frame, Duration)],
    framerate: f64,
    format: VideoFormat,
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    if frames.is_empty() {
        return Err(miette!(
            "cannot write {} with no frames",
            format.extension().to_uppercase()
        )
        .into());
    }
    if !framerate.is_finite() {
        return Err(miette!("video framerate must be finite").into());
    }
    let ffmpeg = which("ffmpeg").ok_or_else(|| {
        miette!(
            "{} output requires ffmpeg on PATH",
            format.extension().to_uppercase()
        )
    })?;
    ensure_parent_dir(path)?;
    let temp_dir = std::env::temp_dir().join(format!(
        "betamax-{}-{}-{}-{}",
        format.extension(),
        std::process::id(),
        VIDEO_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).into_diagnostic()?;
    let result = write_video_inner(
        path, frames, framerate, format, &ffmpeg, &temp_dir, progress,
    );
    let cleanup = fs::remove_dir_all(&temp_dir).into_diagnostic();
    result?;
    cleanup?;
    Ok(())
}

/// Ensure the parent directory for a file output exists.
///
/// Relative paths in the current directory have no parent to create and are accepted as-is.
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .into_diagnostic()
            .map_err(|error| error.wrap_err(format!("failed to create {}", parent.display())))?;
    }
    Ok(())
}

/// Perform the ffmpeg invocation for video output.
///
/// `framerate` is clamped to at least 1 FPS because ffmpeg rejects zero or negative rates and the
/// runner already handles playback speed before this point.
fn write_video_inner(
    path: &Path,
    frames: &[(Frame, Duration)],
    framerate: f64,
    format: VideoFormat,
    ffmpeg: &Path,
    temp_dir: &Path,
    progress: &mut impl MediaProgressReporter,
) -> Result<()> {
    let total = frames.len();
    let mut alpha = false;
    for (index, (frame, _)) in frames.iter().enumerate() {
        let rgba = frame.rgba()?;
        alpha = alpha
            || rgba
                .as_chunks::<BYTES_PER_PIXEL>()
                .0
                .iter()
                .any(|p| p[3] < u8::MAX);
        write_png(
            &temp_dir.join(format!(
                "frame-{index:0width$}.png",
                width = FRAME_INDEX_WIDTH
            )),
            frame,
        )?;
        progress.report(MediaProgress {
            kind: MediaProgressKind::VideoFrames,
            position: index + 1,
            total,
        });
    }
    let timeline = temp_dir.join("frames.ffconcat");
    write_video_timeline(&timeline, frames)?;
    let framerate = format!(
        "{:.precision$}",
        framerate.max(MIN_VIDEO_FRAMERATE),
        precision = VIDEO_FRAMERATE_PRECISION
    );
    let mut command = Command::new(ffmpeg);
    command
        .args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&timeline)
        .arg("-vf")
        .arg(format!("fps={framerate}:round=near:eof_action=pass"));
    // Let the EOF timestamp round upward so even subframe clips produce a frame, then cap
    // the result at the nearest whole output interval to exclude the timeline sentinel.
    let rate: f64 = framerate.parse().into_diagnostic()?;
    let duration: Duration = frames.iter().map(|(_, delay)| *delay).sum();
    let output_frames = (duration.as_secs_f64() * rate).round().max(1.0);
    command
        .arg("-frames:v")
        .arg(format!("{output_frames:.0}"))
        .args(format.args(alpha))
        .arg(path);
    let output = command
        .output()
        .into_diagnostic()
        .map_err(|error| error.wrap_err("failed to run ffmpeg for video output"))?;
    if !output.status.success() {
        let mut message = Vec::new();
        message.write_all(&output.stderr).into_diagnostic()?;
        message.write_all(&output.stdout).into_diagnostic()?;
        return Err(miette!(
            "ffmpeg failed while writing {}: {}",
            path.display(),
            String::from_utf8_lossy(&message).trim()
        )
        .into());
    }
    Ok(())
}

/// Describe image dwell times without expanding coalesced frames on disk.
///
/// PNG's default 25 Hz demuxer clock would round timestamps before output resampling. Use a
/// microsecond clock and round cumulative boundaries so sub-microsecond errors do not accumulate.
/// The repeated last image establishes its end timestamp; the output frame limit excludes it.
fn write_video_timeline(path: &Path, frames: &[(Frame, Duration)]) -> Result<()> {
    let mut file = BufWriter::new(File::create(path).into_diagnostic()?);
    writeln!(file, "ffconcat version 1.0").into_diagnostic()?;
    let mut elapsed = Duration::ZERO;
    let mut previous_micros = 0;
    for (index, (_, delay)) in frames.iter().enumerate() {
        elapsed += *delay;
        let micros = (elapsed.as_nanos() + 500) / 1000;
        let hold = micros - previous_micros;
        writeln!(
            file,
            "file frame-{index:05}.png\noption framerate 1000000\nduration {}.{:06}",
            hold / 1_000_000,
            hold % 1_000_000,
        )
        .into_diagnostic()?;
        previous_micros = micros;
    }
    writeln!(
        file,
        "file frame-{:05}.png\noption framerate 1000000",
        frames.len() - 1,
    )
    .into_diagnostic()?;
    file.flush().into_diagnostic()?;
    Ok(())
}

/// Search `PATH` for a program.
///
/// This helper intentionally mirrors the loose preflight checks elsewhere in the crate: it only
/// checks for a file at the joined path and leaves platform-specific executability failures to the
/// eventual process spawn.
fn which(program: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|path| path.join(program))
        .find(|path| path.is_file())
}

/// Convert a Rust duration to GIF's hundredths-of-a-second delay field.
///
/// GIF cannot represent zero-delay frames reliably, so every frame gets at least one hundredth of a
/// second. Very large durations saturate to the maximum GIF delay field.
fn gif_delay(duration: Duration) -> u16 {
    let hundredths = (duration.as_millis() / GIF_DELAY_UNIT_MS).max(MIN_GIF_DELAY_UNITS);
    u16::try_from(hundredths).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_bgra_to_rgba() -> Result<()> {
        let frame = Frame {
            width: 1,
            height: 1,
            stride: 4,
            format: PixelFormat::Bgra8,
            pixels: vec![1, 2, 3, 4],
        };
        assert_eq!(frame.rgba()?, vec![3, 2, 1, 4]);
        Ok(())
    }

    #[test]
    fn gif_keeps_only_opaque_pixels_and_clears_between_transparent_frames() -> Result<()> {
        let mut rgba = vec![10, 20, 30, 255, 0, 0, 0, 140, 9, 9, 9, 0];
        assert!(gif_binary_alpha(&mut rgba));
        assert_eq!(rgba, vec![10, 20, 30, 255, 0, 0, 0, 0, 0, 0, 0, 0]);
        let mut opaque = vec![10, 20, 30, 255];
        assert!(!gif_binary_alpha(&mut opaque));

        let path = temp_path("betamax-transparent.gif");
        let frame = Frame {
            width: 2,
            height: 1,
            stride: 8,
            format: PixelFormat::Rgba8,
            pixels: vec![10, 20, 30, 255, 0, 0, 0, 140],
        };
        write_gif(&path, &[(frame, Duration::from_millis(100))])?;
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::Indexed);
        let mut decoder = options
            .read_info(File::open(&path).into_diagnostic()?)
            .into_diagnostic()?;
        let decoded = decoder.read_next_frame().into_diagnostic()?.unwrap();
        assert_eq!(decoded.dispose, DisposalMethod::Background);
        let transparent = decoded.transparent.expect("transparent index");
        assert_ne!(decoded.buffer[0], transparent);
        assert_eq!(decoded.buffer[1], transparent);
        fs::remove_file(&path).into_diagnostic()?;
        Ok(())
    }

    #[test]
    fn gif_keeps_unchanged_colors_stable_across_frames() -> Result<()> {
        // A fixed dot beside 300 colors that shift every frame, so each frame needs quantizing.
        let frame = |shift: u8| {
            let mut pixels = vec![0xe0, 0x6c, 0x75, 255];
            for index in 0..300_u16 {
                let [high, low] = index.to_be_bytes();
                pixels.extend_from_slice(&[low.wrapping_add(shift), high * 100, shift, 255]);
            }
            Frame {
                width: 301,
                height: 1,
                stride: 301 * 4,
                format: PixelFormat::Rgba8,
                pixels,
            }
        };
        let path = temp_path("betamax-stable-palette.gif");
        let frames: Vec<_> = (0..4)
            .map(|step| (frame(step * 60), Duration::from_millis(100)))
            .collect();
        write_gif(&path, &frames)?;

        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options
            .read_info(File::open(&path).into_diagnostic()?)
            .into_diagnostic()?;
        let mut dots = Vec::new();
        while let Some(decoded) = decoder.read_next_frame().into_diagnostic()? {
            dots.push(decoded.buffer[..BYTES_PER_PIXEL].to_vec());
        }
        assert_eq!(dots.len(), 4);
        assert!(dots.windows(2).all(|pair| pair[0] == pair[1]), "{dots:?}");
        fs::remove_file(&path).into_diagnostic()?;
        Ok(())
    }

    #[test]
    fn webm_keeps_an_alpha_plane_only_when_frames_need_one() {
        assert!(VideoFormat::Webm.args(true).contains(&"yuva420p"));
        assert!(VideoFormat::Webm.args(false).contains(&"yuv420p"));
        assert!(VideoFormat::Mp4.args(true).contains(&"yuv420p"));
    }

    #[test]
    fn missing_program_lookup_returns_none() {
        assert!(which("betamax-definitely-not-installed").is_none());
    }

    #[test]
    fn gif_writer_reports_frame_progress() -> Result<()> {
        let path = temp_path("betamax-progress.gif");
        let frames = test_frames();
        let mut progress = RecordingProgress::default();

        write_gif_with_progress(&path, &frames, &mut progress)?;

        assert_eq!(
            progress.events,
            vec![
                MediaProgress {
                    kind: MediaProgressKind::Gif,
                    position: 1,
                    total: 2,
                },
                MediaProgress {
                    kind: MediaProgressKind::Gif,
                    position: 2,
                    total: 2,
                },
            ]
        );
        let _ = fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn png_sequence_writer_reports_frame_progress() -> Result<()> {
        let path = temp_path("betamax-progress-frames");
        let frames = test_frames();
        let mut progress = RecordingProgress::default();

        write_png_sequence_with_progress(&path, &frames, &mut progress)?;

        assert_eq!(
            progress.events,
            vec![
                MediaProgress {
                    kind: MediaProgressKind::PngSequence,
                    position: 1,
                    total: 2,
                },
                MediaProgress {
                    kind: MediaProgressKind::PngSequence,
                    position: 2,
                    total: 2,
                },
            ]
        );
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    fn test_frames() -> Vec<(Frame, Duration)> {
        vec![
            (
                Frame {
                    width: 1,
                    height: 1,
                    stride: 4,
                    format: PixelFormat::Rgba8,
                    pixels: vec![255, 0, 0, 255],
                },
                Duration::from_millis(20),
            ),
            (
                Frame {
                    width: 1,
                    height: 1,
                    stride: 4,
                    format: PixelFormat::Rgba8,
                    pixels: vec![0, 0, 255, 255],
                },
                Duration::from_millis(20),
            ),
        ]
    }

    fn temp_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!("{}-{suffix}-{name}", std::process::id(),))
    }

    #[derive(Debug, Default)]
    struct RecordingProgress {
        events: Vec<MediaProgress>,
    }

    impl MediaProgressReporter for RecordingProgress {
        fn report(&mut self, progress: MediaProgress) {
            self.events.push(progress);
        }
    }
}
