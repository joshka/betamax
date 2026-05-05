//! Color mapping from libghostty-vt's resolved palette into Betamax's selected theme.

use libghostty_vt::style::{RgbColor, Style};

use super::theme::TerminalTheme;

/// Color mapping between libghostty-vt's source palette and Betamax's selected output theme.
///
/// libghostty-vt resolves terminal default and ANSI colors against its own palette. Betamax then
/// maps those source colors back onto the selected theme so the software renderer can use copied
/// Ghostty themes while preserving arbitrary truecolor application output.
///
/// TODO: Prefer initializing libghostty-vt with Betamax's selected foreground, background, cursor,
/// and palette once the safe Rust wrapper exposes the C API's color setters. Ghostty's C examples
/// demonstrate this through `ghostty_terminal_set`, but `libghostty-vt` 0.1.1 currently exposes
/// only dimensions and scrollback in `TerminalOptions`.
pub(super) struct RenderTheme {
    /// Target background color.
    pub(super) background: RgbColor,
    /// Target foreground color.
    pub(super) foreground: RgbColor,
    /// Target cursor color.
    pub(super) cursor: RgbColor,
    /// libghostty-vt source background color.
    source_background: RgbColor,
    /// libghostty-vt source foreground color.
    source_foreground: RgbColor,
    /// libghostty-vt source 256-color palette.
    source_palette: [RgbColor; 256],
    /// Target 16-color palette from the selected theme.
    target_palette: [RgbColor; 16],
}

impl RenderTheme {
    /// Create a color mapper from libghostty-vt colors to the selected output theme.
    pub(super) fn new(
        theme: &TerminalTheme,
        source_background: RgbColor,
        source_foreground: RgbColor,
        source_palette: [RgbColor; 256],
    ) -> Self {
        Self {
            background: theme.background,
            foreground: theme.foreground,
            cursor: theme.cursor,
            source_background,
            source_foreground,
            source_palette,
            target_palette: theme.palette,
        }
    }

    /// Map a libghostty-vt color into the selected theme.
    ///
    /// Default background/foreground are mapped directly. ANSI palette colors are mapped by index
    /// for the first 16 colors. Other RGB colors are left unchanged so truecolor app output remains
    /// faithful.
    pub(super) fn map_color(&self, color: RgbColor) -> RgbColor {
        if color == self.source_background {
            return self.background;
        }
        if color == self.source_foreground {
            return self.foreground;
        }
        for (index, source_color) in self.source_palette.iter().take(16).enumerate() {
            if color == *source_color {
                return self.target_palette[index];
            }
        }
        color
    }
}

/// Resolve foreground/background colors after terminal style flags.
///
/// Inverse swaps foreground and background before theme mapping. Background defaults are normalized
/// so default cells map to the selected theme background.
pub(super) fn style_colors(
    style: Style,
    foreground: RgbColor,
    background: RgbColor,
    default_background: RgbColor,
) -> (RgbColor, RgbColor) {
    if style.inverse {
        (background, foreground)
    } else {
        (
            foreground,
            background_for_cell(background, default_background),
        )
    }
}

/// Return a cell background, preserving explicit non-default backgrounds.
fn background_for_cell(background: RgbColor, default_background: RgbColor) -> RgbColor {
    if background != default_background {
        background
    } else {
        default_background
    }
}
