//! Image margin fills: syntax, path resolution, decoding, and fitting to the output canvas.
//!
//! A bare path is the VHS form and stretches to the output like VHS's ffmpeg `scale` filter.
//! `image(path[, fit])` is Betamax's form and defaults to `cover`. Both decode once per run.

use std::path::{Path, PathBuf};

use ::image::imageops::{self, FilterType};
use ::image::{ImageReader, Limits, RgbaImage};
use miette::miette;

use super::super::canvas::Canvas;
use crate::Result;

/// Largest accepted image side, which keeps a mistaken huge file from exhausting memory.
const MAX_IMAGE_SIDE: u32 = 16_384;

/// How an image maps onto an output canvas whose aspect ratio may differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runner) enum ImageFit {
    /// Scale uniformly to fill the output, cropping the overflow equally from both sides.
    Cover,
    /// Scale uniformly to fit inside the output, centered over the theme background.
    Contain,
    /// Repeat at native size from the top-left corner.
    Tile,
    /// Scale each axis to the output size, distorting the aspect ratio as VHS does.
    Stretch,
}

impl ImageFit {
    fn parse(value: &str) -> Option<Self> {
        [
            ("cover", Self::Cover),
            ("contain", Self::Contain),
            ("tile", Self::Tile),
            ("stretch", Self::Stretch),
        ]
        .into_iter()
        .find_map(|(name, fit)| value.eq_ignore_ascii_case(name).then_some(fit))
    }
}

/// An image backdrop as written in the tape, before path resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runner) struct MarginImage {
    pub(super) path: PathBuf,
    pub(super) fit: ImageFit,
}

impl MarginImage {
    /// Recognize an image fill.
    ///
    /// Returns `None` for values owned by color and gradient parsing: empty, `#…`, `linear(…)`,
    /// and `transparent`. Every other value is an image: `image(…)` must be well formed, and
    /// anything else is a bare VHS-style path.
    pub(in crate::runner) fn parse(value: &str) -> Result<Option<Self>> {
        let value = value.trim();
        if value.is_empty()
            || value.starts_with('#')
            || value.starts_with("linear(")
            || value.eq_ignore_ascii_case("transparent")
        {
            return Ok(None);
        }
        let Some(arguments) = value.strip_prefix("image(") else {
            return Ok(Some(Self {
                path: PathBuf::from(value),
                fit: ImageFit::Stretch,
            }));
        };
        let malformed = || {
            miette!(
                "invalid MarginFill \"{value}\": expected image(<path>) or image(<path>, cover|contain|tile|stretch)"
            )
        };
        let arguments = arguments.strip_suffix(')').ok_or_else(malformed)?;
        let mut arguments = arguments.split(',').map(str::trim);
        let path = arguments.next().filter(|path| !path.is_empty());
        let fit = match arguments.next() {
            None => Some(ImageFit::Cover),
            Some(fit) => ImageFit::parse(fit),
        };
        match (path, fit, arguments.next()) {
            (Some(path), Some(fit), None) => Ok(Some(Self {
                path: PathBuf::from(path),
                fit,
            })),
            _ => Err(malformed().into()),
        }
    }

    /// Decode and fit the image to a full output canvas.
    ///
    /// `fallback` fills the uncovered area of `contain`. Image alpha is kept, so a translucent
    /// picture makes the backdrop translucent.
    pub(in crate::runner) fn backdrop(
        &self,
        width: u32,
        height: u32,
        tape_dir: Option<&Path>,
        fallback: libghostty_vt::style::RgbColor,
    ) -> Result<Canvas> {
        let path = self.resolve(tape_dir)?;
        let image = decode(&path)?;
        let pixels = fit(&image, self.fit, width, height, fallback);
        Canvas::from_rgba(width, height, pixels.into_raw())
    }

    /// Resolve against the working directory first, as VHS does, then the tape's directory.
    fn resolve(&self, tape_dir: Option<&Path>) -> Result<PathBuf> {
        let path = expand_home(&self.path);
        if path.exists() {
            return Ok(path);
        }
        let mut looked = vec![std::path::absolute(&path).unwrap_or_else(|_| path.clone())];
        if let Some(dir) = tape_dir.filter(|_| path.is_relative()) {
            let beside_tape = dir.join(&path);
            if beside_tape.exists() {
                return Ok(beside_tape);
            }
            looked.push(beside_tape);
        }
        let looked = looked
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(" or ");
        Err(miette!(
            "MarginFill image not found: {} (looked for {looked})",
            self.path.display()
        )
        .into())
    }
}

