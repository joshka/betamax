//! Caption semantics and typography, plus single-line text rendering shared with window titles.

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap};

use super::canvas::{Canvas, PixelRect};
use crate::ghostty::{fonts, TextSettings};

/// Caption font size relative to the tape font size.
const CAPTION_FONT_SCALE: f32 = 0.9;

/// Minimum readable caption font size.
const MIN_CAPTION_FONT_SIZE: f32 = 14.0;

/// Caption line height relative to caption font size.
pub(super) const CAPTION_LINE_HEIGHT: f32 = 1.2;

/// Vertical caption padding relative to caption font size.
pub(super) const CAPTION_VERTICAL_PADDING: f32 = 0.25;

#[derive(Debug, Clone, Copy)]
pub(super) struct CaptionLayout {
    pub(super) text_x: u32,
    pub(super) text_y: u32,
    pub(super) text_width: u32,
    pub(super) text_height: u32,
    pub(super) font_size: f32,
    pub(super) line_height: f32,
}

impl CaptionLayout {
    /// Return the caption text bounds as a clipping rectangle.
    fn text_rect(self) -> PixelRect {
        PixelRect {
            x: self.text_x,
            y: self.text_y,
            width: self.text_width,
            height: self.text_height,
        }
    }
}

/// Appearance and placement for a shaped single line.
pub(super) struct LineStyle<'a> {
    pub(super) weight: Weight,
    pub(super) ellipsis: &'a str,
    pub(super) centered: bool,
}

/// Shared single-line renderer for captions and window titles.
///
/// This owns `cosmic_text`'s mutable shaping and raster caches. The renderer is kept outside
/// `Settings` so decoration remains a pure operation from caller input to media frame, while
/// callers that render many frames can still reuse the expensive font state.
pub(super) struct CaptionRenderer {
    /// Font database and shaping context.
    font_system: FontSystem,
    /// Glyph raster cache.
    swash_cache: SwashCache,
}

impl CaptionRenderer {
    /// Create a caption renderer with reusable font and glyph caches.
    pub(super) fn new() -> Self {
        Self {
            font_system: fonts::font_system(),
            swash_cache: SwashCache::new(),
        }
    }

    /// Draw caption text into the given layout.
    ///
    /// Caption text is shaped, truncated, and drawn without wrapping. That keeps the presentation
    /// row stable and avoids hiding terminal content or colliding with keyboard labels.
    pub(super) fn draw(
        &mut self,
        target: &mut Canvas,
        caption: &str,
        layout: CaptionLayout,
        text_settings: &TextSettings,
        color: libghostty_vt::style::RgbColor,
    ) {
        self.draw_line(
            target,
            caption,
            layout,
            text_settings,
            color,
            LineStyle {
                weight: Weight::BOLD,
                ellipsis: "...",
                centered: false,
            },
        );
    }

    /// Shape, fit, and rasterize one line using caller-owned typography and placement.
    pub(super) fn draw_line(
        &mut self,
        target: &mut Canvas,
        caption: &str,
        mut layout: CaptionLayout,
        text_settings: &TextSettings,
        color: libghostty_vt::style::RgbColor,
        style: LineStyle<'_>,
    ) {
        let family = fonts::resolve_family(&self.font_system, text_settings.font_family.as_deref());
        let attrs = Attrs::new().family(family).weight(style.weight);
        let metrics = Metrics::new(layout.font_size, layout.line_height);
        let caption = self.truncate_text(caption, layout, metrics, &attrs, style.ellipsis);
        if style.centered {
            let width = self
                .caption_text_width(&caption, layout, metrics, &attrs)
                .ceil() as u32;
            layout.text_x += layout.text_width.saturating_sub(width) / 2;
            layout.text_width = width.min(layout.text_width);
        }

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        buffer.set_size(
            Some(layout.text_width as f32),
            Some(layout.text_height as f32),
        );
        buffer.set_wrap(Wrap::None);
        buffer.set_text(&caption, &attrs, Shaping::Advanced, None);
        buffer.draw(
            &mut self.swash_cache,
            Color::rgb(color.r, color.g, color.b),
            |glyph_x, glyph_y, width, height, glyph_color| {
                target.blend_rect_clipped(
                    layout.text_x as i32 + glyph_x,
                    layout.text_y as i32 + glyph_y,
                    width,
                    height,
                    glyph_color,
                    layout.text_rect(),
                );
            },
        );
    }

