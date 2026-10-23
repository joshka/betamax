//! Key-chip panel measurement, wrapping, glyph shaping, and drawing.

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};

use crate::ghostty::{fonts, TextSettings};
use crate::runner::canvas::{rgb, Canvas, PixelRect};

/// Maximum number of recent overlay events drawn over the output frame.
const KEYBOARD_OVERLAY_MAX_CHIPS: usize = 5;

/// Maximum number of overlay rows drawn over the output frame.
pub(in crate::runner) const KEYBOARD_OVERLAY_MAX_ROWS: usize = 1;

/// Font size used by the compact keyboard overlay.
const KEYBOARD_OVERLAY_FONT_SIZE: f32 = 18.0;

/// Line height used by the compact keyboard overlay.
pub(in crate::runner) const KEYBOARD_OVERLAY_LINE_HEIGHT: f32 = 25.0;

/// Approximate label glyph width for row wrapping.
const KEYBOARD_OVERLAY_CHAR_WIDTH: u32 = 11;

/// Horizontal inset around the overlay HUD.
const KEYBOARD_OVERLAY_INSET_X: u32 = 14;

/// Vertical inset reserved above keyboard chips in the presentation row.
pub(in crate::runner) const KEYBOARD_OVERLAY_INSET_Y: u32 = 12;

/// Horizontal padding inside one key chip.
const KEYBOARD_OVERLAY_CHIP_PAD_X: u32 = 8;

/// Vertical text adjustment inside one key chip.
pub(in crate::runner) const KEYBOARD_OVERLAY_TEXT_OFFSET_Y: u32 = 1;

/// Gap between key chips.
const KEYBOARD_OVERLAY_CHIP_GAP: u32 = 8;

/// Alpha used for individual key chips.
const KEYBOARD_OVERLAY_CHIP_ALPHA: u8 = 235;

/// Radius used for rounded keyboard overlay keycap corners.
const KEYBOARD_OVERLAY_CHIP_CORNER_RADIUS: u32 = 3;

#[derive(Debug, Clone, Copy)]
pub(in crate::runner) struct KeyboardOverlayAnchor {
    pub(in crate::runner) horizontal: HorizontalAnchor,
    pub(in crate::runner) vertical: VerticalAnchor,
    pub(in crate::runner) left_x: u32,
    pub(in crate::runner) top_y: u32,
    pub(in crate::runner) right_x: u32,
    pub(in crate::runner) bottom_y: u32,
    pub(in crate::runner) inset_x: u32,
    pub(in crate::runner) inset_y: u32,
}

impl KeyboardOverlayAnchor {
    fn width(self) -> u32 {
        self.right_x.saturating_sub(self.left_x)
    }

    pub(in crate::runner) fn inner_left_x(self) -> u32 {
        self.left_x.saturating_add(self.inset_x).min(self.right_x)
    }

    fn inner_right_x(self) -> u32 {
        self.right_x.saturating_sub(self.inset_x).max(self.left_x)
    }

    fn inner_top_y(self) -> u32 {
        self.top_y.saturating_add(self.inset_y).min(self.bottom_y)
    }

    pub(in crate::runner) fn inner_bottom_y(self) -> u32 {
        self.bottom_y.saturating_sub(self.inset_y).max(self.top_y)
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::runner) enum HorizontalAnchor {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::runner) enum VerticalAnchor {
    Top,
    Bottom,
}

/// One wrapped key chip ready for drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct KeyboardOverlayChip {
    /// Display label.
    label: String,
    /// Approximate chip width in pixels.
    width: u32,
}

/// Text renderer used for keyboard overlay labels.
struct OverlayTextRenderer {
    /// Preferred font family inherited from terminal text settings.
    font_family: Option<String>,
    /// Font database and shaping context.
    font_system: FontSystem,
    /// Glyph raster cache.
    swash_cache: SwashCache,
}

impl OverlayTextRenderer {
    /// Create a renderer for overlay text.
    fn new(font_family: Option<String>) -> Self {
        Self {
            font_family,
            font_system: fonts::font_system(),
            swash_cache: SwashCache::new(),
        }
    }

    /// Draw one overlay label, clipped to the provided width.
    fn draw(&mut self, target: &mut Canvas, text: &str, x: u32, y: u32, width: u32) {
        if width == 0 {
            return;
        }

        let metrics = Metrics::new(KEYBOARD_OVERLAY_FONT_SIZE, KEYBOARD_OVERLAY_LINE_HEIGHT);
        let family = fonts::resolve_family(&self.font_system, self.font_family.as_deref());
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        let attrs = Attrs::new().family(family);

        buffer.set_size(Some(width as f32), Some(KEYBOARD_OVERLAY_LINE_HEIGHT));
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.draw(
            &mut self.swash_cache,
            Color::rgb(0x10, 0x18, 0x27),
            |glyph_x, glyph_y, width, height, glyph_color| {
                target.blend_rect(
                    x as i32 + glyph_x,
                    y as i32 + glyph_y,
                    width,
                    height,
                    glyph_color,
                );
            },
        );
    }
}

