//! JetBrains Mono font data embedded as Betamax's default font.
//!
//! The font files are unmodified JetBrains Mono 2.304 variable fonts, licensed under the SIL Open
//! Font License 1.1 (see `OFL.txt` in this crate). The `wght` axis provides regular and bold, so
//! two files cover the four terminal styles.

/// Family name the embedded fonts register under.
pub const JETBRAINS_MONO_FAMILY: &str = "JetBrains Mono";

/// JetBrains Mono upright variable font (`JetBrainsMono[wght].ttf`).
pub const JETBRAINS_MONO: &[u8] = include_bytes!("../fonts/JetBrainsMono-wght.ttf");

/// JetBrains Mono italic variable font (`JetBrainsMono-Italic[wght].ttf`).
pub const JETBRAINS_MONO_ITALIC: &[u8] = include_bytes!("../fonts/JetBrainsMono-Italic-wght.ttf");

/// Every embedded font file, for loading into a font database.
pub const ALL: [&[u8]; 2] = [JETBRAINS_MONO, JETBRAINS_MONO_ITALIC];
