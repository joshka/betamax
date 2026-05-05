//! Theme loading and text metrics.
//!
//! Theme names are resolved from the same search paths Ghostty users expect: local Ghostty theme
//! directories first, then `GHOSTTY_RESOURCES_DIR`, then Betamax's copied Ghostty resources. Inline
//! JSON themes are intentionally small and exist for VHS-style tapes and tests.

use std::path::{Path, PathBuf};
use std::{env, fs};

use libghostty_vt::style::RgbColor;
use miette::{miette, IntoDiagnostic};
use serde::Deserialize;

use super::color::{hex_color, parse_hex_color, rgb};
use crate::Result;

/// VHS-like default font size in pixels.
const DEFAULT_FONT_SIZE: f32 = 22.0;
/// VHS-like default letter spacing in pixels.
const DEFAULT_LETTER_SPACING: f32 = 1.0;
/// Default line-height multiplier.
const DEFAULT_LINE_HEIGHT: f32 = 1.0;
/// Default terminal padding in pixels.
const DEFAULT_PADDING: u32 = 60;
/// Approximate monospace cell width as a fraction of font size.
///
/// This is a rasterizer estimate chosen to keep Betamax close to VHS defaults until Ghostty exposes
/// measured text metrics through an off-screen renderer.
const CELL_WIDTH_FONT_RATIO: f32 = 0.56;
/// Minimum cell dimension in pixels.
const MIN_CELL_PIXELS: f32 = 1.0;

/// Typography and padding settings used by Betamax's software rasterizer.
///
/// These settings influence both the rendered frame and the terminal grid calculation performed by
/// the runner. The current cell metrics are approximations chosen to match Betamax's VHS-like
/// defaults until Ghostty exposes a native off-screen renderer.
#[derive(Debug, Clone)]
pub struct TextSettings {
    /// Font size in pixels.
    ///
    /// The renderer treats non-positive effective cell dimensions as one pixel when deriving the
    /// terminal grid, but callers should use positive finite values for predictable output.
    pub font_size: f32,
    /// Preferred font family. If absent, `cosmic-text` falls back to a generic monospace family.
    pub font_family: Option<String>,
    /// Additional letter spacing in pixels.
    ///
    /// Positive values widen cells. Negative values are accepted but may collapse toward the
    /// one-pixel minimum cell width.
    pub letter_spacing: f32,
    /// Line-height multiplier relative to font size.
    ///
    /// Callers should use positive finite values. Very small or negative effective heights are
    /// clamped to one pixel by [`TextSettings::cell_height`].
    pub line_height: f32,
    /// Pixel padding between the canvas edge and the terminal grid.
    ///
    /// Large padding values can leave no room for terminal cells; the runner clamps the resulting
    /// grid to at least one row and column before opening libghostty-vt.
    pub padding: u32,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            font_family: Some("JetBrains Mono".to_string()),
            letter_spacing: DEFAULT_LETTER_SPACING,
            line_height: DEFAULT_LINE_HEIGHT,
            padding: DEFAULT_PADDING,
        }
    }
}

impl TextSettings {
    /// Approximate terminal cell width for the current font settings.
    ///
    /// This is a pragmatic rasterizer estimate, not a font-engine measurement. It keeps Betamax
    /// output close to VHS defaults until Ghostty exposes an off-screen renderer.
    pub fn cell_width(&self) -> u32 {
        ((self.font_size * CELL_WIDTH_FONT_RATIO) + self.letter_spacing)
            .ceil()
            .max(MIN_CELL_PIXELS) as u32
    }

    /// Terminal cell height for the current font settings.
    pub fn cell_height(&self) -> u32 {
        (self.font_size * self.line_height)
            .ceil()
            .max(MIN_CELL_PIXELS) as u32
    }
}

/// Terminal color theme used for rendering.
///
/// Betamax maps libghostty-vt's resolved default colors and ANSI palette into this theme. Truecolor
/// application output that does not match the terminal's default palette is preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalTheme {
    /// User-facing theme name.
    pub name: String,
    /// Default terminal background.
    pub background: RgbColor,
    /// Default terminal foreground.
    pub foreground: RgbColor,
    /// Cursor color used when the terminal has not supplied one.
    pub cursor: RgbColor,
    /// ANSI 16-color target palette.
    pub palette: [RgbColor; 16],
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self::aardvark_blue()
    }
}

