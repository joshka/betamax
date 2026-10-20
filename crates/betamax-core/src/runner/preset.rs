//! Named presentation defaults, resolved before explicit tape settings and theme fallbacks.

use miette::miette;

use super::oklch::Oklch;
use super::presentation::Presentation;
use crate::ghostty::TerminalTheme;
use crate::tape::{Command, Tape, Value};
use crate::Result;

/// A visual baseline, independent of capture timing and output format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Preset {
    #[default]
    Framed,
    Bare,
    Inset,
    Default01,
}

impl Preset {
    /// Last preset wins. Validate every occurrence, including overridden selections.
    pub(super) fn from_tape(tape: &Tape) -> Result<Self> {
        let mut preset = Self::default();
        for command in tape.startup_commands() {
            if let Command::Set { key, value } = command {
                if key != "Preset" {
                    continue;
                }
                let Value::String(name) = value else {
                    return Err(miette!(
                        "Set Preset expects Default, Framed, Bare, Inset, or Default-0.1"
                    )
                    .into());
                };
                preset = match name.to_ascii_lowercase().as_str() {
                    "default" => Self::default(),
                    "framed" => Self::Framed,
                    "bare" => Self::Bare,
                    "inset" => Self::Inset,
                    "default-0.1" => Self::Default01,
                    _ => return Err(miette!(
                        "unknown Preset `{name}`; expected Default, Framed, Bare, Inset, or Default-0.1"
                    )
                    .into()),
                };
            }
        }
        Ok(preset)
    }

    /// Apply one baseline before ordinary Set commands. Default-0.1 retains the pre-preset runtime
    /// defaults.
    pub(super) fn apply(self, presentation: &mut Presentation) -> Result<()> {
        if self == Self::Default01 {
            return Ok(());
        }
        presentation.theme = TerminalTheme::from_name("Aardvark Ink")?;
        presentation.style.window_bar_color = presentation.theme.background_hex();
        presentation.height = 720;
        presentation.text.font_size = 22.0;
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
        if matches!(self, Self::Bare | Self::Default01) {
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
        // The bar and the inset backdrop keep the plain background mix; only the framed gradient
        // follows the background's hue.
        let surface = if dark { mix(0, 25) } else { mix(0, 4) };
        if presentation.style.margin_fill.is_none() {
            presentation.style.margin_fill = Some(if self == Self::Framed {
                framed_gradient(bg, dark)
            } else {
                surface.clone()
            });
        }
        presentation.style.window_bar_color = surface;
    }
}

/// A backdrop gradient in the background's own hue, darker than the window so the window stands
/// out. Both stops add a little chroma, capped so warm light themes stay subtle; neutral
/// backgrounds stay neutral.
fn framed_gradient(bg: libghostty_vt::style::RgbColor, dark: bool) -> String {
    let base = Oklch::from_rgb(bg);
    let bottom_lightness = if dark {
        base.lightness * 0.70
    } else {
        base.lightness - 0.10
    };
    let top = Oklch {
        lightness: base.lightness - 0.03,
        chroma: (base.chroma * 1.6).min(0.05),
        ..base
    };
    let bottom = Oklch {
        lightness: bottom_lightness,
        chroma: (base.chroma * 1.3).min(0.05),
        ..base
    };
    let hex = |color: Oklch| {
        let color = color.to_rgb();
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    };
    format!("linear({}, {})", hex(top), hex(bottom))
}

#[cfg(test)]
mod tests;
