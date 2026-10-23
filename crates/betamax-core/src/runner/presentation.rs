//! Compose window chrome, captions, and keyboard labels in final-output coordinates.
//!
//! This is the single owner of geometry shared by PTY sizing and image decoration. Each feature
//! owns its own metrics and drawing; this module resolves their shared bounds and drawing order.

use std::borrow::Cow;
use std::path::Path;
use std::time::Duration;

use super::canvas::{Canvas, PixelRect};
use super::caption::{
    caption_font_size, caption_overlay_height, CaptionLayout, CaptionRenderer, CAPTION_LINE_HEIGHT,
    CAPTION_VERTICAL_PADDING,
};
use super::keyboard_overlay::{
    draw_keyboard_overlay, keyboard_overlay_panel_width, HorizontalAnchor, KeyboardOverlayAnchor,
    KeyboardOverlayLocation, KeyboardOverlayMode, KeyboardOverlaySettings, VerticalAnchor,
    KEYBOARD_OVERLAY_CORNER_INSET, KEYBOARD_OVERLAY_LINE_HEIGHT, KEYBOARD_OVERLAY_TEXT_OFFSET_Y,
};
use super::margin::margin_backdrop;
use super::window::WindowStyle;
use super::window_shape::WindowOutline;
use crate::ghostty::{TerminalTheme, TextSettings};
use crate::media::Frame;
use crate::Result;

/// Default final output width in pixels.
const DEFAULT_WIDTH: u32 = 1200;

/// Default final output height in pixels.
const DEFAULT_HEIGHT: u32 = 600;

/// Minimum horizontal presentation-row inset without a margin, relative to caption font size.
const PRESENTATION_OVERLAY_MIN_INSET_SCALE: f32 = 0.5;

/// Gap between the window and the GIF presentation-row plate drawn over non-opaque margins.
const PRESENTATION_ROW_PLATE_GAP: u32 = 6;

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
    /// Cached margin canvas, prepared after all startup settings have been applied. Opaque unless
    /// the margin fill is transparent.
    backdrop: Canvas,
    /// Whether the margin fill left any backdrop pixel translucent, so outputs need alpha.
    margin_has_alpha: bool,
    /// Cached bar fill and buttons, prepared with the backdrop after startup settings.
    bar_overlay: Canvas,
    /// Cached shell fill, including decoration outside the terminal rectangle.
    window_canvas: Canvas,
    /// Prepared continuous window and inset-border coverage.
    outline: Option<WindowOutline>,
    /// Theme-colored backing for the presentation row in GIF output over a non-opaque margin.
    gif_row_plate: Option<(Canvas, PixelRect)>,
    /// Whether the tape uses presentation captions.
    pub(super) caption_overlay: bool,
    /// Optional input-sequence overlay for review media.
    pub(super) keyboard_overlay: KeyboardOverlaySettings,
}

