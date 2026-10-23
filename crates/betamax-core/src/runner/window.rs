//! Window chrome configuration, dimensions, button styling, and painting.

use super::border::Border;
use super::canvas::{rgb, Canvas, PixelRect};
use super::insets::Insets;
use super::margin::parse_hex_color;
use super::shadow::Shadow;
use super::window_shape::WindowOutline;
use super::window_title::WindowTitle;
use crate::ghostty::{TerminalTheme, TextSettings};
#[cfg(test)]
use crate::media::Frame;
use crate::Result;

/// Default synthetic window bar height in pixels.
const DEFAULT_WINDOW_BAR_SIZE: u32 = 30;

/// Window-button radius divisor relative to bar height.
const WINDOW_BUTTON_RADIUS_DIVISOR: u32 = 6;

/// Minimum window-button radius in pixels.
const MIN_WINDOW_BUTTON_RADIUS: u32 = 2;

/// Spacing between synthetic window buttons as a multiple of radius.
const WINDOW_BUTTON_GAP_RADIUS_MULTIPLIER: u32 = 3;

/// Clear space between title ink and the button cluster, in button radii.
const WINDOW_TITLE_BUTTON_GAP_RADII: u32 = 2;

/// VHS-style frame decoration around the raw terminal capture.
///
/// Margin and bar dimensions reserve space before the PTY grid is derived. Painting happens after
/// raw terminal capture, so screenshots, GIFs, videos, and PNG sequences share the same chrome.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct WindowStyle {
    /// Outer margin around the captured terminal, in pixels per side.
    pub(super) margin: Insets,
    /// Explicit solid color or linear gradient. `None` follows the final selected theme.
    pub(super) margin_fill: Option<String>,
    /// Window button mode. Empty disables the bar; non-empty draws traffic-light buttons.
    pub(super) window_bar: String,
    /// Height of the synthetic window bar when enabled.
    pub(super) window_bar_size: u32,
    /// Theme-derived window bar fill color as `#RRGGBB`.
    pub(super) window_bar_color: String,
    /// Explicit bar color, preserved across subsequent theme settings.
    pub(super) window_bar_color_override: Option<String>,
    /// Static title configuration and its prepared pixels.
    pub(super) window_title: WindowTitle,
    /// Radius for masking the captured terminal plus window bar.
    pub(super) border_radius: u32,
    /// Static shadow configuration and painting policy.
    pub(super) shadow: Shadow,
    /// Inset outline configuration and drawing policy.
    pub(super) border: Border,
}

impl WindowStyle {
    /// Construct style defaults from the current theme.
    ///
    /// Explicit bar colors remain separate from the theme-derived default so setting order cannot
    /// reset them. An unset margin fill remains distinct so it can follow the final theme.
    pub(super) fn new(theme: &TerminalTheme) -> Self {
        let background = theme.background_hex();
        Self {
            margin: Insets::default(),
            margin_fill: None,
            window_bar: String::new(),
            window_bar_size: DEFAULT_WINDOW_BAR_SIZE,
            window_bar_color: background,
            window_bar_color_override: None,
            window_title: WindowTitle::default(),
            border_radius: 0,
            shadow: Shadow::default(),
            border: Border::default(),
        }
    }

    /// Prepare static window effects before terminal pixels are drawn.
    pub(super) fn prepare_backdrop(&self, backdrop: &mut Canvas, bounds: PixelRect) {
        self.shadow.prepare(backdrop, bounds, self.border_radius);
    }

    /// VHS only applies margin when a margin fill is configured.
    pub(super) fn effective_margin(&self) -> Insets {
        if self.margin_fill.as_deref() == Some("") {
            Insets::default()
        } else {
            self.margin
        }
    }

    /// Height reserved above the terminal canvas for the synthetic window bar.
    pub(super) fn window_bar_height(&self) -> u32 {
        if self.window_bar.is_empty() {
            0
        } else {
            self.window_bar_size
        }
    }