    /// Truncate caption text with the same shaping stack used for drawing.
    pub(super) fn truncate_text(
        &mut self,
        caption: &str,
        layout: CaptionLayout,
        metrics: Metrics,
        attrs: &Attrs<'_>,
        ellipsis: &str,
    ) -> String {
        if self.caption_text_width(caption, layout, metrics, attrs) <= layout.text_width as f32 {
            return caption.to_string();
        }
        if self.caption_text_width(ellipsis, layout, metrics, attrs) > layout.text_width as f32 {
            return ellipsis.to_string();
        }

        let char_ends = caption
            .char_indices()
            .map(|(index, ch)| index + ch.len_utf8())
            .collect::<Vec<_>>();
        let mut low = 0;
        let mut high = char_ends.len();
        while low < high {
            let mid = (low + high).div_ceil(2);
            let candidate = format!("{}{}", &caption[..char_ends[mid - 1]], ellipsis);
            if self.caption_text_width(&candidate, layout, metrics, attrs)
                <= layout.text_width as f32
            {
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        if low == 0 {
            return ellipsis.to_string();
        }
        format!("{}{}", &caption[..char_ends[low - 1]], ellipsis)
    }

    /// Measure a single caption line after font fallback and shaping.
    pub(super) fn caption_text_width(
        &mut self,
        caption: &str,
        layout: CaptionLayout,
        metrics: Metrics,
        attrs: &Attrs<'_>,
    ) -> f32 {
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        buffer.set_size(None, Some(layout.text_height as f32));
        buffer.set_wrap(Wrap::None);
        buffer.set_text(caption, attrs, Shaping::Advanced, None);
        buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0, f32::max)
    }
}

/// Font size used by caption rendering and layout reservation.
pub(super) fn caption_font_size(text: &TextSettings) -> f32 {
    (text.font_size * CAPTION_FONT_SCALE).max(MIN_CAPTION_FONT_SIZE)
}

/// Height reserved for captions when any non-empty caption appears in the tape.
pub(super) fn caption_overlay_height(enabled: bool, text: &TextSettings) -> u32 {
    if !enabled {
        return 0;
    }
    let font_size = caption_font_size(text);
    let line_height = (font_size * CAPTION_LINE_HEIGHT).ceil() as u32;
    let vertical_padding = (font_size * CAPTION_VERTICAL_PADDING).ceil() as u32;
    line_height.saturating_add(vertical_padding.saturating_mul(2))
}

/// Convert parsed caption text into the active presentation state.
///
/// Empty or whitespace-only captions clear the overlay. Non-empty captions keep their original
/// spacing so authors can intentionally align short labels within the rendered overlay.
pub(super) fn active_caption(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use cosmic_text::Family;

    use super::*;

    #[test]
    fn caption_truncation_uses_shaped_width_and_three_dots() {
        let mut renderer = CaptionRenderer::new();
        let layout = CaptionLayout {
            text_x: 0,
            text_y: 0,
            text_width: 150,
            text_height: 28,
            font_size: 20.0,
            line_height: 24.0,
        };
        let metrics = Metrics::new(layout.font_size, layout.line_height);
        let attrs = Attrs::new().family(Family::Monospace).weight(Weight::BOLD);
        let caption = "The bottom terminal row remains visible beside the key chips";

        let truncated = renderer.truncate_text(caption, layout, metrics, &attrs, "...");

        assert!(truncated.ends_with("..."));
        assert!(truncated.len() < caption.len());
        assert!(
            renderer.caption_text_width(&truncated, layout, metrics, &attrs)
                <= layout.text_width as f32
        );
    }

    #[test]
    fn caption_truncation_uses_full_ellipsis_when_space_is_tiny() {
        let mut renderer = CaptionRenderer::new();
        let layout = CaptionLayout {
            text_x: 0,
            text_y: 0,
            text_width: 1,
            text_height: 28,
            font_size: 20.0,
            line_height: 24.0,
        };
        let metrics = Metrics::new(layout.font_size, layout.line_height);
        let attrs = Attrs::new().family(Family::Monospace).weight(Weight::BOLD);

        assert_eq!(
            renderer.truncate_text("overflow", layout, metrics, &attrs, "..."),
            "..."
        );
    }
    #[test]
    fn empty_caption_text_clears_overlay_state() {
        assert_eq!(active_caption(""), None);
        assert_eq!(active_caption("   "), None);
        assert_eq!(active_caption("  Step 1  ").as_deref(), Some("  Step 1  "));
    }
}