impl Presentation {
    /// Construct the 0.1 presentation baseline before presets and explicit tape settings are
    /// applied.
    pub(super) fn new(theme: TerminalTheme) -> Result<Self> {
        Ok(Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            text: TextSettings::default(),
            style: WindowStyle::new(&theme),
            backdrop: Canvas::new(0, 0, theme.background)?,
            margin_has_alpha: false,
            bar_overlay: Canvas::transparent(0, 0)?,
            window_canvas: Canvas::transparent(0, 0)?,
            outline: None,
            gif_row_plate: None,
            caption_overlay: false,
            keyboard_overlay: KeyboardOverlaySettings::default(),
            theme,
        })
    }

    /// Prepare the backdrop once after all startup settings have been applied.
    ///
    /// `tape_dir` is a fallback base for relative `MarginFill` image paths.
    pub(super) fn prepare_backdrop(&mut self, tape_dir: Option<&Path>) -> Result<()> {
        if self.terminal_canvas_width() == 0 || self.terminal_canvas_height() == 0 {
            return Err(miette::miette!(
                "Width and Height leave no terminal canvas; increase the dimensions or use Set Preset Bare"
            ).into());
        }
        self.backdrop = margin_backdrop(
            self.width,
            self.height,
            self.style.margin_fill.as_deref(),
            self.theme.background,
            tape_dir,
        )?;
        self.margin_has_alpha = self.backdrop.has_alpha();
        let bounds = PixelRect {
            x: self.style.effective_margin().left,
            y: self.style.effective_margin().top,
            width: self.terminal_canvas_width(),
            height: self.terminal_canvas_height() + self.style.window_bar_height(),
        };
        let padding = self.text.padding;
        let content = PixelRect {
            x: bounds.x.saturating_add(padding),
            y: bounds
                .y
                .saturating_add(self.style.window_bar_height())
                .saturating_add(padding),
            width: self
                .terminal_canvas_width()
                .saturating_sub(padding.saturating_mul(2)),
            height: self
                .terminal_canvas_height()
                .saturating_sub(padding.saturating_mul(2)),
        };
        let border_width = if self.style.border.visible() {
            self.style.border.width
        } else {
            0
        };
        let outline = WindowOutline::containing(
            bounds,
            content,
            self.style.border_radius,
            border_width,
            self.style.effective_margin().min_side(),
        )?;
        self.style
            .prepare_backdrop(&mut self.backdrop, outline.rect);
        self.window_canvas = self.backdrop.clone();
        let shell = outline.rect;
        self.window_canvas.fill_rect(
            shell.x,
            shell.y,
            shell.width,
            shell.height,
            self.theme.background,
        );
        if self.style.window_bar_height() > 0 {
            self.window_canvas.fill_rect(
                shell.x,
                shell.y,
                shell.width,
                bounds.y + self.style.window_bar_height() - shell.y,
                self.style.bar_color(self.theme.background),
            );
        }
        self.gif_row_plate = self.prepare_gif_row_plate(outline.rect)?;
        self.outline = Some(outline);
        self.bar_overlay =
            self.style
                .prepare_bar(self.width, self.height, self.theme.background)?;
        Ok(())
    }

    /// Prepare a theme-colored plate behind the presentation row for GIF output.
    ///
    /// Captions and keyboard chips are anti-aliased against whatever sits behind them. Over a
    /// clear or translucent margin that is the viewer's page, which alpha formats such as WebP and
    /// PNG keep, but GIF's single-bit transparency drops the soft text edges and leaves captions
    /// broken up on light and dark pages alike. GIF frames are composited over this plate when
    /// they are written, so shared frames stay untouched for the other outputs. The plate covers
    /// the whole reserved row, so it stays put as captions change and chips appear.
    fn prepare_gif_row_plate(&self, shell: PixelRect) -> Result<Option<(Canvas, PixelRect)>> {
        let row_height = self.presentation_overlay_height().min(self.height);
        if row_height == 0 || !self.margin_has_alpha {
            return Ok(None);
        }
        // Match the window's horizontal extent and its inset from the image edge, so the plate
        // reads as a caption strip aligned with the window. The window outline can extend into
        // the row, and narrow margins leave little room, so the gap above the plate never
        // exceeds the window's own inset and the plate may extend into the bottom margin to
        // keep the chips inside it.
        let gap = PRESENTATION_ROW_PLATE_GAP.min(shell.y);
        let top_y = self
            .height
            .saturating_sub(self.style.effective_margin().bottom)
            .saturating_sub(row_height)
            .max(shell.y.saturating_add(shell.height))
            .saturating_add(gap);
        // Leave the same gap below the plate as the outline leaves inside the bottom margin.
        let outline_growth = self.style.effective_margin().top.saturating_sub(shell.y);
        let bottom_y = self.height.saturating_sub(
            self.style
                .effective_margin()
                .bottom
                .saturating_sub(outline_growth),
        );
        let rect = PixelRect {
            x: shell.x,
            y: top_y,
            width: shell.width,
            height: bottom_y.saturating_sub(top_y),
        };
        let mut plate = Canvas::transparent(self.width, self.height)?;
        plate.fill_rounded_rect_alpha(
            rect,
            self.theme.background,
            u8::MAX,
            self.style.border_radius,
        );
        Ok(Some((plate, rect)))
    }

    /// Return frames prepared for GIF encoding, keeping every frame's hold time.
    ///
    /// Over a non-opaque margin each frame is composited over the presentation-row plate from
    /// [`Self::prepare_gif_row_plate`]; otherwise the shared frames are borrowed unchanged.
    pub(super) fn gif_frames<'a>(
        &self,
        frames: &'a [(Frame, Duration)],
    ) -> Result<Cow<'a, [(Frame, Duration)]>> {
        let Some((plate, rect)) = &self.gif_row_plate else {
            return Ok(Cow::Borrowed(frames));
        };
        frames
            .iter()
            .map(|(frame, delay)| {
                let mut canvas = Canvas::from_frame(frame)?;
                canvas.underlay(plate, *rect);
                Ok((canvas.into_frame(), *delay))
            })
            .collect::<Result<Vec<_>>>()
            .map(Cow::Owned)
    }

    /// Width of the terminal canvas before margin and window-bar decoration.
    pub(super) fn margin_has_alpha(&self) -> bool {
        self.margin_has_alpha
    }

    pub(super) fn terminal_canvas_width(&self) -> u32 {
        self.width
            .saturating_sub(self.style.effective_margin().horizontal())
    }

    /// Height of the terminal canvas before margin and window-bar decoration.
    pub(super) fn terminal_canvas_height(&self) -> u32 {
        self.height
            .saturating_sub(self.style.effective_margin().vertical())
            .saturating_sub(self.style.window_bar_height())
            .saturating_sub(self.presentation_overlay_height())
    }

    /// Output width whose terminal canvas is exactly `canvas_width` pixels wide.
    ///
    /// This inverts [`Self::terminal_canvas_width`] for `Set Columns`.
    pub(super) fn frame_width_for_canvas(&self, canvas_width: u32) -> u32 {
        canvas_width.saturating_add(self.style.effective_margin().horizontal())
    }

    /// Output height whose terminal canvas is exactly `canvas_height` pixels tall.
    ///
    /// This inverts [`Self::terminal_canvas_height`] for `Set Rows`.
    pub(super) fn frame_height_for_canvas(&self, canvas_height: u32) -> u32 {
        canvas_height
            .saturating_add(self.style.effective_margin().vertical())
            .saturating_add(self.style.window_bar_height())
            .saturating_add(self.presentation_overlay_height())
    }

    /// Height reserved below the terminal canvas for presentation-only overlays.
    fn presentation_overlay_height(&self) -> u32 {
        let keyboard_overlay_height = if self.keyboard_overlay_in_caption_row() {
            self.keyboard_overlay.keyboard_overlay_height()
        } else {
            0
        };
        caption_overlay_height(self.caption_overlay, &self.text).max(keyboard_overlay_height)
    }

    /// Whether keyboard chips can appear in the presentation row beside captions.
    fn keyboard_overlay_in_caption_row(&self) -> bool {
        self.keyboard_overlay.mode != KeyboardOverlayMode::Off
            && self.keyboard_overlay.location == KeyboardOverlayLocation::CaptionRow
    }

    /// Left edge for presentation content, aligned with the terminal frame.
    fn presentation_overlay_left_x(&self) -> u32 {
        self.style.effective_margin().left.min(self.width)
    }

    /// Right edge for presentation content, aligned with the terminal frame.
    fn presentation_overlay_right_x(&self) -> u32 {
        self.width
            .saturating_sub(self.style.effective_margin().right)
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
        let mut output = self.window_canvas.clone();
        output.blit(frame, margin.left, margin.top + bar_height)?;
        self.style.draw(
            &mut output,
            &self.backdrop,
            &self.bar_overlay,
            self.outline.as_ref().expect("prepared window outline"),
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
            .saturating_sub(self.style.effective_margin().bottom)
            .saturating_sub(height);
        let centered_text_y = y
            .saturating_add(height.saturating_sub(caption_height) / 2)
            .saturating_add(vertical_padding);
        // Align with the keyboard chip row whenever the row can hold chips, so the caption does
        // not shift vertically as labels appear and expire.
        let text_y = if self.keyboard_overlay_in_caption_row() {
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
        let terminal_left_x = margin.left.min(self.width);
        let terminal_right_x = self.width.saturating_sub(margin.right);
        let terminal_top_y = margin
            .top
            .saturating_add(self.style.window_bar_height())
            .min(self.height);
        let terminal_bottom_y = terminal_top_y
            .saturating_add(self.terminal_canvas_height())
            .min(self.height.saturating_sub(margin.bottom));

        match self.keyboard_overlay.location {
            KeyboardOverlayLocation::CaptionRow => KeyboardOverlayAnchor {
                horizontal: HorizontalAnchor::Right,
                vertical: VerticalAnchor::Bottom,
                left_x: self.presentation_overlay_left_x(),
                top_y: self
                    .height
                    .saturating_sub(margin.bottom)
                    .saturating_sub(self.presentation_overlay_height().min(self.height)),
                right_x: self
                    .presentation_overlay_right_x()
                    .saturating_sub(self.presentation_overlay_inset_x()),
                bottom_y: self.height.saturating_sub(margin.bottom),
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
        if self.style.effective_margin().any() {
            return optical_inset;
        }
        let min_inset =
            (caption_font_size(&self.text) * PRESENTATION_OVERLAY_MIN_INSET_SCALE).ceil() as u32;
        self.text.padding.max(optical_inset).max(min_inset)
    }
}

#[cfg(test)]
mod tests;
