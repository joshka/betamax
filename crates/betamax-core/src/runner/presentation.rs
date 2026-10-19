//! Compose window chrome, captions, and keyboard labels in final-output coordinates.
//!
//! This is the single owner of geometry shared by PTY sizing and image decoration. Each feature
//! owns its own metrics and drawing; this module resolves their shared bounds and drawing order.

use std::time::Duration;

use super::canvas::{Canvas, PixelRect};
use super::caption::{
    caption_font_size, caption_overlay_height, CaptionLayout, CaptionRenderer, CAPTION_LINE_HEIGHT,
    CAPTION_VERTICAL_PADDING,
};
use super::keyboard_overlay::{
    draw_keyboard_overlay, keyboard_overlay_panel_width, HorizontalAnchor, KeyboardOverlayAnchor,
    KeyboardOverlayLocation, KeyboardOverlaySettings, VerticalAnchor,
    KEYBOARD_OVERLAY_CORNER_INSET, KEYBOARD_OVERLAY_LINE_HEIGHT, KEYBOARD_OVERLAY_TEXT_OFFSET_Y,
};
use super::margin::margin_backdrop;
use super::window::WindowStyle;
use crate::ghostty::{TerminalTheme, TextSettings};
use crate::media::Frame;
use crate::Result;

/// Default final output width in pixels.
const DEFAULT_WIDTH: u32 = 1200;

/// Default final output height in pixels.
const DEFAULT_HEIGHT: u32 = 600;

/// Minimum horizontal presentation-row inset without a margin, relative to caption font size.
const PRESENTATION_OVERLAY_MIN_INSET_SCALE: f32 = 0.5;

/// Gap between caption text and right-aligned keyboard chips.
const PRESENTATION_OVERLAY_GAP: u32 = 16;

/// Output configuration and cached backdrop for one tape run.
#[derive(Debug, Clone)]
pub(super) struct Presentation {
    /// Final output width after margin/window decoration.
    pub(super) width: u32,
    /// Final output height after margin/window decoration.
    pub(super) height: u32,
    /// Font, cell, and terminal padding settings.
    pub(super) text: TextSettings,
    /// Loaded terminal color theme.
    pub(super) theme: TerminalTheme,
    /// Outer frame decoration settings.
    pub(super) style: WindowStyle,
    /// Cached opaque margin canvas, prepared after all startup settings have been applied.
    backdrop: Canvas,
    /// Cached bar fill and buttons, prepared with the backdrop after startup settings.
    bar_overlay: Canvas,
    /// Whether the tape uses presentation captions.
    pub(super) caption_overlay: bool,
    /// Optional input-sequence overlay for review media.
    pub(super) keyboard_overlay: KeyboardOverlaySettings,
}