impl TerminalTheme {
    /// Built-in default theme used by Betamax.
    pub fn aardvark_blue() -> Self {
        Self {
            name: "Aardvark Blue".to_string(),
            background: rgb(0x10, 0x20, 0x40),
            foreground: rgb(0xdd, 0xdd, 0xdd),
            cursor: rgb(0x00, 0x7a, 0xcc),
            palette: [
                rgb(0x19, 0x19, 0x19),
                rgb(0xaa, 0x34, 0x2e),
                rgb(0x4b, 0x8c, 0x0f),
                rgb(0xdb, 0xba, 0x00),
                rgb(0x13, 0x70, 0xd3),
                rgb(0xc4, 0x3a, 0xc3),
                rgb(0x00, 0x8e, 0xb0),
                rgb(0xbe, 0xbe, 0xbe),
                rgb(0x45, 0x45, 0x45),
                rgb(0xf0, 0x5b, 0x50),
                rgb(0x95, 0xdc, 0x55),
                rgb(0xff, 0xe7, 0x63),
                rgb(0x60, 0xa4, 0xec),
                rgb(0xe2, 0x6b, 0xe2),
                rgb(0x60, 0xb6, 0xcb),
                rgb(0xf7, 0xf7, 0xf7),
            ],
        }
    }

    /// Loads a theme by name or from inline JSON.
    ///
    /// Lookup trims surrounding whitespace. An empty name returns [`TerminalTheme::default`].
    /// Inline JSON is selected when the trimmed value starts with `{`; missing JSON colors inherit
    /// from [`TerminalTheme::aardvark_blue`]. Non-JSON names are resolved in this order:
    /// user Ghostty theme directories, `GHOSTTY_RESOURCES_DIR`, then Betamax's copied Ghostty
    /// resources. The built-in `Aardvark Blue` theme is available even if resource lookup fails.
    ///
    /// Inline JSON supports these optional string fields: `name`, `background`, `foreground`,
    /// `cursor`, `cursorColor`, and the 16 ANSI color names `black` through `brightWhite`.
    ///
    /// # Examples
    ///
    /// ```
    /// use betamax_core::ghostty::TerminalTheme;
    ///
    /// # fn main() -> betamax_core::Result<()> {
    /// let theme = TerminalTheme::from_name("Aardvark Blue")?;
    /// assert_eq!(theme.background_hex(), "#102040");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when a named theme cannot be found, when a theme file cannot be read, when
    /// an inline JSON theme is malformed, or when any configured color is not `#RRGGBB`.
    pub fn from_name(name: &str) -> Result<Self> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(Self::default());
        }
        if name.starts_with('{') {
            return Self::from_json(name);
        }
        if name.eq_ignore_ascii_case("Aardvark Blue") {
            return Ok(Self::aardvark_blue());
        }
        let Some(path) = find_theme_file(name) else {
            return Err(miette!("theme was not found: {name}").into());
        };
        Self::from_ghostty_file(name, &path)
    }

    /// Return the background color as `#RRGGBB`.
    pub fn background_hex(&self) -> String {
        hex_color(self.background)
    }

    /// Parse a small JSON theme object overlaid on the default theme.
    ///
    /// Missing colors inherit from Aardvark Blue so partial objects are usable in short tapes.
    fn from_json(source: &str) -> Result<Self> {
        let json: JsonTheme = serde_json::from_str(source).into_diagnostic()?;
        let mut theme = Self::default();
        if let Some(name) = json.name.as_ref() {
            theme.name = name.clone();
        }
        if let Some(color) = json.background.as_ref() {
            theme.background = parse_hex_color(color)?;
        }
        if let Some(color) = json.foreground.as_ref() {
            theme.foreground = parse_hex_color(color)?;
        }
        if let Some(color) = json.cursor.as_ref().or(json.cursor_color.as_ref()) {
            theme.cursor = parse_hex_color(color)?;
        }

        for (index, color) in json.palette_entries() {
            if let Some(color) = color {
                theme.palette[index] = parse_hex_color(color)?;
            }
        }

        Ok(theme)
    }

    /// Parse Ghostty's `key = value` theme file format.
    ///
    /// Unknown keys are ignored. Palette entries are expected as `palette = index=#RRGGBB`,
    /// matching Ghostty's resource files.
    fn from_ghostty_file(name: &str, path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .into_diagnostic()
            .map_err(|error| error.wrap_err(format!("failed to read theme {}", path.display())))?;
        let mut theme = Self {
            name: name.to_string(),
            ..Self::default()
        };

        for line in source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "palette" => {
                    let Some((index, color)) = value.split_once('=') else {
                        continue;
                    };
                    let Ok(index) = index.trim().parse::<usize>() else {
                        continue;
                    };
                    if let Some(slot) = theme.palette.get_mut(index) {
                        *slot = parse_hex_color(color.trim())?;
                    }
                }
                "background" => theme.background = parse_hex_color(value)?,
                "foreground" => theme.foreground = parse_hex_color(value)?,
                "cursor-color" | "cursor" => theme.cursor = parse_hex_color(value)?,
                _ => {}
            }
        }

        Ok(theme)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonTheme {
    /// Optional display name overriding the default theme name.
    name: Option<String>,
    black: Option<String>,
    red: Option<String>,
    green: Option<String>,
    yellow: Option<String>,
    blue: Option<String>,
    magenta: Option<String>,
    purple: Option<String>,
    cyan: Option<String>,
    white: Option<String>,
    bright_black: Option<String>,
    bright_red: Option<String>,
    bright_green: Option<String>,
    bright_yellow: Option<String>,
    bright_blue: Option<String>,
    bright_magenta: Option<String>,
    bright_purple: Option<String>,
    bright_cyan: Option<String>,
    bright_white: Option<String>,
    background: Option<String>,
    foreground: Option<String>,
    cursor: Option<String>,
    cursor_color: Option<String>,
}

