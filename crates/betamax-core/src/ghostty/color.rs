//! Color helpers shared by Ghostty theme loading, rasterization, and state snapshots.

use libghostty_vt::style::RgbColor;
use miette::{miette, IntoDiagnostic};

use crate::Result;

/// Construct an RGB color.
pub(super) const fn rgb(r: u8, g: u8, b: u8) -> RgbColor {
    RgbColor { r, g, b }
}

/// Format an RGB color as lowercase `#RRGGBB`.
pub(super) fn hex_color(color: RgbColor) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// Parse a strict `#RRGGBB` color used by theme loading.
pub(super) fn parse_hex_color(value: &str) -> Result<RgbColor> {
    let value = value.trim();
    let value = value
        .strip_prefix('#')
        .ok_or_else(|| miette!("theme color must start with `#`: {value}"))?;
    if value.len() != 6 {
        return Err(miette!("theme color must be #rrggbb: #{value}").into());
    }
    Ok(rgb(
        u8::from_str_radix(&value[0..2], 16).into_diagnostic()?,
        u8::from_str_radix(&value[2..4], 16).into_diagnostic()?,
        u8::from_str_radix(&value[4..6], 16).into_diagnostic()?,
    ))
}