impl Presentation {
    /// Construct the initial presentation before tape settings are applied.
    pub(super) fn new(theme: TerminalTheme) -> Result<Self> {
        Ok(Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            text: TextSettings::default(),
            style: WindowStyle::new(&theme),
            backdrop: Canvas::new(0, 0, theme.background)?,
            bar_overlay: Canvas::transparent(0, 0)?,
            caption_overlay: false,
            keyboard_overlay: KeyboardOverlaySettings::default(),
            theme,
        })
    }

    /// Prepare the backdrop once after all startup settings have been applied.
    pub(super) fn prepare_backdrop(&mut self) -> Result<()> {
        self.backdrop = margin_backdrop(
            self.width,
            self.height,
            self.style.margin_fill.as_deref(),
            self.theme.background,
        )?;
        let bounds = PixelRect {
            x: self.style.effective_margin(),
            y: self.style.effective_margin(),
            width: self.terminal_canvas_width(),
            height: self.terminal_canvas_height() + self.style.window_bar_height(),
        };
        self.style.prepare_backdrop(&mut self.backdrop, bounds);
        self.bar_overlay =
            self.style
                .prepare_bar(self.width, self.height, self.theme.background)?;
        Ok(())
    }

    /// Width of the terminal canvas before margin and window-bar decoration.
    pub(super) fn terminal_canvas_width(&self) -> u32 {
        self.width
            .saturating_sub(self.style.effective_margin().saturating_mul(2))
    }

    /// Height of the terminal canvas before margin and window-bar decoration.
    pub(super) fn terminal_canvas_height(&self) -> u32 {
        self.height
            .saturating_sub(self.style.effective_margin().saturating_mul(2))
            .saturating_sub(self.style.window_bar_height())
            .saturating_sub(self.presentation_overlay_height())
    }

    /// Height reserved below the terminal canvas for presentation-only overlays.
    fn presentation_overlay_height(&self) -> u32 {
        let keyboard_overlay_height =
            if self.keyboard_overlay.location == KeyboardOverlayLocation::CaptionRow {
                self.keyboard_overlay.keyboard_overlay_height()
            } else {
                0
            };
        caption_overlay_height(self.caption_overlay, &self.text).max(keyboard_overlay_height)
    }

    /// Left edge for presentation content, aligned with the terminal frame.
    fn presentation_overlay_left_x(&self) -> u32 {
        self.style.effective_margin().min(self.width)
    }

    /// Right edge for presentation content, aligned with the terminal frame.
    fn presentation_overlay_right_x(&self) -> u32 {
        self.width.saturating_sub(self.style.effective_margin())
    }

    /// Decorate a single raw terminal frame with caption text and keyboard labels.
    #[cfg(test)]
    pub(super) fn decorate_frame_with_overlays(
        &self,
        frame: &Frame,
        caption: Option<&str>,
        keyboard_overlay_labels: &[String],
    ) -> Result<Frame> {
        self.decorate_frame_with_title(frame, caption, keyboard_overlay_labels, None)
    }

    /// Decorate a checkpoint with its currently resolved title.
    pub(super) fn decorate_frame_with_title(
        &self,
        frame: &Frame,
        caption: Option<&str>,
        keyboard_overlay_labels: &[String],
        title: Option<&str>,
    ) -> Result<Frame> {
        let mut caption_renderer = CaptionRenderer::new();
        self.decorate_frame_with_overlay_renderer(
            frame,
            caption,
            keyboard_overlay_labels,
            &mut caption_renderer,
            title,
        )
    }

    /// Decorate captured frames with one reusable caption renderer.
    ///
    /// Animated outputs may render hundreds of frames with the same caption text and font. Keeping
    /// one renderer here preserves the shaping and glyph caches for the batch without making cache
    /// lifetime part of `Presentation`.
    pub(super) fn decorate_captured_frames(
        &self,
        frames: impl IntoIterator<Item = (super::capture::CapturedFrame, Duration)>,
    ) -> Result<Vec<(Frame, Duration)>> {
        let mut caption_renderer = CaptionRenderer::new();
        let mut decorated = Vec::new();
        for (captured, delay) in frames {
            decorated.push((
                self.decorate_frame_with_overlay_renderer(
                    &captured.frame,
                    captured.caption.as_deref(),
                    &captured.keyboard_overlay_labels,
                    &mut caption_renderer,
                    captured.window_title.as_deref(),
                )?,
                delay,
            ));
        }
        Ok(decorated)
    }

    /// Decorate a frame using a caller-owned caption renderer cache.
    ///
    /// Decoration happens in final output coordinates: margin, window bar, rounded mask, then the
    /// presentation row for captions and keyboard labels. Keeping presentation drawing here
    /// guarantees the same overlay path for final PNGs, screenshots, GIFs, and videos.
    fn decorate_frame_with_overlay_renderer(
        &self,
        frame: &Frame,
        caption: Option<&str>,
        keyboard_overlay_labels: &[String],
        caption_renderer: &mut CaptionRenderer,
        title: Option<&str>,
    ) -> Result<Frame> {
        let margin = self.style.effective_margin();
        let bar_height = self.style.window_bar_height();
        let mut output = self.backdrop.clone();
        output.blit(frame, margin, margin + bar_height)?;
        self.style.draw(
            &mut output,
            frame,
            &self.backdrop,
            &self.bar_overlay,
            &self.theme,
            &self.text,
            title,
        )?;
        let active_caption = caption.filter(|caption| !caption.trim().is_empty());
        let keyboard_avoid_width = if self.keyboard_overlay.visible(keyboard_overlay_labels)
            && self.keyboard_overlay.location == KeyboardOverlayLocation::CaptionRow
        {
            keyboard_overlay_panel_width(keyboard_overlay_labels, output.width)
                .saturating_add(PRESENTATION_OVERLAY_GAP)
        } else {
            0
        };
        if let Some(caption) = active_caption {
            self.draw_caption(&mut output, caption, caption_renderer, keyboard_avoid_width);
        }
        if self.keyboard_overlay.visible(keyboard_overlay_labels) {
            draw_keyboard_overlay(
                &mut output,
                keyboard_overlay_labels,
                &self.text,
                self.keyboard_overlay_anchor(),
            );
        }
        Ok(output.into_frame())
    }

    /// Draw a caption into the final decorated frame.
    ///
    /// Captions are clipped to their available row width and truncated with `...` when active
    /// keyboard labels need space on the right.
    fn draw_caption(
        &self,
        output: &mut Canvas,
        caption: &str,
        caption_renderer: &mut CaptionRenderer,
        avoid_right_width: u32,
    ) {
        let Some(layout) = self.caption_layout(avoid_right_width) else {
            return;
        };
        caption_renderer.draw(output, caption, layout, &self.text, self.theme.foreground);
    }

    /// Compute caption geometry in final output-frame coordinates.
    ///
    /// The caption sits in the shared bottom presentation row below the terminal canvas. Keyboard
    /// labels are right-aligned in the same row; the caption width is reduced so the two surfaces
    /// do not overlap.
    fn caption_layout(&self, avoid_right_width: u32) -> Option<CaptionLayout> {
        if !self.caption_overlay {
            return None;
        }
        let left_x = self.caption_overlay_left_x();
        let right_x = self.presentation_overlay_right_x();
        let width = right_x
            .saturating_sub(left_x)
            .saturating_sub(avoid_right_width);
        if width == 0 || self.height == 0 {
            return None;
        }

        let font_size = caption_font_size(&self.text);
        let line_height = (font_size * CAPTION_LINE_HEIGHT).ceil();
        let vertical_padding = (font_size * CAPTION_VERTICAL_PADDING).ceil() as u32;
        let height = self.presentation_overlay_height().min(self.height);
        let caption_height = caption_overlay_height(self.caption_overlay, &self.text).min(height);

        let y = self
            .height
            .saturating_sub(self.style.effective_margin())
            .saturating_sub(height);
        let centered_text_y = y
            .saturating_add(height.saturating_sub(caption_height) / 2)
            .saturating_add(vertical_padding);
        let text_y = if avoid_right_width > 0 {
            let keyboard_anchor = self.keyboard_overlay_anchor();
            let keyboard_row_height = KEYBOARD_OVERLAY_LINE_HEIGHT.ceil() as u32;
            let keyboard_text_y = keyboard_anchor
                .bottom_y
                .saturating_sub(keyboard_row_height)
                .saturating_add(KEYBOARD_OVERLAY_TEXT_OFFSET_Y);
            keyboard_text_y.min(
                keyboard_anchor
                    .bottom_y
                    .saturating_sub(caption_height.saturating_sub(vertical_padding * 2)),
            )
        } else {
            centered_text_y
        };
        Some(CaptionLayout {
            text_x: left_x,
            text_y,
            text_width: width,
            text_height: caption_height.saturating_sub(vertical_padding.saturating_mul(2)),
            font_size,
            line_height,
        })
    }

    /// Anchor for keyboard overlay chips in final output-frame coordinates.
    fn keyboard_overlay_anchor(&self) -> KeyboardOverlayAnchor {
        let margin = self.style.effective_margin();
        let terminal_left_x = margin.min(self.width);
        let terminal_right_x = self.width.saturating_sub(margin);
        let terminal_top_y = margin
            .saturating_add(self.style.window_bar_height())
            .min(self.height);
        let terminal_bottom_y = terminal_top_y
            .saturating_add(self.terminal_canvas_height())
            .min(self.height.saturating_sub(margin));

        match self.keyboard_overlay.location {
            KeyboardOverlayLocation::CaptionRow => KeyboardOverlayAnchor {
                horizontal: HorizontalAnchor::Right,
                vertical: VerticalAnchor::Bottom,
                left_x: self.presentation_overlay_left_x(),
                top_y: self
                    .height
                    .saturating_sub(margin)
                    .saturating_sub(self.presentation_overlay_height().min(self.height)),
                right_x: self
                    .presentation_overlay_right_x()
                    .saturating_sub(self.presentation_overlay_inset_x()),
                bottom_y: self.height.saturating_sub(margin),
                inset_x: 0,
                inset_y: 0,
            },
            KeyboardOverlayLocation::TopLeft => KeyboardOverlayAnchor {
                horizontal: HorizontalAnchor::Left,
                vertical: VerticalAnchor::Top,
                left_x: terminal_left_x,
                top_y: terminal_top_y,
                right_x: terminal_right_x,
                bottom_y: terminal_bottom_y,
                inset_x: KEYBOARD_OVERLAY_CORNER_INSET,
                inset_y: KEYBOARD_OVERLAY_CORNER_INSET,
            },
            KeyboardOverlayLocation::TopRight => KeyboardOverlayAnchor {
                horizontal: HorizontalAnchor::Right,
                vertical: VerticalAnchor::Top,
                left_x: terminal_left_x,
                top_y: terminal_top_y,
                right_x: terminal_right_x,
                bottom_y: terminal_bottom_y,
                inset_x: KEYBOARD_OVERLAY_CORNER_INSET,
                inset_y: KEYBOARD_OVERLAY_CORNER_INSET,
            },
            KeyboardOverlayLocation::BottomLeft => KeyboardOverlayAnchor {
                horizontal: HorizontalAnchor::Left,
                vertical: VerticalAnchor::Bottom,
                left_x: terminal_left_x,
                top_y: terminal_top_y,
                right_x: terminal_right_x,
                bottom_y: terminal_bottom_y,
                inset_x: KEYBOARD_OVERLAY_CORNER_INSET,
                inset_y: KEYBOARD_OVERLAY_CORNER_INSET,
            },
            KeyboardOverlayLocation::BottomRight => KeyboardOverlayAnchor {
                horizontal: HorizontalAnchor::Right,
                vertical: VerticalAnchor::Bottom,
                left_x: terminal_left_x,
                top_y: terminal_top_y,
                right_x: terminal_right_x,
                bottom_y: terminal_bottom_y,
                inset_x: KEYBOARD_OVERLAY_CORNER_INSET,
                inset_y: KEYBOARD_OVERLAY_CORNER_INSET,
            },
        }
    }

    /// Left edge for caption text after the presentation-row inset.
    fn caption_overlay_left_x(&self) -> u32 {
        self.presentation_overlay_left_x()
            .saturating_add(self.presentation_overlay_inset_x())
            .min(self.presentation_overlay_right_x())
    }

    /// Horizontal inset applied to both ends of the presentation row.
    ///
    /// With a margin, the presentation row sits below a visible frame, so content aligns with the
    /// frame edge plus an optical inset of half the `BorderRadius`. Without a margin, the row spans
    /// the full image width, so content uses the terminal `Padding` and lines up with terminal
    /// column 0, keeping at least half the caption font size so text never touches the image edge.
    fn presentation_overlay_inset_x(&self) -> u32 {
        let optical_inset = self.style.border_radius / 2;
        if self.style.effective_margin() > 0 {
            return optical_inset;
        }
        let min_inset =
            (caption_font_size(&self.text) * PRESENTATION_OVERLAY_MIN_INSET_SCALE).ceil() as u32;
        self.text.padding.max(optical_inset).max(min_inset)
    }
}

#[cfg(test)]
mod tests;
