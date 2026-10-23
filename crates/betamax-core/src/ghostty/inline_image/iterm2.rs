//! iTerm2 inline images (`OSC 1337 ; File = <args> : <base64> BEL`).
//!
//! Only `inline=1` files are images; other files are downloads and are dropped. Sizes follow
//! iTerm2: `N` cells, `Npx` pixels, `N%` of the terminal, or `auto` for the image's own size.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use image::imageops::{self, FilterType};
use image::RgbaImage;

use super::Extent;

/// An iTerm2 image decoded and scaled to the size it occupies on screen.
pub(super) struct Iterm2Image {
    /// Scaled pixels.
    pub(super) image: RgbaImage,
    /// Whether the cursor stays put (`doNotMoveCursor=1`) instead of moving past the image.
    pub(super) keep_cursor: bool,
}

/// Parse and decode the body of an `OSC 1337 ; File=` sequence (everything after `File=`).
///
/// `terminal` is the grid size in cells and `cell` the cell size in pixels, used to resolve
/// cell and percentage sizes. Returns `None` for non-inline files and undecodable images.
pub(super) fn decode(body: &[u8], terminal: Extent, cell: Extent) -> Option<Iterm2Image> {
    let split = body.iter().position(|&byte| byte == b':')?;
    let (args, payload) = (&body[..split], &body[split + 1..]);
    let args = String::from_utf8_lossy(args);
    let mut inline = false;
    let mut width = Size::Auto;
    let mut height = Size::Auto;
    let mut preserve_aspect = true;
    let mut keep_cursor = false;
    for arg in args.split(';') {
        let Some((key, value)) = arg.split_once('=') else {
            continue;
        };
        match key {
            "inline" => inline = value == "1",
            "width" => width = Size::parse(value),
            "height" => height = Size::parse(value),
            "preserveAspectRatio" => preserve_aspect = value != "0",
            "doNotMoveCursor" => keep_cursor = value == "1",
            _ => {}
        }
    }
    if !inline {
        tracing::debug!("dropping non-inline iTerm2 file transfer");
        return None;
    }
    let payload: Vec<u8> = payload
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let bytes = STANDARD
        .decode(payload)
        .inspect_err(|error| tracing::debug!(%error, "invalid iTerm2 image base64"))
        .ok()?;
    let decoded = image::load_from_memory(&bytes)
        .inspect_err(|error| tracing::debug!(%error, "undecodable iTerm2 image"))
        .ok()?
        .to_rgba8();

    let (natural_width, natural_height) = decoded.dimensions();
    let box_width = width.pixels(terminal.width, cell.width);
    let box_height = height.pixels(terminal.height, cell.height);
    let (target_width, target_height) = match (box_width, box_height) {
        (None, None) => (natural_width, natural_height),
        (Some(width), None) => (width, scale(natural_height, width, natural_width)),
        (None, Some(height)) => (scale(natural_width, height, natural_height), height),
        (Some(width), Some(height)) if preserve_aspect => {
            let by_width = scale(natural_height, width, natural_width);
            if by_width <= height {
                (width, by_width)
            } else {
                (scale(natural_width, height, natural_height), height)
            }
        }
        (Some(width), Some(height)) => (width, height),
    };
    let image = if (target_width, target_height) == (natural_width, natural_height) {
        decoded
    } else {
        imageops::resize(
            &decoded,
            target_width.max(1),
            target_height.max(1),
            FilterType::Triangle,
        )
    };
    Some(Iterm2Image { image, keep_cursor })
}

/// `value * numerator / denominator`, rounded, at least one.
fn scale(value: u32, numerator: u32, denominator: u32) -> u32 {
    let scaled = u64::from(value) * u64::from(numerator) / u64::from(denominator.max(1));
    u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
}

/// One iTerm2 width or height argument.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Size {
    Auto,
    Cells(u32),
    Pixels(u32),
    Percent(u32),
}

impl Size {
    fn parse(value: &str) -> Self {
        let number = |text: &str| text.trim().parse::<u32>().ok();
        if let Some(pixels) = value.strip_suffix("px").and_then(number) {
            Self::Pixels(pixels)
        } else if let Some(percent) = value.strip_suffix('%').and_then(number) {
            Self::Percent(percent)
        } else if let Some(cells) = number(value) {
            Self::Cells(cells)
        } else {
            Self::Auto
        }
    }

    /// Resolve to pixels given the terminal extent in cells and the cell extent in pixels.
    fn pixels(self, terminal_cells: u32, cell_pixels: u32) -> Option<u32> {
        match self {
            Self::Auto => None,
            Self::Cells(cells) => Some(cells.saturating_mul(cell_pixels)),
            Self::Pixels(pixels) => Some(pixels),
            Self::Percent(percent) => Some(
                terminal_cells
                    .saturating_mul(cell_pixels)
                    .saturating_mul(percent)
                    / 100,
            ),
        }
        .filter(|&pixels| pixels > 0)
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageFormat, Rgba};

    use super::*;

    fn body(args: &str, width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, Rgba([1, 2, 3, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png).unwrap();
        format!("{args}:{}", STANDARD.encode(png.into_inner())).into_bytes()
    }

    const TERMINAL: Extent = Extent {
        width: 80,
        height: 24,
    };
    const CELL: Extent = Extent {
        width: 10,
        height: 20,
    };

    #[test]
    fn natural_size_by_default() {
        let image = decode(&body("inline=1;size=99", 30, 15), TERMINAL, CELL).unwrap();
        assert_eq!(image.image.dimensions(), (30, 15));
        assert!(!image.keep_cursor);
    }

    #[test]
    fn cell_width_keeps_aspect_ratio() {
        let image = decode(&body("inline=1;width=6", 30, 15), TERMINAL, CELL).unwrap();
        assert_eq!(image.image.dimensions(), (60, 30));
    }

    #[test]
    fn box_fits_preserving_aspect_unless_disabled() {
        let fitted = decode(
            &body("inline=1;width=100px;height=20px", 30, 15),
            TERMINAL,
            CELL,
        )
        .unwrap();
        assert_eq!(fitted.image.dimensions(), (40, 20));
        let stretched = decode(
            &body(
                "inline=1;width=100px;height=20px;preserveAspectRatio=0;doNotMoveCursor=1",
                30,
                15,
            ),
            TERMINAL,
            CELL,
        )
        .unwrap();
        assert_eq!(stretched.image.dimensions(), (100, 20));
        assert!(stretched.keep_cursor);
    }

    #[test]
    fn percent_is_relative_to_the_terminal() {
        let image = decode(&body("inline=1;width=50%", 30, 15), TERMINAL, CELL).unwrap();
        assert_eq!(image.image.dimensions(), (400, 200));
    }

    #[test]
    fn downloads_and_garbage_are_dropped() {
        assert!(decode(&body("name=eA==;size=3", 4, 4), TERMINAL, CELL).is_none());
        assert!(decode(b"inline=1:not base64!", TERMINAL, CELL).is_none());
        assert!(decode(b"inline=1", TERMINAL, CELL).is_none());
    }
}