    /// Prepare bar pixels once in output coordinates, preserving short-bar button clipping.
    pub(super) fn prepare_bar(
        &self,
        width: u32,
        height: u32,
        fallback: libghostty_vt::style::RgbColor,
    ) -> Result<Canvas> {
        let margin = self.effective_margin();
        let bar_height = self.window_bar_height();
        if bar_height == 0 {
            return Canvas::transparent(0, 0);
        }
        let radius = (bar_height / WINDOW_BUTTON_RADIUS_DIVISOR).max(MIN_WINDOW_BUTTON_RADIUS);
        let height = height.min(
            margin
                .top
                .saturating_add(bar_height)
                .saturating_add(radius)
                .saturating_add(1),
        );
        let mut overlay = Canvas::transparent(width, height)?;
        let color = self.bar_color(fallback);
        overlay.fill_rect(
            margin.left,
            margin.top,
            width.saturating_sub(margin.horizontal()),
            bar_height,
            color,
        );
        self.draw_window_buttons(&mut overlay)?;
        Ok(overlay)
    }

    /// Paint cached chrome, then restore rounded corners from the backdrop.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &self,
        output: &mut Canvas,
        backdrop: &Canvas,
        bar_overlay: &Canvas,
        outline: &WindowOutline,
        theme: &TerminalTheme,
        text: &TextSettings,
        title: Option<&str>,
    ) -> Result<()> {
        let bar_height = self.window_bar_height();
        let bar = self.bar_color(theme.background);
        if bar_height > 0 {
            output.composite_overlay(bar_overlay);
            let buttons = WindowButtons::new(
                output.width,
                self.effective_margin(),
                bar_height,
                &self.window_bar,
            );
            let title_bounds = self.title_bounds(output.width, &buttons);
            self.window_title
                .draw(output, title_bounds, text, theme, bar, title)?;
        }
        if self.border.visible() {
            self.border
                .draw(output, outline, backdrop, theme.background);
        } else if self.border_radius > 0 {
            output.apply_window_mask(&outline.outer, outline.rect, backdrop);
        }
        Ok(())
    }

    /// Resolve explicit and theme-derived bar colors consistently for bar and title pixels.
    pub(super) fn bar_color(
        &self,
        fallback: libghostty_vt::style::RgbColor,
    ) -> libghostty_vt::style::RgbColor {
        let color = self
            .window_bar_color_override
            .as_deref()
            .unwrap_or(&self.window_bar_color);
        parse_hex_color(color).unwrap_or(fallback)
    }

    /// Resolve symmetric title clearance from the actual output-space button extent.
    fn title_bounds(&self, output_width: u32, buttons: &WindowButtons) -> PixelRect {
        let margin = self.effective_margin();
        let bounds = buttons.horizontal_bounds();
        let occupied_width = if self.window_bar.ends_with("Right") {
            output_width
                .saturating_sub(margin.right)
                .saturating_sub(bounds.start)
        } else {
            bounds.end.saturating_sub(margin.left)
        };
        let inset = occupied_width.saturating_add(buttons.radius * WINDOW_TITLE_BUTTON_GAP_RADII);
        PixelRect {
            x: margin.left + inset,
            y: margin.top,
            width: output_width
                .saturating_sub(margin.horizontal())
                .saturating_sub(inset.saturating_mul(2)),
            height: self.window_bar_height(),
        }
    }

    /// Draw macOS-style window buttons.
    ///
    /// The current style language treats any non-empty mode as enabled. Modes ending in `Right`
    /// place the buttons on the right; all other values place them on the left.
    fn draw_window_buttons(&self, output: &mut Canvas) -> Result<()> {
        let bar_height = self.window_bar_height();
        let buttons = WindowButtons::new(
            output.width,
            self.effective_margin(),
            bar_height,
            &self.window_bar,
        );
        let cy = buttons.cy;
        let colors = [
            rgb(0xff, 0x5f, 0x57),
            rgb(0xff, 0xbd, 0x2e),
            rgb(0x28, 0xc8, 0x40),
        ];
        for (index, color) in colors.into_iter().enumerate() {
            output.fill_circle(
                buttons.first_x + buttons.step * index as u32,
                cy,
                buttons.radius,
                color,
            )?;
        }
        Ok(())
    }
}

/// Horizontal button geometry shared by drawing and title clearance.
struct WindowButtons {
    first_x: u32,
    cy: u32,
    step: u32,
    radius: u32,
}

impl WindowButtons {
    fn new(output_width: u32, margin: Insets, bar_height: u32, mode: &str) -> Self {
        let radius = (bar_height / WINDOW_BUTTON_RADIUS_DIVISOR).max(MIN_WINDOW_BUTTON_RADIUS);
        let step = radius * WINDOW_BUTTON_GAP_RADIUS_MULTIPLIER;
        // Buttons sit half a bar height in from the window's corner on both axes.
        let edge_offset = bar_height / 2;
        let first_x = if mode.ends_with("Right") {
            output_width.saturating_sub(margin.right + edge_offset + step * 2)
        } else {
            margin.left + edge_offset
        };
        Self {
            first_x,
            cy: margin.top + edge_offset,
            step,
            radius,
        }
    }