impl JsonTheme {
    /// Return JSON palette entries in ANSI order.
    ///
    /// Both `magenta` and `purple` spellings are accepted because common terminal theme JSON
    /// formats use both names.
    fn palette_entries(&self) -> [(usize, Option<&str>); 16] {
        [
            (0, self.black.as_deref()),
            (1, self.red.as_deref()),
            (2, self.green.as_deref()),
            (3, self.yellow.as_deref()),
            (4, self.blue.as_deref()),
            (5, self.magenta.as_deref().or(self.purple.as_deref())),
            (6, self.cyan.as_deref()),
            (7, self.white.as_deref()),
            (8, self.bright_black.as_deref()),
            (9, self.bright_red.as_deref()),
            (10, self.bright_green.as_deref()),
            (11, self.bright_yellow.as_deref()),
            (12, self.bright_blue.as_deref()),
            (
                13,
                self.bright_magenta
                    .as_deref()
                    .or(self.bright_purple.as_deref()),
            ),
            (14, self.bright_cyan.as_deref()),
            (15, self.bright_white.as_deref()),
        ]
    }
}

/// Lists available Ghostty theme names.
///
/// Duplicate names are collapsed with earlier directories taking precedence, then the final list is
/// sorted case-insensitively for stable CLI output. Theme files from unreadable directories are
/// skipped; errors from entries inside readable directories are returned.
///
/// # Examples
///
/// ```
/// use betamax_core::ghostty::theme_names;
///
/// # fn main() -> betamax_core::Result<()> {
/// let names = theme_names()?;
/// assert!(names.iter().any(|name| name == "Aardvark Blue"));
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns an error if a selected theme directory entry or file type cannot be inspected.
pub fn theme_names() -> Result<Vec<String>> {
    let mut names = Vec::new();
    for dir in theme_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let entry = entry.into_diagnostic()?;
            if !entry.file_type().into_diagnostic()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".DS_Store" || names.iter().any(|existing| existing == &name) {
                continue;
            }
            names.push(name);
        }
    }
    names.sort_by_key(|name| name.to_ascii_lowercase());
    Ok(names)
}

/// Find the first theme file matching a name in the configured theme search path.
fn find_theme_file(name: &str) -> Option<PathBuf> {
    theme_dirs()
        .into_iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

/// Return theme search directories in precedence order.
///
/// User configuration is searched before bundled resources so users can override copied themes with
/// local edits.
fn theme_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        dirs.push(PathBuf::from(config_home).join("ghostty/themes"));
    }
    if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".config/ghostty/themes"));
    }
    if let Some(resources) = env::var_os("GHOSTTY_RESOURCES_DIR") {
        dirs.push(PathBuf::from(resources).join("themes"));
    }
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/ghostty/themes"));
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_copied_ghostty_themes() {
        let names = theme_names().unwrap();

        assert!(names.iter().any(|name| name == "Aardvark Blue"));
        assert!(names.iter().any(|name| name == "Dracula"));
    }

    #[test]
    fn loads_copied_ghostty_theme_file() {
        let theme = TerminalTheme::from_name("Dracula").unwrap();

        assert_eq!(theme.name, "Dracula");
        assert_eq!(theme.background, rgb(0x28, 0x2a, 0x36));
        assert_eq!(theme.foreground, rgb(0xf8, 0xf8, 0xf2));
        assert_eq!(theme.palette[4], rgb(0xbd, 0x93, 0xf9));
    }
}
