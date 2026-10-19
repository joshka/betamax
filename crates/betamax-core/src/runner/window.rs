//! Window chrome configuration, dimensions, button styling, and painting.

use super::canvas::{rgb, Canvas, PixelRect};
use super::margin::parse_hex_color;
use super::shadow::Shadow;
use crate::ghostty::TerminalTheme;
use crate::media::Frame;

/// Default synthetic window bar height in pixels.
const DEFAULT_WINDOW_BAR_SIZE: u32 = 30;

/// Window-button radius divisor relative to bar height.
const WINDOW_BUTTON_RADIUS_DIVISOR: u32 = 6;

/// Minimum window-button radius in pixels.
const MIN_WINDOW_BUTTON_RADIUS: u32 = 2;

/// Spacing between synthetic window buttons as a multiple of radius.
const WINDOW_BUTTON_GAP_RADIUS_MULTIPLIER: u32 = 3;

/// VHS-style frame decoration around the raw terminal capture.
///
/// Margin and bar dimensions reserve space before the PTY grid is derived. Painting happens after
/// raw terminal capture, so screenshots, GIFs, videos, and PNG sequences share the same chrome.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct WindowStyle {
    /// Outer margin around the captured terminal, in pixels.
    pub(super) margin: u32,
    /// Explicit solid color or linear gradient. `None` follows the final selected theme.
    pub(super) margin_fill: Option<String>,
    /// Window button mode. Empty disables the bar; non-empty draws traffic-light buttons.
    pub(super) window_bar: String,
    /// Height of the synthetic window bar when enabled.
    pub(super) window_bar_size: u32,
    /// Window bar fill color as `#RRGGBB`; invalid values fall back to the theme background.
    pub(super) window_bar_color: String,
    /// Radius for masking the captured terminal plus window bar.
    pub(super) border_radius: u32,
    /// Static shadow configuration and painting policy.
    pub(super) shadow: Shadow,
}

impl WindowStyle {
    /// Construct style defaults from the current theme.
    ///
    /// The window-bar background is stored as raw text because a tape can later replace it with a
    /// user-provided color. An unset margin fill remains distinct so it can follow the final theme.
    pub(super) fn new(theme: &TerminalTheme) -> Self {
        let background = theme.background_hex();
        Self {
            margin: 0,
            margin_fill: None,
            window_bar: String::new(),
            window_bar_size: DEFAULT_WINDOW_BAR_SIZE,
            window_bar_color: background,
            border_radius: 0,
            shadow: Shadow::default(),
        }
    }

    /// Prepare static window effects before terminal pixels are drawn.
    pub(super) fn prepare_backdrop(&self, backdrop: &mut Canvas, bounds: PixelRect) {
        self.shadow.prepare(backdrop, bounds, self.border_radius);
    }

    /// VHS only applies margin when a margin fill is configured.
    pub(super) fn effective_margin(&self) -> u32 {
        if self.margin_fill.as_deref() == Some("") {
            0
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

    /// Paint chrome over the terminal, then restore rounded corners from the backdrop.
    pub(super) fn draw(
        &self,
        output: &mut Canvas,
        terminal: &Frame,
        backdrop: &Canvas,
        fallback: libghostty_vt::style::RgbColor,
    ) {
        let margin = self.effective_margin();
        let bar_height = self.window_bar_height();
        let bar = parse_hex_color(&self.window_bar_color).unwrap_or(fallback);
        if bar_height > 0 {
            output.fill_rect(margin, margin, terminal.width, bar_height, bar);
            self.draw_window_buttons(output);
        }
        if self.border_radius > 0 {
            output.apply_rounded_rect_mask(
                margin,
                margin,
                terminal.width,
                terminal.height + bar_height,
                self.border_radius,
                backdrop,
            );
        }
    }

    /// Draw macOS-style window buttons.
    ///
    /// The current style language treats any non-empty mode as enabled. Modes ending in `Right`
    /// place the buttons on the right; all other values place them on the left.
    fn draw_window_buttons(&self, output: &mut Canvas) {
        let bar_height = self.window_bar_height();
        let cx = self.effective_margin() + bar_height / 2;
        let cy = cx;
        let radius = (bar_height / WINDOW_BUTTON_RADIUS_DIVISOR).max(MIN_WINDOW_BUTTON_RADIUS);
        let gap = radius * WINDOW_BUTTON_GAP_RADIUS_MULTIPLIER;
        let right = self.window_bar.ends_with("Right");
        let start_x = if right {
            output.width.saturating_sub(cx + gap * 2)
        } else {
            cx
        };
        let colors = [
            rgb(0xff, 0x5f, 0x57),
            rgb(0xff, 0xbd, 0x2e),
            rgb(0x28, 0xc8, 0x40),
        ];
        for (index, color) in colors.into_iter().enumerate() {
            output.fill_circle(start_x + gap * index as u32, cy, radius, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::runner::settings::Settings;
    use crate::tape::Tape;

    #[test]
    fn parses_frame_style_settings() {
        let tape = Tape::parse(
            r##"
            Set Margin 12
            Set MarginFill "#674EFF"
            Set WindowBar Colorful
            Set WindowBarSize 40
            Set BorderRadius 8
            "##,
        )
        .unwrap();

        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.presentation.style.margin, 12);
        assert_eq!(
            settings.presentation.style.margin_fill.as_deref(),
            Some("#674EFF")
        );
        assert_eq!(settings.presentation.style.window_bar, "Colorful");
        assert_eq!(settings.presentation.style.window_bar_size, 40);
        assert_eq!(settings.presentation.style.border_radius, 8);
    }
}
