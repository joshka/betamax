//! Owned RGBA canvas for output-frame decoration.
//!
//! Blitting, clipping, blending, and geometric masks share one opaque pixel representation.
//! Window and overlay modules choose the shapes and colors; presentation owns drawing order.

use cosmic_text::Color;
use miette::miette;

use crate::media::{Frame, PixelFormat};
use crate::Result;

/// Simple RGBA canvas used only for post-render decoration.
///
/// The raw terminal frame may come from any supported [`PixelFormat`], but the decoration path
/// normalizes into tightly packed RGBA because margin fills, window buttons, and rounded-corner
/// masking are easier to reason about in a single byte order.
#[derive(Debug, Clone)]
pub(super) struct Canvas {
    /// Output width in pixels.
    pub(super) width: u32,
    /// Output height in pixels.
    pub(super) height: u32,
    /// RGBA8 pixel buffer with tightly packed rows.
    pixels: Vec<u8>,
}

impl Canvas {
    /// Allocate a solid-color RGBA frame, checking for integer overflow.
    pub(super) fn new(
        width: u32,
        height: u32,
        color: libghostty_vt::style::RgbColor,
    ) -> Result<Self> {
        let len = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
            .ok_or_else(|| miette!("frame is too large"))?;
        let mut frame = Self {
            width,
            height,
            pixels: vec![0; len],
        };
        frame.fill_rect(0, 0, width, height, color);
        Ok(frame)
    }