    /// Full horizontal pixel extent, including the outermost circle pixels.
    fn horizontal_bounds(&self) -> std::ops::Range<u32> {
        let left = self.first_x.saturating_sub(self.radius);
        let right = self
            .first_x
            .saturating_add(self.step * 2)
            .saturating_add(self.radius)
            .saturating_add(1);
        left..right
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::settings::Settings;
    use crate::tape::Tape;

    fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
        let offset = (y as usize * frame.width as usize + x as usize) * 4;
        frame.pixels[offset..offset + 4].try_into().unwrap()
    }

    fn bar_settings(commands: &str) -> Settings {
        let tape = Tape::parse(&format!(
            r##"
            Set Preset Default-0.1
            Set Width 160
            Set Height 120
            Set Padding 12
            Set Margin 8
            Set MarginFill "#445566"
            Set WindowBar Rings
            Set WindowBarSize 30
            Set BorderRadius 10
            {commands}
            "##
        ))
        .unwrap();
        Settings::from_tape(&tape).unwrap()
    }

    fn decorated_bar(settings: &Settings) -> Frame {
        let raw = Canvas::new(
            settings.presentation.terminal_canvas_width(),
            settings.presentation.terminal_canvas_height(),
            settings.presentation.theme.background,
        )
        .unwrap()
        .into_frame();
        settings
            .presentation
            .decorate_frame_with_overlays(&raw, None, &[])
            .unwrap()
    }

    #[test]
    fn window_bar_color_survives_theme_ordering() {
        let theme = r##"Set Theme '{"name":"Test","background":"#151723"}'"##;
        for commands in [
            format!("Set WindowBarColor \"#aAbBcC\"\n{theme}"),
            format!("{theme}\nSet WindowBarColor \"#aAbBcC\""),
        ] {
            let settings = bar_settings(&commands);
            assert_eq!(
                settings
                    .presentation
                    .style
                    .window_bar_color_override
                    .as_deref(),
                Some("#aAbBcC")
            );
            let output = decorated_bar(&settings);
            assert_eq!(pixel(&output, 80, 20), [0xaa, 0xbb, 0xcc, 255]);
            assert_eq!(pixel(&output, 80, 38), [0x15, 0x17, 0x23, 255]);
        }
    }

    #[test]
    fn invalid_window_bar_colors_follow_the_active_theme() {
        let theme = r##"Set Theme '{"name":"Test","background":"#151723"}'"##;
        for color in ["", "red", "#123", "#12345678", "#zzzzzz", "#aéxyz"] {
            for commands in [
                format!("Set WindowBarColor \"{color}\"\n{theme}"),
                format!("{theme}\nSet WindowBarColor \"{color}\""),
            ] {
                let output = decorated_bar(&bar_settings(&commands));
                assert_eq!(
                    pixel(&output, 80, 20),
                    [0x15, 0x17, 0x23, 255],
                    "{commands}"
                );
            }
        }
    }

    #[test]
    fn window_bar_color_rejects_non_string_values() {
        for (value, kind) in [("42", "number"), ("true", "bool"), ("1s", "duration")] {
            let tape = Tape::parse(&format!(
                "Set Preset Default-0.1\nSet WindowBarColor {value}"
            ))
            .unwrap();
            let error = Settings::from_tape(&tape).unwrap_err().to_string();
            assert!(error.contains(&format!("Set WindowBarColor expects string, got {kind}")));
        }
    }

