//! Owned RGBA canvas for output-frame decoration.
//!
//! Blitting, clipping, blending, and geometric masks share one straight-alpha pixel
//! representation. Pixels are opaque unless the margin fill is transparent.
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
    /// Allocate a straight-alpha decoration layer, composited over opaque output later.
    pub(super) fn transparent(width: u32, height: u32) -> Result<Self> {
        let mut layer = Self::new(width, height, rgb(0, 0, 0))?;
        layer.pixels.fill(0);
        Ok(layer)
    }

    /// Composite a cached straight-alpha decoration layer in output coordinates.
    pub(super) fn composite_overlay(&mut self, overlay: &Self) {
        for y in 0..overlay.height.min(self.height) {
            for x in 0..overlay.width.min(self.width) {
                let offset = overlay.offset(x, y);
                let [r, g, b, a] = overlay.pixels[offset..offset + 4].try_into().unwrap();
                if a == 0 {
                    continue;
                }
                if a == OPAQUE_ALPHA {
                    self.set_pixel(x, y, rgb(r, g, b));
                } else {
                    self.blend_pixel(x, y, r, g, b, a);
                }
            }
        }
    }

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
    /// Alpha fills are composited onto the existing pixel; they never make output more transparent.
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

    /// Fill a circle with local path coverage, preserving the integer pixel-center lattice.
    ///
    /// Window buttons are prepared once in the cached bar. The half-pixel offset keeps their
    /// center on the same pixel as the former integer-distance circle without widening the edge.
    pub(super) fn fill_circle(
        &mut self,
        cx: u32,
        cy: u32,
        radius: u32,
        color: libghostty_vt::style::RgbColor,
    ) -> Result<()> {
        if radius == 0 {
            return Ok(());
        }
        let size = radius
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| miette!("circle is too large"))?;
        let center = radius as f32 + 0.5;
        // Cubics avoid radius-dependent conic flattening before the local coverage transform.
        let r = radius as f32;
        let k = r * 0.552_284_8;
        let mut builder = tiny_skia::PathBuilder::new();
        builder.move_to(center + r, center);
        builder.cubic_to(
            center + r,
            center + k,
            center + k,
            center + r,
            center,
            center + r,
        );
        builder.cubic_to(
            center - k,
            center + r,
            center - r,
            center + k,
            center - r,
            center,
        );
        builder.cubic_to(
            center - r,
            center - k,
            center - k,
            center - r,
            center,
            center - r,
        );
        builder.cubic_to(
            center + k,
            center - r,
            center + r,
            center - k,
            center + r,
            center,
        );
        builder.close();
        let path = builder.finish().expect("nonempty circle");
        let coverage = path_coverage(&path, size, size)?;
        for y in 0..size {
            for x in 0..size {
                let px = i64::from(cx) + i64::from(x) - i64::from(radius);
                let py = i64::from(cy) + i64::from(y) - i64::from(radius);
                if px < 0 || py < 0 || px >= i64::from(self.width) || py >= i64::from(self.height) {
                    continue;
                }
                // Reflect the same coverage at each quadrant, independent of scan direction.
                let mut sum = 0u16;
                for (mx, my) in [
                    (x, y),
                    (size - 1 - x, y),
                    (x, size - 1 - y),
                    (size - 1 - x, size - 1 - y),
                ] {
                    sum += u16::from(coverage[(my * size + mx) as usize]);
                }
                let alpha = ((sum + 2) / 4) as u8;
                if alpha > 0 {
                    self.blend_pixel(px as u32, py as u32, color.r, color.g, color.b, alpha);
                }
            }
        }
        Ok(())
    }

    /// Apply prepared continuous coverage once against the exact shadowed backdrop sample.
    pub(super) fn apply_window_mask(
        &mut self,
        mask: &super::window_shape::WindowMask,
        rect: PixelRect,
        backdrop: &Canvas,
    ) {
        for y in rect.y..rect.y.saturating_add(rect.height).min(self.height) {
            for x in rect.x..rect.x.saturating_add(rect.width).min(self.width) {
                let outside = 1.0 - mask.coverage(x, y);
                if outside > 0.0 {
                    self.composite_pixel(x, y, rgb(0, 0, 0), 0.0, backdrop, outside);
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn apply_rounded_rect_mask(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
        backdrop: &Canvas,
    ) {
        let rect = PixelRect {
            x,
            y,
            width,
            height,
        };
        let mask = super::window_shape::WindowShape::new(rect, radius)
            .mask()
            .unwrap();
        self.apply_window_mask(&mask, rect, backdrop);
    }

    /// Mix an existing pixel with a solid color and a same-coordinate backdrop sample.
    /// The remaining weight belongs to the existing pixel; the solid color is opaque.
    pub(super) fn composite_pixel(
        &mut self,
        x: u32,
        y: u32,
        color: libghostty_vt::style::RgbColor,
        color_weight: f64,
        backdrop: &Canvas,
        backdrop_weight: f64,
    ) {
        let offset = self.offset(x, y);
        let background = backdrop.offset(x, y);
        let existing_alpha = self.pixels[offset + 3];
        let backdrop_alpha = backdrop.pixels[background + 3];
        if existing_alpha < OPAQUE_ALPHA || backdrop_alpha < OPAQUE_ALPHA {
            // Mix premultiplied samples so a clear backdrop fades coverage rather than color.
            let existing_weight =
                (1.0 - color_weight - backdrop_weight) * f64::from(existing_alpha) / 255.0;
            let backdrop_weight = backdrop_weight * f64::from(backdrop_alpha) / 255.0;
            let alpha = existing_weight + color_weight + backdrop_weight;
            for (channel, value) in [color.r, color.g, color.b].into_iter().enumerate() {
                let premultiplied = f64::from(self.pixels[offset + channel]) * existing_weight
                    + f64::from(value) * color_weight
                    + f64::from(backdrop.pixels[background + channel]) * backdrop_weight;
                self.pixels[offset + channel] = if alpha > 0.0 {
                    (premultiplied / alpha).round().clamp(0.0, 255.0) as u8
                } else {
                    0
                };
            }
            self.pixels[offset + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
            return;
        }
        for (channel, value) in [color.r, color.g, color.b].into_iter().enumerate() {
            self.pixels[offset + channel] = (f64::from(self.pixels[offset + channel])
                * (1.0 - color_weight - backdrop_weight)
                + f64::from(value) * color_weight
                + f64::from(backdrop.pixels[background + channel]) * backdrop_weight)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        self.pixels[offset + 3] = OPAQUE_ALPHA;
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
        if a == 0 {
            return;
        }
        if self.pixels[offset + 3] < OPAQUE_ALPHA {
            let source = u32::from(a);
            let target = u32::from(self.pixels[offset + 3]);
            let weight = source * 255 + target * (255 - source);
            for (channel, value) in [r, g, b].into_iter().enumerate() {
                self.pixels[offset + channel] = ((u32::from(value) * source * 255
                    + u32::from(self.pixels[offset + channel]) * target * (255 - source))
                    / weight) as u8;
            }
            self.pixels[offset + 3] = ((weight + 127) / 255) as u8;
            return;
        }
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

/// Rasterize a small shape mask at local 4× density, then average pixel areas.
///
/// This improves the path rasterizer's precision on tiny curves. Only the reusable shape mask
/// grows; terminal frames and full decoration canvases never change size.
pub(super) fn path_coverage(path: &tiny_skia::Path, width: u32, height: u32) -> Result<Vec<u8>> {
    const SCALE: u32 = 4;
    let scaled_width = width
        .checked_mul(SCALE)
        .ok_or_else(|| miette!("shape is too large"))?;
    let scaled_height = height
        .checked_mul(SCALE)
        .ok_or_else(|| miette!("shape is too large"))?;
    let mut mask = tiny_skia::Mask::new(scaled_width, scaled_height)
        .ok_or_else(|| miette!("shape is too large"))?;
    mask.fill_path(
        path,
        tiny_skia::FillRule::Winding,
        true,
        tiny_skia::Transform::from_scale(SCALE as f32, SCALE as f32),
    );
    let mut coverage = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0u32;
            for sy in 0..SCALE {
                for sx in 0..SCALE {
                    let offset = (y * SCALE + sy) as usize * scaled_width as usize
                        + (x * SCALE + sx) as usize;
                    sum += u32::from(mask.data()[offset]);
                }
            }
            coverage.push(((sum + SCALE * SCALE / 2) / (SCALE * SCALE)) as u8);
        }
    }
    Ok(coverage)
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
    fn circle_edges_have_coverage_without_a_halo_or_weight_gain() {
        for radius in [2, 5, 10] {
            let size = radius * 2 + 5;
            let center = radius + 2;
            let mut frame = Canvas::new(size, size, rgb(0, 0, 0)).unwrap();
            frame
                .fill_circle(center, center, radius, rgb(255, 255, 255))
                .unwrap();
            let mut area = 0.0;
            let mut fractional = false;
            for y in 0..size {
                for x in 0..size {
                    let color = pixel(&frame, x, y);
                    area += f64::from(color[0]) / 255.0;
                    fractional |= color[0] > 0 && color[0] < 255;
                    for reflected in [
                        pixel(&frame, size - 1 - x, y),
                        pixel(&frame, x, size - 1 - y),
                    ] {
                        for (a, b) in color.into_iter().zip(reflected) {
                            assert!(a.abs_diff(b) <= 1);
                        }
                    }
                    assert_eq!(color[3], 255);
                }
            }
            assert!(fractional);
            assert!((area - std::f64::consts::PI * f64::from(radius * radius)).abs() < 1.0);
            assert_eq!(pixel(&frame, center, center), [255; 4]);
            assert_eq!(pixel(&frame, 0, center), [0, 0, 0, 255]);
        }
    }

    #[test]
    fn cached_circle_edges_composite_over_the_actual_destination() {
        let mut layer = Canvas::transparent(8, 8).unwrap();
        layer.fill_circle(1, 1, 3, rgb(240, 80, 40)).unwrap();
        assert!(layer
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0 && p[3] < 255));
        let mut cached = Canvas::new(8, 8, rgb(30, 60, 90)).unwrap();
        cached.composite_overlay(&layer);
        let mut direct = Canvas::new(8, 8, rgb(30, 60, 90)).unwrap();
        direct.fill_circle(1, 1, 3, rgb(240, 80, 40)).unwrap();
        assert_eq!(cached.pixels, direct.pixels);
        assert!(cached.pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
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
