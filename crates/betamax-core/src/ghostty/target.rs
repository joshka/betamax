//! Tightly packed RGBA target used by the software terminal renderer.

use cosmic_text::Color;
use libghostty_vt::render::CursorVisualStyle;
use libghostty_vt::style::RgbColor;
use miette::miette;

use crate::media::{Frame, PixelFormat};
use crate::Result;

/// Number of bytes in one packed RGBA pixel.
const BYTES_PER_PIXEL: usize = 4;
/// Fully opaque alpha value.
const OPAQUE_ALPHA: u8 = 0xff;
/// Denominator for 8-bit alpha blending.
const ALPHA_DENOMINATOR: u16 = 255;
/// Approximate width of a terminal bar cursor as a fraction of one cell.
const BAR_CURSOR_WIDTH_FRACTION: u32 = 8;
/// Minimum bar cursor width in pixels.
const MIN_BAR_CURSOR_WIDTH: u32 = 1;
/// Underline cursor thickness in pixels.
const UNDERLINE_CURSOR_HEIGHT: u32 = 3;

/// Tightly packed RGBA render target for software rasterization.
///
/// The target is intentionally small and purpose-built: it supports opaque fills, glyph alpha
/// blending, and cursor drawing. Higher-level frame decoration uses a separate target in
/// `runner::settings` so terminal rendering does not need to know about VHS-style chrome.
pub(super) struct PixelTarget {
    /// Target width in pixels.
    width: u32,
    /// Target height in pixels.
    height: u32,
    /// Tightly packed RGBA8 pixels.
    pixels: Vec<u8>,
}

impl PixelTarget {
    /// Allocate an empty target, checking for size overflow.
    pub(super) fn new(width: u32, height: u32) -> Result<Self> {
        let len = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
            .ok_or_else(|| miette!("frame is too large"))?;
        Ok(Self {
            width,
            height,
            pixels: vec![0; len],
        })
    }

    /// Fill the entire target with an opaque color.
    pub(super) fn clear(&mut self, color: RgbColor) {
        for pixel in self.pixels.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
            pixel.copy_from_slice(&[color.r, color.g, color.b, OPAQUE_ALPHA]);
        }
    }

    /// Fill a rectangle after clipping it to the target bounds.
    pub(super) fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: RgbColor) {
        let Some((x0, y0, x1, y1)) = self.clipped_rect(x as i32, y as i32, width, height) else {
            return;
        };
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.set_pixel(xx, yy, color.r, color.g, color.b, OPAQUE_ALPHA);
            }
        }
    }

    /// Alpha-blend a glyph rectangle into the target.
    pub(super) fn blend_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        let [r, g, b, a] = color.as_rgba();
        if a == 0 {
            return;
        }

        let Some((x0, y0, x1, y1)) = self.clipped_rect(x, y, width, height) else {
            return;
        };
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.blend_pixel(xx, yy, r, g, b, a);
            }
        }
    }

    /// Draw a cursor using libghostty-vt's visual style.
    ///
    /// Unknown cursor styles fall back to a filled block because that is the most visible failure
    /// mode for generated documentation and tests.
    pub(super) fn draw_cursor(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: RgbColor,
        style: CursorVisualStyle,
    ) {
        match style {
            CursorVisualStyle::Bar => self.fill_rect(
                x,
                y,
                width.max(MIN_BAR_CURSOR_WIDTH) / BAR_CURSOR_WIDTH_FRACTION + 1,
                height,
                color,
            ),
            CursorVisualStyle::Underline => self.fill_rect(
                x,
                y + height.saturating_sub(UNDERLINE_CURSOR_HEIGHT),
                width,
                UNDERLINE_CURSOR_HEIGHT,
                color,
            ),
            CursorVisualStyle::BlockHollow => {
                self.fill_rect(x, y, width, 1, color);
                self.fill_rect(x, y + height.saturating_sub(1), width, 1, color);
                self.fill_rect(x, y, 1, height, color);
                self.fill_rect(x + width.saturating_sub(1), y, 1, height, color);
            }
            _ => self.fill_rect(x, y, width, height, color),
        }
    }

    /// Convert the target into the shared media frame type.
    pub(super) fn into_frame(self) -> Frame {
        Frame {
            width: self.width,
            height: self.height,
            stride: self.width as usize * BYTES_PER_PIXEL,
            format: PixelFormat::Rgba8,
            pixels: self.pixels,
        }
    }

    /// Return a clipped rectangle as `(x0, y0, x1, y1)`.
    fn clipped_rect(
        &self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<(u32, u32, u32, u32)> {
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x1 = x.saturating_add(width as i32).clamp(0, self.width as i32) as u32;
        let y1 = y.saturating_add(height as i32).clamp(0, self.height as i32) as u32;
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
    }

    /// Write one pixel without blending.
    fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        let offset = self.pixel_offset(x, y);
        self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&[r, g, b, a]);
    }

    /// Alpha-blend one pixel over the existing target color.
    fn blend_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        let offset = self.pixel_offset(x, y);
        let alpha = u16::from(a);
        let inv_alpha = ALPHA_DENOMINATOR - alpha;
        self.pixels[offset] = ((u16::from(r) * alpha + u16::from(self.pixels[offset]) * inv_alpha)
            / ALPHA_DENOMINATOR) as u8;
        self.pixels[offset + 1] = ((u16::from(g) * alpha
            + u16::from(self.pixels[offset + 1]) * inv_alpha)
            / ALPHA_DENOMINATOR) as u8;
        self.pixels[offset + 2] = ((u16::from(b) * alpha
            + u16::from(self.pixels[offset + 2]) * inv_alpha)
            / ALPHA_DENOMINATOR) as u8;
        self.pixels[offset + 3] = OPAQUE_ALPHA;
    }

    /// Return the byte offset for a pixel.
    fn pixel_offset(&self, x: u32, y: u32) -> usize {
        ((y as usize * self.width as usize) + x as usize) * BYTES_PER_PIXEL
    }
}