/// Expand a leading `~/` to the home directory.
fn expand_home(path: &Path) -> PathBuf {
    match (path.strip_prefix("~"), std::env::var_os("HOME")) {
        (Ok(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => path.to_path_buf(),
    }
}

/// Decode a PNG, JPEG, or WebP file into straight-alpha RGBA8, sniffing the format from content.
fn decode(path: &Path) -> Result<RgbaImage> {
    let failed = |cause: String| {
        miette!(
            "failed to read MarginFill image {}: {cause}",
            path.display()
        )
    };
    let mut reader = ImageReader::open(path)
        .map_err(|error| failed(error.to_string()))?
        .with_guessed_format()
        .map_err(|error| failed(error.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    reader.limits(limits);
    let image = reader.decode().map_err(|error| failed(error.to_string()))?;
    Ok(image.into_rgba8())
}

/// Size and offset of the scaled image inside the output, before cropping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placement {
    width: u32,
    height: u32,
    x: i64,
    y: i64,
}

/// Uniformly scale to cover or fit inside the output, centered.
fn uniform_placement(image: &RgbaImage, width: u32, height: u32, cover: bool) -> Placement {
    let scale_x = f64::from(width) / f64::from(image.width());
    let scale_y = f64::from(height) / f64::from(image.height());
    let scale = if cover {
        scale_x.max(scale_y)
    } else {
        scale_x.min(scale_y)
    };
    let scaled = |side: u32, output: u32| {
        let side = (f64::from(side) * scale).round().max(1.0) as u32;
        if cover {
            side.max(output)
        } else {
            side.min(output)
        }
    };
    let scaled_width = scaled(image.width(), width);
    let scaled_height = scaled(image.height(), height);
    Placement {
        width: scaled_width,
        height: scaled_height,
        x: (i64::from(width) - i64::from(scaled_width)) / 2,
        y: (i64::from(height) - i64::from(scaled_height)) / 2,
    }
}

/// Produce an output-sized straight-alpha image for one fit mode.
///
/// `contain` leaves its uncovered area opaque `fallback`; image pixels, alpha included, replace it.
fn fit(
    image: &RgbaImage,
    fit: ImageFit,
    width: u32,
    height: u32,
    fallback: libghostty_vt::style::RgbColor,
) -> RgbaImage {
    match fit {
        ImageFit::Stretch => resize(image, width, height),
        ImageFit::Tile => RgbaImage::from_fn(width, height, |x, y| {
            *image.get_pixel(x % image.width(), y % image.height())
        }),
        ImageFit::Cover | ImageFit::Contain => {
            let placement = uniform_placement(image, width, height, fit == ImageFit::Cover);
            let scaled = resize(image, placement.width, placement.height);
            let mut output = if fit == ImageFit::Contain {
                RgbaImage::from_pixel(
                    width,
                    height,
                    ::image::Rgba([fallback.r, fallback.g, fallback.b, u8::MAX]),
                )
            } else {
                RgbaImage::new(width, height)
            };
            imageops::replace(&mut output, &scaled, placement.x, placement.y);
            output
        }
    }
}

/// Lanczos3 resampling, premultiplied when the image has alpha so clear pixels do not bleed
/// their (usually black) color into visible edges.
fn resize(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    if image.dimensions() == (width, height) {
        return image.clone();
    }
    let has_alpha = image.pixels().any(|pixel| pixel[3] != u8::MAX);
    if !has_alpha {
        return imageops::resize(image, width, height, FilterType::Lanczos3);
    }
    let mut premultiplied = image.clone();
    for pixel in premultiplied.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    let mut resized = imageops::resize(&premultiplied, width, height, FilterType::Lanczos3);
    for pixel in resized.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = (u16::from(*channel) * 255 + alpha / 2)
                .checked_div(alpha)
                .map_or(0, |value| value.min(255) as u8);
        }
    }
    resized
}

#[cfg(test)]
mod tests;