    #[test]
    fn custom_bar_rounds_only_window_corners_and_preserves_buttons() {
        for mode in ["Rings", "RingsRight"] {
            let settings = bar_settings(&format!(
                "Set WindowBar {mode}\nSet WindowBarColor \"#0f101a\""
            ));
            let output = decorated_bar(&settings);
            assert_eq!(pixel(&output, 8, 8), [0x44, 0x55, 0x66, 255]);
            assert_eq!(pixel(&output, 80, 8), [0x0f, 0x10, 0x1a, 255]);
            let edge = pixel(&output, 12, 9);
            for (actual, expected) in edge.into_iter().zip([50_u8, 61, 76, 255]) {
                assert!(actual.abs_diff(expected) <= 1, "rounded edge: {edge:?}");
            }
            // The bar/terminal seam is straight, including at both window edges.
            for x in [8, 80, 151] {
                assert_eq!(pixel(&output, x, 37), [0x0f, 0x10, 0x1a, 255]);
                assert_eq!(pixel(&output, x, 38), [0x10, 0x20, 0x40, 255]);
            }
            let first_button_x = if mode == "Rings" { 23 } else { 107 };
            for (index, expected) in [
                [0xff, 0x5f, 0x57, 255],
                [0xff, 0xbd, 0x2e, 255],
                [0x28, 0xc8, 0x40, 255],
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(
                    pixel(&output, first_button_x + index as u32 * 15, 23),
                    expected
                );
            }
            let again = decorated_bar(&settings);
            assert_eq!(output.pixels, again.pixels);
            assert!(output
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255));
        }
    }

    #[test]
    fn automatic_border_uses_theme_contrast_with_a_custom_bar() {
        let settings = bar_settings(
            r##"
            Set Theme '{"name":"Light","background":"#ffffff"}'
            Set WindowBarColor "#0f101a"
            Set WindowBorder 2
            Set WindowBorderOpacity 1
            "##,
        );
        let output = decorated_bar(&settings);

        assert_eq!(pixel(&output, 80, 8), [0, 0, 0, 255]);
        assert_eq!(pixel(&output, 80, 20), [0x0f, 0x10, 0x1a, 255]);
        assert_eq!(pixel(&output, 80, 38), [255, 255, 255, 255]);
    }

    #[test]
    fn default_window_bar_preserves_legacy_pixels() {
        for mode in ["", "Colorful", "Rings", "RingsRight", "ColorfulRight"] {
            for bar_height in [2, 30] {
                let settings = bar_settings(&format!(
                    "Set WindowBar \"{mode}\"\nSet WindowBarSize {bar_height}"
                ));
                assert!(settings
                    .presentation
                    .style
                    .window_bar_color_override
                    .is_none());
                let output = decorated_bar(&settings);
                let raw = Canvas::new(
                    settings.presentation.terminal_canvas_width(),
                    settings.presentation.terminal_canvas_height(),
                    settings.presentation.theme.background,
                )
                .unwrap()
                .into_frame();
                // Reference the decoration sequence used before WindowBarColor existed.
                let fill = rgb(0x44, 0x55, 0x66);
                let mut legacy = Canvas::new(
                    settings.presentation.width,
                    settings.presentation.height,
                    fill,
                )
                .unwrap();
                let margin = settings.presentation.style.effective_margin().top;
                let height = settings.presentation.style.window_bar_height();
                legacy.blit(&raw, margin, margin + height).unwrap();
                if height > 0 {
                    legacy.fill_rect(
                        margin,
                        margin,
                        raw.width,
                        height,
                        settings.presentation.theme.background,
                    );
                    settings
                        .presentation
                        .style
                        .draw_window_buttons(&mut legacy)
                        .unwrap();
                }
                legacy.apply_rounded_rect_mask(
                    margin,
                    margin,
                    raw.width,
                    raw.height + height,
                    10,
                    &Canvas::new(
                        settings.presentation.width,
                        settings.presentation.height,
                        fill,
                    )
                    .unwrap(),
                );
                assert_eq!(
                    output.pixels,
                    legacy.into_frame().pixels,
                    "{mode}, {bar_height}"
                );
            }
        }
    }

    #[test]
    fn window_bar_color_is_ignored_when_the_bar_is_disabled() {
        let plain = decorated_bar(&bar_settings("Set WindowBar \"\""));
        let custom = decorated_bar(&bar_settings(
            "Set WindowBar \"\"\nSet WindowBarColor \"#0f101a\"",
        ));
        assert_eq!(plain.pixels, custom.pixels);
    }

    #[test]
    fn parses_frame_style_settings() {
        let tape = Tape::parse(
            r##"
            Set Preset Default-0.1
            Set Margin 12
            Set MarginFill "#674EFF"
            Set WindowBar Colorful
            Set WindowBarSize 40
            Set BorderRadius 8
            "##,
        )
        .unwrap();

        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.presentation.style.margin, Insets::uniform(12));
        assert_eq!(
            settings.presentation.style.margin_fill.as_deref(),
            Some("#674EFF")
        );
        assert_eq!(settings.presentation.style.window_bar, "Colorful");
        assert_eq!(settings.presentation.style.window_bar_size, 40);
        assert_eq!(settings.presentation.style.border_radius, 8);
    }
}
