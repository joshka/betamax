//! Inset window-border settings, automatic contrast, coverage, and compositing policy.

use miette::miette;

use super::canvas::{rgb, Canvas};
use super::margin::parse_hex_color;
use super::window_shape::WindowOutline;
use crate::Result;

/// Optional inset outline; width zero preserves the original rounded-mask path.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Border {
    /// Inset width in integer pixels; zero disables the outline.
    pub(super) width: u32,
    /// Explicit color override; None derives neutral contrast from the final theme background.
    pub(super) color: Option<libghostty_vt::style::RgbColor>,
    /// Strength independent of rounded-edge coverage, clamped to 0–1.
    pub(super) opacity: f64,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            width: 0,
            color: None,
            opacity: 0.15,
        }
    }
}

impl Border {
    pub(super) fn set_width(&mut self, width: f64) -> Result<()> {
        if !width.is_finite() || width < 0.0 {
            return Err(miette!("Set WindowBorder expects a non-negative finite number").into());
        }
        self.width = width as u32;
        Ok(())
    }

    pub(super) fn set_color(&mut self, color: &str) -> Result<()> {
        self.color = if color.trim() == "auto" {
            None
        } else {
            Some(
                parse_hex_color(color)
                    .ok_or_else(|| miette!("Set WindowBorderColor expects auto or #rrggbb"))?,
            )
        };
        Ok(())
    }

    pub(super) fn set_opacity(&mut self, opacity: f64) -> Result<()> {
        if !opacity.is_finite() {
            return Err(miette!("Set WindowBorderOpacity expects a finite number").into());
        }
        self.opacity = opacity.clamp(0.0, 1.0);
        Ok(())
    }

    pub(super) fn visible(&self) -> bool {
        self.width > 0 && self.opacity > 0.0
    }

    /// Composite an inset ring and the rounded window in one coverage pass.
    ///
    /// Inner window, border, and backdrop weights sum to one. Applying a second rounded mask
    /// after drawing the ring would attenuate corner coverage twice and leave a dark fringe.
    pub(super) fn draw(
        &self,
        output: &mut Canvas,
        outline: &WindowOutline,
        backdrop: &Canvas,
        background: libghostty_vt::style::RgbColor,
    ) {
        let color = self
            .color
            .unwrap_or_else(|| contrasting_border_color(background));
        let rect = outline.rect;
        for y in rect.y..rect.y.saturating_add(rect.height).min(output.height) {
            for x in rect.x..rect.x.saturating_add(rect.width).min(output.width) {
                let outer = outline.outer.coverage(x, y);
                let inner = outline.inner.coverage(x, y);
                let border = (outer - inner).max(0.0) * self.opacity;
                if outer == 1.0 && border == 0.0 {
                    continue;
                }
                output.composite_pixel(x, y, color, border, backdrop, 1.0 - outer);
            }
        }
    }
}

