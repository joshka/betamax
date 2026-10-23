//! Animated GIF encoding with one palette shared by every frame.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::time::Duration;

use ::gif::{Encoder as GifEncoder, Frame as GifFrame, Repeat};
use miette::{miette, IntoDiagnostic};

use super::{
    ensure_parent_dir, Frame, MediaProgress, MediaProgressKind, MediaProgressReporter,
    NoMediaProgress, BYTES_PER_PIXEL,
};
use crate::Result;

mod delta;

use delta::{DeltaFrame, FrameDelta};

/// Entries in a GIF color table.
const GIF_PALETTE_SIZE: usize = 256;
/// Palette index reserved for clear pixels and, in animations, for pixels left unchanged.
const GIF_TRANSPARENT_INDEX: u8 = 255;
/// Most frequent colors given exact palette entries when an animation needs quantizing.
const GIF_EXACT_COLORS: usize = 128;
/// Approximate cap on NeuQuant training pixels, so long animations train in bounded memory.
const GIF_MAX_TRAINING_PIXELS: usize = 1 << 20;
/// Minimum NeuQuant training pixels, so its learning loop always has samples to step through.
const GIF_MIN_TRAINING_PIXELS: usize = 4096;
/// NeuQuant sampling factor used when a GIF has more colors than its palette holds.
///
/// The value favors reasonable encode speed for CLI-generated demos. A future benchmark pass can
/// tune this if GIF quality or encode time becomes a release concern.
const GIF_QUANTIZATION_SPEED: i32 = 10;
/// Milliseconds represented by one GIF delay unit.
const GIF_DELAY_UNIT_MS: u128 = 10;
/// Minimum non-zero delay GIF viewers reliably honor.
const MIN_GIF_DELAY_UNITS: u128 = 1;

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
    // Unchanged pixels reuse the transparent index, so animations reserve it even when opaque,
    // unless that slot is the only thing standing between 256 colors and exact entries.
    let reserve_transparent =
        any_transparent || (frames.len() > 1 && counts.len() != GIF_PALETTE_SIZE);
    let mut palette = GifPalette::new(counts, reserve_transparent);
    let transparent = reserve_transparent.then_some(GIF_TRANSPARENT_INDEX);

    let mut encoder =
        GifEncoder::new(BufWriter::new(file), width, height, palette.rgb()).into_diagnostic()?;
    encoder.set_repeat(Repeat::Infinite).into_diagnostic()?;
    let mut write = |frame: DeltaFrame| -> Result<()> {
        let rect = frame.rect;
        let gif_frame = GifFrame {
            left: u16::try_from(rect.left).expect("frame fits the GIF screen"),
            top: u16::try_from(rect.top).expect("frame fits the GIF screen"),
            width: u16::try_from(rect.width).expect("frame fits the GIF screen"),
            height: u16::try_from(rect.height).expect("frame fits the GIF screen"),
            buffer: frame.buffer.into(),
            delay: frame.delay,
            dispose: frame.dispose,
            transparent,
            ..GifFrame::default()
        };
        encoder.write_frame(&gif_frame).into_diagnostic()?;
        Ok(())
    };

    let mut delta = FrameDelta::new(usize::from(width), usize::from(height), reserve_transparent);
    let total = frames.len();
    for (index, (frame, delay)) in frames.iter().enumerate() {
        let mut rgba = frame.rgba()?;
        gif_binary_alpha(&mut rgba);
        if let Some(finished) = delta.push(palette.indices(&rgba), *delay) {
            write(finished)?;
        }
        progress.report(MediaProgress {
            kind: MediaProgressKind::Gif,
            position: index + 1,
            total,
        });
    }
    if let Some(last) = delta.finish() {
        write(last)?;
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
        // Terminal frames are mostly long runs of one color; hash once per run, not per pixel.
        let mut run: Option<([u8; 3], usize)> = None;
        for pixel in rgba.as_chunks::<BYTES_PER_PIXEL>().0 {
            if pixel[3] < u8::MAX {
                continue;
            }
            let color = [pixel[0], pixel[1], pixel[2]];
            match &mut run {
                Some((run_color, length)) if *run_color == color => *length += 1,
                _ => {
                    if let Some((run_color, length)) = run.replace((color, 1)) {
                        *counts.entry(run_color).or_default() += length;
                    }
                }
            }
        }
        if let Some((color, length)) = run {
            *counts.entry(color).or_default() += length;
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
        // Reuse the previous pixel's entry across runs of one color, as in `count_colors`.
        let mut last: Option<([u8; 3], u8)> = None;
        rgba.as_chunks::<BYTES_PER_PIXEL>()
            .0
            .iter()
            .map(|pixel| {
                if pixel[3] < u8::MAX {
                    return GIF_TRANSPARENT_INDEX;
                }
                let color = [pixel[0], pixel[1], pixel[2]];
                if let Some((last_color, index)) = last {
                    if last_color == color {
                        return index;
                    }
                }
                let index = *self
                    .lookup
                    .entry(color)
                    .or_insert_with(|| nearest(&self.rgb[..self.opaque_colors * 3], color));
                last = Some((color, index));
                index
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
    use std::fs;

    use super::*;
    use crate::media::tests::{temp_path, test_frames, RecordingProgress};
    use crate::media::PixelFormat;

    /// Decode `path` with an independent decoder that composites each frame onto the canvas.
    fn decode_composited(path: &Path) -> Result<Vec<(Vec<u8>, Duration)>> {
        use image::AnimationDecoder;

        let file = std::io::BufReader::new(File::open(path).into_diagnostic()?);
        let frames = image::codecs::gif::GifDecoder::new(file)
            .into_diagnostic()?
            .into_frames()
            .collect_frames()
            .into_diagnostic()?;
        Ok(frames
            .into_iter()
            .map(|frame| {
                let delay = Duration::from(frame.delay());
                (frame.into_buffer().into_raw(), delay)
            })
            .collect())
    }

    /// Frame rectangle as (left, top, width, height), and delay, as stored in the file.
    type StoredFrame = ((u16, u16, u16, u16), u16);

    /// Raw frame rectangles and delays as stored in the file, before compositing.
    fn stored_frames(path: &Path) -> Result<Vec<StoredFrame>> {
        let mut options = ::gif::DecodeOptions::new();
        options.set_color_output(::gif::ColorOutput::Indexed);
        let mut decoder = options
            .read_info(File::open(path).into_diagnostic()?)
            .into_diagnostic()?;
        let mut frames = Vec::new();
        while let Some(frame) = decoder.read_next_frame().into_diagnostic()? {
            let rect = (frame.left, frame.top, frame.width, frame.height);
            frames.push((rect, frame.delay));
        }
        Ok(frames)
    }

    fn rgba_frame(width: u32, height: u32, pixels: Vec<u8>) -> Frame {
        Frame {
            width,
            height,
            stride: width as usize * BYTES_PER_PIXEL,
            format: PixelFormat::Rgba8,
            pixels,
        }
    }

    /// A 40x20 frame of two colors, with a `color` block of 3x2 pixels at (`x`, `y`).
    fn block_frame(x: usize, y: usize, color: [u8; 4]) -> Frame {
        let mut pixels = Vec::new();
        for row in 0..20 {
            for column in 0..40 {
                pixels.extend_from_slice(&if (x..x + 3).contains(&column)
                    && (y..y + 2).contains(&row)
                {
                    color
                } else if row < 4 {
                    [40, 40, 60, 255]
                } else {
                    [10, 10, 10, 255]
                });
            }
        }
        rgba_frame(40, 20, pixels)
    }

    #[test]
    fn gif_keeps_only_opaque_pixels() -> Result<()> {
        let mut rgba = vec![10, 20, 30, 255, 0, 0, 0, 140, 9, 9, 9, 0];
        assert!(gif_binary_alpha(&mut rgba));
        assert_eq!(rgba, vec![10, 20, 30, 255, 0, 0, 0, 0, 0, 0, 0, 0]);
        let mut opaque = vec![10, 20, 30, 255];
        assert!(!gif_binary_alpha(&mut opaque));

        let path = temp_path("betamax-transparent.gif");
        let frame = rgba_frame(2, 1, vec![10, 20, 30, 255, 0, 0, 0, 140]);
        write_gif(&path, &[(frame, Duration::from_millis(100))])?;
        let decoded = decode_composited(&path)?;
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, vec![10, 20, 30, 255, 0, 0, 0, 0]);
        fs::remove_file(&path).into_diagnostic()?;
        Ok(())
    }

    #[test]
    fn gif_crops_frames_to_the_changed_region() -> Result<()> {
        let path = temp_path("betamax-cropped.gif");
        let frames = [
            (
                block_frame(5, 6, [10, 10, 10, 255]),
                Duration::from_millis(100),
            ),
            (
                block_frame(5, 6, [200, 80, 80, 255]),
                Duration::from_millis(100),
            ),
        ];
        write_gif(&path, &frames)?;

        let stored = stored_frames(&path)?;
        assert_eq!(stored[0].0, (0, 0, 40, 20));
        assert_eq!(stored[1].0, (5, 6, 3, 2));
        fs::remove_file(&path).into_diagnostic()?;
        Ok(())
    }

    #[test]
    fn gif_merges_unchanged_frames_into_one_delay() -> Result<()> {
        let path = temp_path("betamax-merged.gif");
        let frames = [
            (
                block_frame(5, 6, [200, 80, 80, 255]),
                Duration::from_millis(100),
            ),
            (
                block_frame(5, 6, [200, 80, 80, 255]),
                Duration::from_millis(250),
            ),
            (
                block_frame(9, 6, [200, 80, 80, 255]),
                Duration::from_millis(100),
            ),
        ];
        write_gif(&path, &frames)?;

        let stored = stored_frames(&path)?;
        assert_eq!(stored.len(), 2);
        assert_eq!((stored[0].1, stored[1].1), (35, 10));
        fs::remove_file(&path).into_diagnostic()?;
        Ok(())
    }

    #[test]
    fn gif_frames_decode_to_the_original_pixels() -> Result<()> {
        // A moving block with a transparent margin that appears, grows and disappears, so frames
        // need cropping, kept pixels and clearing. Few colors keep every pixel exact.
        let frame = |step: usize| {
            let mut frame = block_frame(4 * step, 2 * step, [200, 80, 80, 255]);
            for (index, pixel) in frame
                .pixels
                .as_chunks_mut::<BYTES_PER_PIXEL>()
                .0
                .iter_mut()
                .enumerate()
            {
                let (x, y) = (index % 40, index / 40);
                if !step.is_multiple_of(3) && (x < step || y + step >= 20) {
                    pixel.copy_from_slice(&[0, 0, 0, 0]);
                }
            }
            (frame, Duration::from_millis(100))
        };
        let frames: Vec<_> = (0..7).map(frame).collect();
        let path = temp_path("betamax-round-trip.gif");
        write_gif(&path, &frames)?;

        let decoded = decode_composited(&path)?;
        assert_eq!(decoded.len(), frames.len());
        for (index, ((expected, _), (actual, delay))) in frames.iter().zip(&decoded).enumerate() {
            assert!(expected.pixels == *actual, "frame {index} differs");
            assert_eq!(*delay, Duration::from_millis(100));
        }
        fs::remove_file(&path).into_diagnostic()?;
        Ok(())
    }

    #[test]
    fn gif_keeps_exactly_256_colors_exact_across_frames() -> Result<()> {
        // Every gray level, so reserving a slot for unchanged pixels would force quantizing.
        let frame = |swap: bool| {
            let mut pixels: Vec<u8> = (0..=u8::MAX)
                .flat_map(|gray| [gray, gray, gray, 255])
                .collect();
            if swap {
                pixels[..2 * BYTES_PER_PIXEL].rotate_left(BYTES_PER_PIXEL);
            }
            rgba_frame(16, 16, pixels)
        };
        let frames = [
            (frame(false), Duration::from_millis(100)),
            (frame(true), Duration::from_millis(100)),
        ];
        let path = temp_path("betamax-256-colors.gif");
        write_gif(&path, &frames)?;

        let decoded = decode_composited(&path)?;
        assert_eq!(decoded.len(), 2);
        for ((expected, _), (actual, _)) in frames.iter().zip(&decoded) {
            assert!(expected.pixels == *actual);
        }
        assert_eq!(stored_frames(&path)?[1].0, (0, 0, 2, 1));
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
            rgba_frame(301, 1, pixels)
        };
        let path = temp_path("betamax-stable-palette.gif");
        let frames: Vec<_> = (0..4)
            .map(|step| (frame(step * 60), Duration::from_millis(100)))
            .collect();
        write_gif(&path, &frames)?;

        let dots: Vec<_> = decode_composited(&path)?
            .into_iter()
            .map(|(pixels, _)| pixels[..BYTES_PER_PIXEL].to_vec())
            .collect();
        assert_eq!(dots.len(), 4);
        assert!(dots.windows(2).all(|pair| pair[0] == pair[1]), "{dots:?}");
        // The dot never changes, so no frame after the first redraws it.
        assert!(stored_frames(&path)?[1..]
            .iter()
            .all(|((left, ..), _)| *left > 0));
        fs::remove_file(&path).into_diagnostic()?;
        Ok(())
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
}
