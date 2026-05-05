//! Media file writers and frame conversion.
//!
//! The renderer produces raw frames. This module owns the format-specific side effects for PNG,
//! GIF, JSON, and video outputs. Video is intentionally implemented through `ffmpeg` for now so the
//! Rust code only has to provide a deterministic PNG frame sequence and clear error reporting.
//!
//! Most users should let [`crate::Runner`] write media outputs from a tape. Use this module when
//! embedding Betamax's renderer directly or when tests need to inspect/write raw frames.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gif::{Encoder as GifEncoder, Frame as GifFrame, Repeat};
use miette::{miette, IntoDiagnostic};
use serde::Serialize;

use crate::Result;

/// Number of bytes in one RGBA or BGRA pixel.
const BYTES_PER_PIXEL: usize = 4;
/// Width used for numbered PNG sequence frame names.
const FRAME_INDEX_WIDTH: usize = 5;
/// Quantization speed passed to the `gif` crate.
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
                    for pixel in row.chunks_exact(4) {
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
    if frames.is_empty() {
        return Err(miette!("cannot write PNG sequence with no frames").into());
    }
    fs::create_dir_all(path)
        .into_diagnostic()
        .map_err(|error| error.wrap_err(format!("failed to create {}", path.display())))?;
    for (index, (frame, _)) in frames.iter().enumerate() {
        write_png(
            &path.join(format!("{index:0width$}.png", width = FRAME_INDEX_WIDTH)),
            frame,
        )?;
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
    let Some((first, _)) = frames.first() else {
        return Err(miette!("cannot write GIF with no frames").into());
    };

    let width = u16::try_from(first.width).map_err(|_| miette!("GIF width is too large"))?;
    let height = u16::try_from(first.height).map_err(|_| miette!("GIF height is too large"))?;
    ensure_parent_dir(path)?;
    let file = File::create(path)
        .into_diagnostic()
        .map_err(|error| error.wrap_err(format!("failed to create {}", path.display())))?;
    let mut encoder =
        GifEncoder::new(BufWriter::new(file), width, height, &[]).into_diagnostic()?;
    encoder.set_repeat(Repeat::Infinite).into_diagnostic()?;

    for (frame, delay) in frames {
        if frame.width != first.width || frame.height != first.height {
            return Err(miette!("all GIF frames must have the same dimensions").into());
        }
        let mut rgba = frame.rgba()?;
        let mut gif_frame =
            GifFrame::from_rgba_speed(width, height, &mut rgba, GIF_QUANTIZATION_SPEED);
        gif_frame.delay = gif_delay(*delay);
        encoder.write_frame(&gif_frame).into_diagnostic()?;
    }

    Ok(())
}

/// Write an MP4 video through `ffmpeg`.
///
/// Video support is process-backed for now. The Rust side writes deterministic temporary PNG frames
/// and delegates encoding details to the user's installed `ffmpeg`.
///
/// # Errors
///
/// Returns an error if `ffmpeg` is not on `PATH`, if temporary PNG frame creation fails, if ffmpeg
/// exits unsuccessfully, or if cleanup of the temporary frame directory fails after encoding.
pub fn write_mp4(path: &Path, frames: &[(Frame, Duration)], framerate: f64) -> Result<()> {
    write_video(path, frames, framerate, VideoFormat::Mp4)
}

/// Write a WebM video through `ffmpeg`.
///
/// See [`write_mp4`] for the process-backed encoding tradeoff. WebM uses VP9-specific ffmpeg
/// arguments selected by the private video-format helper.
///
/// # Errors
///
/// Returns an error if `ffmpeg` is not on `PATH`, if temporary PNG frame creation fails, if ffmpeg
/// exits unsuccessfully, or if cleanup of the temporary frame directory fails after encoding.
pub fn write_webm(path: &Path, frames: &[(Frame, Duration)], framerate: f64) -> Result<()> {
    write_video(path, frames, framerate, VideoFormat::Webm)
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
    /// [`write_video_inner`].
    fn args(self) -> &'static [&'static str] {
        match self {
            Self::Mp4 => &["-pix_fmt", "yuv420p", "-movflags", "+faststart"],
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
    if frames.is_empty() {
        return Err(miette!(
            "cannot write {} with no frames",
            format.extension().to_uppercase()
        )
        .into());
    }
    let ffmpeg = which("ffmpeg").ok_or_else(|| {
        miette!(
            "{} output requires ffmpeg on PATH",
            format.extension().to_uppercase()
        )
    })?;
    ensure_parent_dir(path)?;
    let temp_dir = std::env::temp_dir().join(format!(
        "betamax-{}-{}",
        format.extension(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).into_diagnostic()?;
    let result = write_video_inner(path, frames, framerate, format, &ffmpeg, &temp_dir);
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
) -> Result<()> {
    for (index, (frame, _)) in frames.iter().enumerate() {
        write_png(
            &temp_dir.join(format!(
                "frame-{index:0width$}.png",
                width = FRAME_INDEX_WIDTH
            )),
            frame,
        )?;
    }
    let mut command = Command::new(ffmpeg);
    command
        .arg("-y")
        .arg("-framerate")
        .arg(format!(
            "{:.precision$}",
            framerate.max(MIN_VIDEO_FRAMERATE),
            precision = VIDEO_FRAMERATE_PRECISION
        ))
        .arg("-i")
        .arg(temp_dir.join("frame-%05d.png"));
    command.args(format.args()).arg(path);
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
    fn converts_bgra_to_rgba() {
        let frame = Frame {
            width: 1,
            height: 1,
            stride: 4,
            format: PixelFormat::Bgra8,
            pixels: vec![1, 2, 3, 4],
        };
        assert_eq!(frame.rgba().unwrap(), vec![3, 2, 1, 4]);
    }

    #[test]
    fn missing_program_lookup_returns_none() {
        assert!(which("betamax-definitely-not-installed").is_none());
    }
}