/// Choose neutral contrast using linear-sRGB relative luminance and the WCAG contrast crossover.
fn contrasting_border_color(
    color: libghostty_vt::style::RgbColor,
) -> libghostty_vt::style::RgbColor {
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b);
    if luminance > 0.179 {
        rgb(0, 0, 0)
    } else {
        rgb(255, 255, 255)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::canvas::PixelRect;
    use crate::runner::settings::Settings;
    use crate::tape::Tape;

    fn pixel(frame: &Canvas, x: u32, y: u32) -> [u8; 4] {
        let frame = frame.clone().into_frame();
        let offset = (y as usize * frame.width as usize + x as usize) * 4;
        frame.pixels[offset..offset + 4].try_into().unwrap()
    }

    #[test]
    fn window_border_blends_inset_without_changing_interior_or_backdrop() {
        let backdrop = Canvas::new(12, 12, rgb(30, 40, 50)).unwrap();
        let mut frame = backdrop.clone();
        frame.fill_rect(2, 2, 8, 8, rgb(20, 20, 20));
        Border {
            width: 1,
            color: Some(rgb(255, 255, 255)),
            opacity: 0.15,
        }
        .draw(
            &mut frame,
            &WindowOutline::new(
                PixelRect {
                    x: 2,
                    y: 2,
                    width: 8,
                    height: 8,
                },
                0,
                1,
            )
            .unwrap(),
            &backdrop,
            rgb(20, 20, 20),
        );
        assert_eq!(pixel(&frame, 5, 9), [55, 55, 55, 255]);
        assert_eq!(pixel(&frame, 5, 8), [20, 20, 20, 255]);
        assert_eq!(pixel(&frame, 5, 10), [30, 40, 50, 255]);
        assert_eq!(
            contrasting_border_color(rgb(21, 23, 35)),
            rgb(255, 255, 255)
        );
        assert_eq!(contrasting_border_color(rgb(240, 240, 240)), rgb(0, 0, 0));
    }

    #[test]
    fn window_border_rounded_coverage_matches_existing_mask() {
        let backdrop = Canvas::new(12, 12, rgb(90, 70, 50)).unwrap();
        let mut original = backdrop.clone();
        original.fill_rect(1, 1, 10, 10, rgb(20, 20, 20));
        let mut masked = original.clone();
        masked.apply_rounded_rect_mask(1, 1, 10, 10, 4, &backdrop);
        Border {
            width: 1,
            color: Some(rgb(255, 255, 255)),
            opacity: 0.0,
        }
        .draw(
            &mut original,
            &WindowOutline::new(
                PixelRect {
                    x: 1,
                    y: 1,
                    width: 10,
                    height: 10,
                },
                4,
                1,
            )
            .unwrap(),
            &backdrop,
            rgb(20, 20, 20),
        );
        for (a, b) in original
            .clone()
            .into_frame()
            .pixels
            .iter()
            .zip(&masked.into_frame().pixels)
        {
            assert!((i16::from(*a) - i16::from(*b)).abs() <= 1);
        }
        Border {
            width: u32::MAX,
            color: Some(rgb(255, 255, 255)),
            opacity: 1.0,
        }
        .draw(
            &mut original,
            &WindowOutline::new(
                PixelRect {
                    x: 1,
                    y: 1,
                    width: 1,
                    height: 1,
                },
                50,
                1,
            )
            .unwrap(),
            &backdrop,
            rgb(20, 20, 20),
        );
        assert_eq!(pixel(&original, 1, 1), [255, 255, 255, 255]);
    }

    #[test]
    fn window_border_settings_preserve_geometry_and_theme_overrides() {
        let plain =
            Settings::from_tape(&Tape::parse("Set Width 160\nSet Height 140").unwrap()).unwrap();
        let border = Settings::from_tape(&Tape::parse(
            "Set Width 160\nSet Height 140\nSet WindowBorder 1.9\nSet WindowBorderOpacity 2\nSet WindowBorderColor \"#123456\"\nSet Theme \"Aardvark Blue\""
        ).unwrap()).unwrap();
        assert_eq!(border.presentation.style.border.width, 1);
        assert_eq!(border.presentation.style.border.opacity, 1.0);
        assert_eq!(
            border.presentation.style.border.color,
            parse_hex_color("#123456")
        );
        assert_eq!((plain.columns, plain.rows), (border.columns, border.rows));
        assert_eq!(
            (
                plain.presentation.terminal_canvas_width(),
                plain.presentation.terminal_canvas_height()
            ),
            (
                border.presentation.terminal_canvas_width(),
                border.presentation.terminal_canvas_height()
            )
        );
        let auto = Settings::from_tape(
            &Tape::parse("Set WindowBorderColor \"#123456\"\nSet WindowBorderColor auto").unwrap(),
        )
        .unwrap();
        assert!(auto.presentation.style.border.color.is_none());
        for source in [
            "Set WindowBorder -1",
            "Set WindowBorder true",
            "Set WindowBorderColor nope",
            "Set WindowBorderOpacity true",
        ] {
            assert!(
                Settings::from_tape(&Tape::parse(source).unwrap()).is_err(),
                "{source}"
            );
        }
    }
}