    /// Replace one in-bounds pixel with an opaque RGB color.
    pub(super) fn set_pixel(&mut self, x: u32, y: u32, color: libghostty_vt::style::RgbColor) {
        let offset = self.offset(x, y);
        self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&[
            color.r,
            color.g,
            color.b,
            OPAQUE_ALPHA,
        ]);
    }

    /// Copy a frame into this frame at a pixel offset.
    ///
    /// The source is converted to RGBA first, so callers can pass either RGBA or BGRA frames.
    pub(super) fn blit(&mut self, frame: &Frame, x: u32, y: u32) -> Result<()> {
        let rgba = frame.rgba()?;
        for row in 0..frame.height {
            for col in 0..frame.width {
                let src = ((row as usize * frame.width as usize) + col as usize) * BYTES_PER_PIXEL;
                let dst = self.offset(x + col, y + row);
                self.pixels[dst..dst + BYTES_PER_PIXEL]
                    .copy_from_slice(&rgba[src..src + BYTES_PER_PIXEL]);
            }
        }
        Ok(())
    }

    /// Fill a clipped rectangle.
    pub(super) fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: libghostty_vt::style::RgbColor,
    ) {
        let Some((x0, y0, x1, y1)) = self.clipped_rect(x as i32, y as i32, width, height) else {
            return;
        };
        for yy in y0..y1 {
            for xx in x0..x1 {
                let offset = self.offset(xx, yy);
                self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&[
                    color.r,
                    color.g,
                    color.b,
                    OPAQUE_ALPHA,
                ]);
            }
        }
    }

    /// Fill a clipped rectangle with alpha blending.
    ///
    /// Alpha fills are composited onto an opaque frame instead of introducing transparent output
    /// pixels, because the media encoders currently treat rendered frames as opaque RGBA.
    fn fill_rect_alpha(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: libghostty_vt::style::RgbColor,
        alpha: u8,
    ) {
        let Some((x0, y0, x1, y1)) = self.clipped_rect(x as i32, y as i32, width, height) else {
            return;
        };
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.blend_pixel(xx, yy, color.r, color.g, color.b, alpha);
            }
        }
    }

    /// Fill a clipped rounded rectangle with alpha blending.
    ///
    /// Rounded corners are computed by skipping pixels outside corner circles; interior pixels
    /// receive normal alpha blending.
    pub(super) fn fill_rounded_rect_alpha(
        &mut self,
        rect: PixelRect,
        color: libghostty_vt::style::RgbColor,
        alpha: u8,
        radius: u32,
    ) {
        let Some((x0, y0, x1, y1)) =
            self.clipped_rect(rect.x as i32, rect.y as i32, rect.width, rect.height)
        else {
            return;
        };
        let radius = radius.min(rect.width / 2).min(rect.height / 2) as i32;
        if radius <= 0 {
            self.fill_rect_alpha(rect.x, rect.y, rect.width, rect.height, color, alpha);
            return;
        }
        for yy in y0..y1 {
            let top_corner = yy < rect.y.saturating_add(radius as u32);
            let bottom_corner = yy >= y1.saturating_sub(radius as u32);
            for xx in x0..x1 {
                let left_corner = xx < rect.x.saturating_add(radius as u32);
                let right_corner = xx >= x1.saturating_sub(radius as u32);
                if (left_corner || right_corner) && (top_corner || bottom_corner) {
                    let cx = if left_corner {
                        rect.x as i32 + radius
                    } else {
                        x1 as i32 - radius - 1
                    };
                    let cy = if top_corner {
                        rect.y as i32 + radius
                    } else {
                        y1 as i32 - radius - 1
                    };
                    let coverage = rounded_corner_coverage(
                        xx,
                        yy,
                        cx,
                        cy,
                        radius as f32,
                        ROUNDED_RECT_AA_SAMPLES,
                    );
                    if coverage <= 0.0 {
                        continue;
                    }
                    let corner_alpha =
                        (f32::from(alpha) * coverage).round().clamp(0.0, 255.0) as u8;
                    self.blend_pixel(xx, yy, color.r, color.g, color.b, corner_alpha);
                    continue;
                }
                self.blend_pixel(xx, yy, color.r, color.g, color.b, alpha);
            }
        }
    }

    /// Alpha-blend a glyph rectangle into the frame.
    ///
    /// `cosmic_text` reports glyph rectangles in signed coordinates relative to the text origin.
    /// Clipping here lets shaped glyphs extend beyond the overlay bounds without panicking.
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

    /// Alpha-blend a glyph rectangle clipped to a caller-provided rectangle.
    pub(super) fn blend_rect_clipped(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: Color,
        clip: PixelRect,
    ) {
        let [r, g, b, a] = color.as_rgba();
        if a == 0 {
            return;
        }
        let Some((x0, y0, x1, y1)) = self.clipped_rect(x, y, width, height) else {
            return;
        };
        let Some((clip_x0, clip_y0, clip_x1, clip_y1)) =
            self.clipped_rect(clip.x as i32, clip.y as i32, clip.width, clip.height)
        else {
            return;
        };
        let x0 = x0.max(clip_x0);
        let y0 = y0.max(clip_y0);
        let x1 = x1.min(clip_x1);
        let y1 = y1.min(clip_y1);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.blend_pixel(xx, yy, r, g, b, a);
            }
        }
    }

    /// Fill a clipped circle using integer distance checks.
    pub(super) fn fill_circle(
        &mut self,
        cx: u32,
        cy: u32,
        radius: u32,
        color: libghostty_vt::style::RgbColor,
    ) {
        let radius = radius as i32;
        for y in -radius..=radius {
            for x in -radius..=radius {
                if x * x + y * y > radius * radius {
                    continue;
                }
                let px = cx as i32 + x;
                let py = cy as i32 + y;
                if px < 0 || py < 0 || px >= self.width as i32 || py >= self.height as i32 {
                    continue;
                }
                let offset = self.offset(px as u32, py as u32);
                self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&[
                    color.r,
                    color.g,
                    color.b,
                    OPAQUE_ALPHA,
                ]);
            }
        }
    }

    /// Restore the backdrop outside a rounded rectangle and blend its anti-aliased edge.
    ///
    /// This is a visual mask, not an alpha mask, because current output formats are opaque.
    pub(super) fn apply_rounded_rect_mask(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
        backdrop: &Canvas,
    ) {
        let x1 = x.saturating_add(width).min(self.width);
        let y1 = y.saturating_add(height).min(self.height);
        let radius = radius.min(width / 2).min(height / 2) as i32;
        for yy in y..y1 {
            for xx in x..x1 {
                let left = xx < x + radius as u32;
                let right = xx >= x1.saturating_sub(radius as u32);
                let top = yy < y + radius as u32;
                let bottom = yy >= y1.saturating_sub(radius as u32);
                if !(left || right) || !(top || bottom) {
                    continue;
                }
                let cx = if left {
                    x as i32 + radius
                } else {
                    x1 as i32 - radius - 1
                };
                let cy = if top {
                    y as i32 + radius
                } else {
                    y1 as i32 - radius - 1
                };
                let coverage =
                    rounded_corner_coverage(xx, yy, cx, cy, radius as f32, ROUNDED_RECT_AA_SAMPLES);
                let backdrop_offset = backdrop.offset(xx, yy);
                let fill = &backdrop.pixels[backdrop_offset..backdrop_offset + BYTES_PER_PIXEL];
                if coverage <= 0.0 {
                    let offset = self.offset(xx, yy);
                    self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(fill);
                } else if coverage < 1.0 {
                    let alpha = ((1.0 - coverage) * f32::from(OPAQUE_ALPHA))
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    self.blend_pixel(xx, yy, fill[0], fill[1], fill[2], alpha);
                }
            }
        }
    }

    /// Return the byte offset for a pixel in the tightly packed RGBA buffer.
    fn offset(&self, x: u32, y: u32) -> usize {
        ((y as usize * self.width as usize) + x as usize) * BYTES_PER_PIXEL
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

    /// Alpha-blend one pixel over the existing frame color.
    pub(super) fn blend_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        let offset = self.offset(x, y);
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

    /// Convert the solid frame into the shared media frame type.
    pub(super) fn into_frame(self) -> Frame {
        Frame {
            width: self.width,
            height: self.height,
            stride: self.width as usize * BYTES_PER_PIXEL,
            format: PixelFormat::Rgba8,
            pixels: self.pixels,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PixelRect {
    /// Left edge in output pixels.
    pub(super) x: u32,
    /// Top edge in output pixels.
    pub(super) y: u32,
    /// Width in output pixels.
    pub(super) width: u32,
    /// Height in output pixels.
    pub(super) height: u32,
}

/// Number of bytes in one RGBA pixel.
const BYTES_PER_PIXEL: usize = 4;

/// Fully opaque alpha byte.
const OPAQUE_ALPHA: u8 = 0xff;

/// Denominator for 8-bit alpha blending.
const ALPHA_DENOMINATOR: u16 = 255;

/// Sub-pixel samples per axis for anti-aliased rounded-rectangle edges.
const ROUNDED_RECT_AA_SAMPLES: u32 = 4;

/// Construct an RGB color.
pub(super) const fn rgb(r: u8, g: u8, b: u8) -> libghostty_vt::style::RgbColor {
    libghostty_vt::style::RgbColor { r, g, b }
}

/// Estimate how much of one pixel is inside a rounded-rectangle corner circle.
fn rounded_corner_coverage(x: u32, y: u32, cx: i32, cy: i32, radius: f32, samples: u32) -> f32 {
    if samples == 0 {
        return 0.0;
    }
    let sample_step = 1.0 / samples as f32;
    let radius_sq = radius * radius;
    let mut covered = 0;
    for sample_y in 0..samples {
        for sample_x in 0..samples {
            let px = x as f32 + (sample_x as f32 + 0.5) * sample_step;
            let py = y as f32 + (sample_y as f32 + 0.5) * sample_step;
            let dx = px - cx as f32;
            let dy = py - cy as f32;
            if dx * dx + dy * dy <= radius_sq {
                covered += 1;
            }
        }
    }
    covered as f32 / (samples * samples) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(frame: &Canvas, x: u32, y: u32) -> [u8; 4] {
        let offset = frame.offset(x, y);
        [
            frame.pixels[offset],
            frame.pixels[offset + 1],
            frame.pixels[offset + 2],
            frame.pixels[offset + 3],
        ]
    }

    #[test]
    fn caption_glyph_blending_is_clipped_to_the_caption_rect() {
        let mut frame = Canvas::new(10, 10, rgb(0, 0, 0)).unwrap();

        frame.blend_rect_clipped(
            0,
            0,
            10,
            10,
            Color::rgb(255, 0, 0),
            PixelRect {
                x: 2,
                y: 3,
                width: 4,
                height: 2,
            },
        );

        assert_eq!(pixel(&frame, 1, 3), [0, 0, 0, 255]);
        assert_ne!(pixel(&frame, 2, 3), [0, 0, 0, 255]);
        assert_ne!(pixel(&frame, 5, 4), [0, 0, 0, 255]);
        assert_eq!(pixel(&frame, 6, 4), [0, 0, 0, 255]);
        assert_eq!(pixel(&frame, 2, 5), [0, 0, 0, 255]);
    }
}
