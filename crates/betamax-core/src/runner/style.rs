//! Named presentation defaults, resolved before explicit tape settings and theme fallbacks.

use miette::miette;

use super::presentation::Presentation;
use crate::ghostty::TerminalTheme;
use crate::tape::{Command, Tape, Value};
use crate::Result;

/// A visual baseline, independent of capture timing and output format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Style {
    #[default]
    Framed,
    Bare,
    Inset,
    Vhs,
}

impl Style {
    /// Last style wins. Validate every occurrence, including overridden selections.
    pub(super) fn from_tape(tape: &Tape) -> Result<Self> {
        let mut style = Self::default();
        for command in &tape.commands {
            if let Command::Set { key, value } = command {
                if key != "Style" {
                    continue;
                }
                let Value::String(name) = value else {
                    return Err(miette!("Set Style expects Framed, Bare, Inset, or VHS").into());
                };
                style = match name.to_ascii_lowercase().as_str() {
                    "framed" => Self::Framed,
                    "bare" => Self::Bare,
                    "inset" => Self::Inset,
                    "vhs" => Self::Vhs,
                    _ => {
                        return Err(miette!(
                            "unknown Style `{name}`; expected Framed, Bare, Inset, or VHS"
                        )
                        .into())
                    }
                };
            }
        }
        Ok(style)
    }

    /// Apply one baseline before ordinary Set commands. VHS retains the former runtime defaults.
    pub(super) fn apply(self, presentation: &mut Presentation) -> Result<()> {
        if self == Self::Vhs {
            return Ok(());
        }
        presentation.theme = TerminalTheme::from_name("Aardvark Ink")?;
        presentation.style.window_bar_color = presentation.theme.background_hex();
        presentation.height = 720;
        presentation.text.font_size = 24.0;
        presentation.text.letter_spacing = 0.0;
        presentation.text.padding = 0;
        presentation.style.window_title.follow_terminal = true;
        if self == Self::Bare {
            return Ok(());
        }
        presentation.style.margin = if self == Self::Framed { 32 } else { 16 };
        presentation.style.border_radius = 11;
        presentation.style.border.width = 1;
        if self == Self::Framed {
            presentation.style.window_bar = "Colorful".into();
            presentation.style.shadow.enabled = true;
            presentation.style.shadow.blur = 20;
            presentation.style.shadow.offset = 6;
        }
        Ok(())
    }

    /// Theme-derived style surfaces follow the final theme; explicit colors remain authoritative.
    pub(super) fn resolve_surfaces(self, presentation: &mut Presentation) {
        if matches!(self, Self::Bare | Self::Vhs) {
            return;
        }
        let bg = presentation.theme.background;
        let fg = presentation.theme.foreground;
        let brightness = |color: libghostty_vt::style::RgbColor| {
            u32::from(color.r) * 2126 + u32::from(color.g) * 7152 + u32::from(color.b) * 722
        };
        let dark = brightness(bg) < brightness(fg);
        let mix = |target: u8, percent: u32| {
            let channel = |value: u8| {
                (u32::from(value) * (100 - percent) + u32::from(target) * percent + 50) / 100
            };
            format!(
                "#{:02x}{:02x}{:02x}",
                channel(bg.r),
                channel(bg.g),
                channel(bg.b)
            )
        };
        let (top, bottom) = if dark {
            (mix(255, 8), mix(0, 25))
        } else {
            (mix(0, 12), mix(0, 4))
        };
        if presentation.style.margin_fill.is_none() {
            presentation.style.margin_fill = Some(if self == Self::Framed {
                format!("linear({top}, {bottom})")
            } else {
                bottom.clone()
            });
        }
        presentation.style.window_bar_color = bottom;
    }
}

#[cfg(test)]
mod tests;
