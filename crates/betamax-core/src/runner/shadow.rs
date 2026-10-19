//! Window drop-shadow configuration, numeric interpretation, and static backdrop painting.

use miette::miette;

use super::canvas::{Canvas, PixelRect};
use super::window_shape::WindowShape;
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Shadow {
    pub(super) enabled: bool,
    pub(super) blur: u32,
    pub(super) offset: i32,
    pub(super) opacity: f64,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            enabled: false,
            blur: 28,
            offset: 9,
            opacity: 0.55,
        }
    }
}

impl Shadow {
    /// Interpret the CSS blur radius while bounding startup work.
    pub(super) fn set_blur(&mut self, blur: f64) -> Result<()> {
        if !blur.is_finite() || blur < 0.0 {
            return Err(miette!("Set ShadowBlur expects a non-negative finite number").into());
        }
        self.blur = blur.min(256.0) as u32;
        Ok(())
    }

    /// Interpret the signed vertical displacement.
    pub(super) fn set_offset(&mut self, offset: f64) -> Result<()> {
        if !offset.is_finite() {
            return Err(miette!("Set ShadowOffset expects a finite number").into());
        }
        self.offset = offset as i32;
        Ok(())
    }

    /// Interpret opacity with the supported inclusive range.
    pub(super) fn set_opacity(&mut self, opacity: f64) -> Result<()> {
        if !opacity.is_finite() {
            return Err(miette!("Set ShadowOpacity expects a finite number").into());
        }
        self.opacity = opacity.clamp(0.0, 1.0);
        Ok(())
    }

    /// Prepare static window effects on the cached backdrop before terminal pixels are drawn.
    pub(super) fn prepare(&self, backdrop: &mut Canvas, bounds: PixelRect, radius: u32) {
        if self.enabled && bounds.x > 0 && self.opacity > 0.0 {
            self.paint_rounded_shadow(backdrop, bounds, radius);
        }
    }

    /// Paint a black, offset rounded-window shadow into this opaque backdrop.
    fn paint_rounded_shadow(&self, backdrop: &mut Canvas, rect: PixelRect, radius: u32) {
        let width = f64::from(rect.width);
        let height = f64::from(rect.height);
        if width == 0.0 || height == 0.0 {
            return;
        }
        let outline = WindowShape::new(rect, radius).shadow_outline();
        for y in 0..backdrop.height {
            for x in 0..backdrop.width {
                let distance =
                    outline.distance(x as f32 + 0.5, y as f32 + 0.5 - self.offset as f32);
                let alpha =
                    (shadow_coverage(distance, self.blur) * self.opacity * 255.0).round() as u8;
                backdrop.blend_pixel(x, y, 0, 0, 0, alpha);
            }
        }
    }
}

/// Approximate a Gaussian-blurred edge using the normal CDF.
///
/// CSS defines a shadow's standard deviation as half its blur radius. This analytic form is
/// evaluated once per run for a static decoration, avoiding a per-frame blur buffer.
fn shadow_coverage(distance: f64, blur: u32) -> f64 {
    if blur == 0 {
        return (0.5 - distance).clamp(0.0, 1.0);
    }
    let sigma = f64::from(blur) / 2.0;
    let z = distance / sigma;
    0.5 * (1.0 - (0.797_884_560_8 * (z + 0.044_715 * z.powi(3))).tanh())
}

#[cfg(test)]
mod tests {
    use crate::media::Frame;
    use crate::runner::canvas::{rgb, Canvas};
    use crate::runner::settings::Settings;
    use crate::tape::Tape;
    fn shadow_settings(extra: &str) -> Settings {
        let source = format!(
            r##"Set Width 160
Set Height 140
Set Padding 0
Set Margin 30
Set MarginFill "#c8c8c8"
Set BorderRadius 12
{extra}"##
        );
        Settings::from_tape(&Tape::parse(&format!("Set Style VHS\n{source}")).unwrap()).unwrap()
    }

    fn decorated(settings: &Settings) -> Frame {
        let raw = Canvas::new(
            settings.presentation.terminal_canvas_width(),
            settings.presentation.terminal_canvas_height(),
            rgb(240, 240, 240),
        )
        .unwrap()
        .into_frame();
        settings
            .presentation
            .decorate_frame_with_overlays(&raw, None, &[])
            .unwrap()
    }

    fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
        let offset = (y as usize * frame.width as usize + x as usize) * 4;
        [
            frame.pixels[offset],
            frame.pixels[offset + 1],
            frame.pixels[offset + 2],
            frame.pixels[offset + 3],
        ]
    }

    #[test]
    fn shadow_defaults_and_explicit_values_parse() {
        let defaults = shadow_settings("");
        assert!(!defaults.presentation.style.shadow.enabled);
        assert_eq!(defaults.presentation.style.shadow.blur, 28);
        assert_eq!(defaults.presentation.style.shadow.offset, 9);
        assert_eq!(defaults.presentation.style.shadow.opacity, 0.55);

        let configured = shadow_settings(
            "Set Shadow true\nSet ShadowBlur 20\nSet ShadowOffset -7\nSet ShadowOpacity 0.4",
        );
        assert!(configured.presentation.style.shadow.enabled);
        assert_eq!(configured.presentation.style.shadow.blur, 20);
        assert_eq!(configured.presentation.style.shadow.offset, -7);
        assert_eq!(configured.presentation.style.shadow.opacity, 0.4);
    }

    #[test]
    fn shadow_rejects_invalid_and_wrong_type_values() {
        for (source, expected) in [
            ("Set ShadowBlur -1", "non-negative finite number"),
            ("Set ShadowBlur NaN", "non-negative finite number"),
            ("Set ShadowOpacity inf", "finite number"),
            ("Set ShadowOffset NaN", "finite number"),
            ("Set Shadow 1", "Set Shadow expects bool, got number"),
            (
                "Set ShadowBlur true",
                "Set ShadowBlur expects number, got bool",
            ),
        ] {
            let tape = Tape::parse(&format!("Set Style VHS\n{source}")).unwrap();
            let error = Settings::from_tape(&tape).unwrap_err().to_string();
            assert!(error.contains(expected), "{source}: {error}");
        }
    }

    #[test]
    fn shadow_darkens_the_gradient_backdrop_and_preserves_opaque_output() {
        let shadowed = shadow_settings(
            "Set Shadow true\nSet MarginFill \"linear(to: bottom, #5060a0, #c0d0f0)\"",
        );
        let plain = shadow_settings("Set MarginFill \"linear(to: bottom, #5060a0, #c0d0f0)\"");
        let shadowed_frame = decorated(&shadowed);
        let plain_frame = decorated(&plain);

        assert!(pixel(&shadowed_frame, 80, 119)[0] < pixel(&plain_frame, 80, 119)[0]);
        assert!(shadowed_frame
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255));
    }
}
