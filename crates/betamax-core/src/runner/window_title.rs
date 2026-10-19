//! Window-title configuration, typography, truncation, and bounded raster caching.

use std::cell::RefCell;
use std::collections::VecDeque;

use cosmic_text::Weight;

use super::canvas::{Canvas, PixelRect};
use super::caption::{CaptionLayout, CaptionRenderer, LineStyle};
use super::margin::parse_hex_color;
use crate::ghostty::{TerminalTheme, TextSettings};
use crate::media::Frame;
use crate::Result;

/// Window-title font size relative to the bar height.
const WINDOW_TITLE_FONT_SCALE: f32 = 0.4;
/// Line height relative to the title font size.
const WINDOW_TITLE_LINE_HEIGHT: f32 = 1.2;

/// Initial title configuration and cached title patches shared by every output in a run.
#[derive(Debug, Clone, Default)]
pub(super) struct WindowTitle {
    /// Initial tape text; empty starts with no title.
    pub(super) text: String,
    /// Explicit color; empty or invalid values use automatic theme-derived contrast.
    pub(super) color: String,
    /// Whether OSC title notifications update the displayed title.
    pub(super) follow_terminal: bool,
    /// Up to 32 recently used opaque title patches, with immutable run geometry/style.
    pixels: RefCell<VecDeque<(String, Option<Frame>)>>,
}

// The prepared pixels are derived state, not part of configuration equality.
impl PartialEq for WindowTitle {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.color == other.color
            && self.follow_terminal == other.follow_terminal
    }
}

impl WindowTitle {
    /// Copy the cached title patch into the space reserved by window chrome.
    pub(super) fn draw(
        &self,
        output: &mut Canvas,
        bounds: PixelRect,
        text_settings: &TextSettings,
        theme: &TerminalTheme,
        bar: libghostty_vt::style::RgbColor,
        title: Option<&str>,
    ) -> Result<()> {
        let text = title.unwrap_or(&self.text);
        let mut pixels = self.pixels.borrow_mut();
        let patch = if let Some(index) = pixels.iter().position(|(cached, _)| cached == text) {
            pixels.remove(index).expect("located cache entry")
        } else {
            (
                text.to_owned(),
                self.prepare(bounds, text_settings, theme, bar, text)?,
            )
        };
        if let Some(frame) = &patch.1 {
            output.blit(frame, bounds.x, bounds.y)?;
        }
        if pixels.len() == 32 {
            pixels.pop_front();
        }
        pixels.push_back(patch);
        Ok(())
    }

    /// Shape and rasterize one opaque title patch after all startup settings are known.
    fn prepare(
        &self,
        bounds: PixelRect,
        text_settings: &TextSettings,
        theme: &TerminalTheme,
        bar: libghostty_vt::style::RgbColor,
        title: &str,
    ) -> Result<Option<Frame>> {
        if bounds.height == 0 || bounds.width == 0 || title.is_empty() {
            return Ok(None);
        }
        let color = parse_hex_color(&self.color)
            .unwrap_or_else(|| automatic_title_color(theme.foreground, bar));
        let text = title.lines().collect::<Vec<_>>().join(" ");
        let mut title = Canvas::new(bounds.width, bounds.height, bar)?;
        let font_size = bounds.height as f32 * WINDOW_TITLE_FONT_SCALE;
        let line_height = font_size * WINDOW_TITLE_LINE_HEIGHT;
        let layout = CaptionLayout {
            text_x: 0,
            text_y: bounds.height.saturating_sub(line_height.ceil() as u32) / 2,
            text_width: bounds.width,
            text_height: bounds.height,
            font_size,
            line_height,
        };
        CaptionRenderer::new().draw_line(
            &mut title,
            &text,
            layout,
            text_settings,
            color,
            LineStyle {
                weight: Weight::NORMAL,
                ellipsis: "…",
                centered: true,
            },
        );
        Ok(Some(title.into_frame()))
    }
}

/// Keep the title subdued when possible, while ensuring contrast against the effective bar.
///
/// Start at 65% theme foreground and increase in whole-percent steps until reaching 4.5:1.
/// If even the full theme foreground cannot reach that contrast, use black or white instead.
/// This runs only while preparing the cached title; explicit colors bypass it entirely.
fn automatic_title_color(
    foreground: libghostty_vt::style::RgbColor,
    bar: libghostty_vt::style::RgbColor,
) -> libghostty_vt::style::RgbColor {
    let bar_luminance = relative_luminance(bar);
    let contrast = |color| {
        let luminance = relative_luminance(color);
        (luminance.max(bar_luminance) + 0.05) / (luminance.min(bar_luminance) + 0.05)
    };
    for percent in 65..=100 {
        let share = percent as f64 / 100.0;
        let mix = |foreground, bar| {
            (f64::from(foreground) * share + f64::from(bar) * (1.0 - share)).round() as u8
        };
        let color = super::canvas::rgb(
            mix(foreground.r, bar.r),
            mix(foreground.g, bar.g),
            mix(foreground.b, bar.b),
        );
        if contrast(color) >= 4.5 {
            return color;
        }
    }
    let black = super::canvas::rgb(0, 0, 0);
    let white = super::canvas::rgb(255, 255, 255);
    if contrast(black) >= contrast(white) {
        black
    } else {
        white
    }
}

/// Linear-sRGB relative luminance used to compare title ink with its bar fill.
fn relative_luminance(color: libghostty_vt::style::RgbColor) -> f64 {
    let linear = |channel| {
        let encoded = f64::from(channel) / 255.0;
        if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

#[cfg(test)]
mod tests;