/// Draw the configured input sequence as compact key chips over the bottom of the frame.
pub(in crate::runner) fn draw_keyboard_overlay(
    target: &mut Canvas,
    labels: &[String],
    text_settings: &TextSettings,
    anchor: KeyboardOverlayAnchor,
) {
    let labels = recent_keyboard_overlay_labels(labels);
    let rows = keyboard_overlay_rows(&labels, anchor.width());
    if rows.is_empty() {
        return;
    }

    let row_height = KEYBOARD_OVERLAY_LINE_HEIGHT.ceil() as u32;
    let row_width = rows
        .iter()
        .map(|row| keyboard_overlay_row_width(row))
        .max()
        .unwrap_or(0);
    let panel_left_x = anchor.inner_left_x().min(target.width);
    let panel_right_x = anchor.inner_right_x().min(target.width);
    let panel_width = row_width.min(panel_right_x.saturating_sub(panel_left_x));
    if panel_width == 0 {
        return;
    }
    let row_block_height = row_height.saturating_mul(rows.len() as u32);
    let panel_x = match anchor.horizontal {
        HorizontalAnchor::Left => panel_left_x,
        HorizontalAnchor::Right => panel_right_x.saturating_sub(panel_width),
    };
    let panel_top_y = anchor.inner_top_y().min(target.height);
    let panel_bottom_y = anchor.inner_bottom_y().min(target.height);
    let mut y = match anchor.vertical {
        VerticalAnchor::Top => panel_top_y,
        VerticalAnchor::Bottom => panel_bottom_y.saturating_sub(row_block_height),
    };
    let mut text = OverlayTextRenderer::new(text_settings.font_family.clone());
    for row in rows {
        let row_width = keyboard_overlay_row_width(&row);
        let mut x = match anchor.horizontal {
            HorizontalAnchor::Left => panel_x,
            HorizontalAnchor::Right => {
                panel_x.saturating_add(panel_width.saturating_sub(row_width))
            }
        };
        for chip in row {
            target.fill_rounded_rect_alpha(
                PixelRect {
                    x,
                    y,
                    width: chip.width,
                    height: row_height,
                },
                rgb(0xf3, 0xf4, 0xf6),
                KEYBOARD_OVERLAY_CHIP_ALPHA,
                KEYBOARD_OVERLAY_CHIP_CORNER_RADIUS,
            );
            text.draw(
                target,
                &chip.label,
                x.saturating_add(KEYBOARD_OVERLAY_CHIP_PAD_X),
                y.saturating_add(KEYBOARD_OVERLAY_TEXT_OFFSET_Y),
                chip.width
                    .saturating_sub(KEYBOARD_OVERLAY_CHIP_PAD_X.saturating_mul(2)),
            );
            x = x
                .saturating_add(chip.width)
                .saturating_add(KEYBOARD_OVERLAY_CHIP_GAP);
        }
        y = y.saturating_add(row_height);
    }
}

/// Return the bounded set of recent labels shown in the HUD.
fn recent_keyboard_overlay_labels(labels: &[String]) -> Vec<String> {
    let start = labels.len().saturating_sub(KEYBOARD_OVERLAY_MAX_CHIPS);
    labels[start..].to_vec()
}

/// Wrap overlay labels into bounded rows of key chips.
fn keyboard_overlay_rows(labels: &[String], width: u32) -> Vec<Vec<KeyboardOverlayChip>> {
    let usable_width = width
        .saturating_sub(KEYBOARD_OVERLAY_INSET_X.saturating_mul(2))
        .max(1);
    let mut rows: Vec<Vec<KeyboardOverlayChip>> = Vec::new();
    let mut current_row = Vec::new();
    let mut current_width = 0u32;

    for label in labels {
        let chip = KeyboardOverlayChip {
            label: label.clone(),
            width: keyboard_overlay_chip_width(label, usable_width),
        };
        let gap = if current_row.is_empty() {
            0
        } else {
            KEYBOARD_OVERLAY_CHIP_GAP
        };
        if !current_row.is_empty()
            && current_width.saturating_add(gap).saturating_add(chip.width) > usable_width
        {
            rows.push(current_row);
            current_row = Vec::new();
            current_width = 0;
        }
        current_width = current_width
            .saturating_add(if current_row.is_empty() {
                0
            } else {
                KEYBOARD_OVERLAY_CHIP_GAP
            })
            .saturating_add(chip.width);
        current_row.push(chip);
    }

    if !current_row.is_empty() {
        rows.push(current_row);
    }
    if rows.len() > KEYBOARD_OVERLAY_MAX_ROWS {
        let keep_from = rows.len() - KEYBOARD_OVERLAY_MAX_ROWS;
        rows.drain(0..keep_from);
        if let Some(first_row) = rows.first_mut() {
            first_row.insert(
                0,
                KeyboardOverlayChip {
                    label: "...".to_string(),
                    width: keyboard_overlay_chip_width("...", usable_width),
                },
            );
        }
    }
    rows
}

/// Width of the right-aligned keyboard overlay area for the current labels.
pub(in crate::runner) fn keyboard_overlay_panel_width(labels: &[String], width: u32) -> u32 {
    keyboard_overlay_rows(&recent_keyboard_overlay_labels(labels), width)
        .iter()
        .map(|row| keyboard_overlay_row_width(row))
        .max()
        .unwrap_or(0)
        .min(width)
}

/// Approximate the width of one overlay row.
fn keyboard_overlay_row_width(row: &[KeyboardOverlayChip]) -> u32 {
    let chips_width = row
        .iter()
        .map(|chip| chip.width)
        .fold(0u32, u32::saturating_add);
    let gaps = row.len().saturating_sub(1).try_into().unwrap_or(u32::MAX);
    chips_width.saturating_add(KEYBOARD_OVERLAY_CHIP_GAP.saturating_mul(gaps))
}

/// Estimate one key chip width while keeping very long labels inside the overlay.
fn keyboard_overlay_chip_width(label: &str, usable_width: u32) -> u32 {
    let text_width = label.chars().count() as u32 * KEYBOARD_OVERLAY_CHAR_WIDTH;
    text_width
        .saturating_add(KEYBOARD_OVERLAY_CHIP_PAD_X.saturating_mul(2))
        .min(usable_width)
        .max(KEYBOARD_OVERLAY_CHIP_PAD_X.saturating_mul(2))
}
