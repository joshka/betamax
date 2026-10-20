//! Startup configuration and execution timing for the runner.
//!
//! Apply tape settings in source order, then derive the PTY grid from presentation geometry.
//! Feature modules own decoration and overlay behavior; this module owns startup dispatch and
//! diagnostics for unknown settings and incorrect value kinds.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use miette::miette;
use regex::Regex;

use super::keyboard_overlay::{parse_keyboard_overlay_location, parse_keyboard_overlay_mode};
use super::presentation::Presentation;
use super::preset::Preset;
use crate::ghostty::TerminalTheme;
use crate::shell::DEFAULT_SHELL;
use crate::tape::{Command, Tape, Value, WaitPattern};
use crate::wait::regex_source;
use crate::Result;

/// Default capture cadence in frames per second.
const DEFAULT_FRAMERATE: u16 = 50;
/// Default delay between characters typed by `Type`.
const DEFAULT_TYPING_DELAY: Duration = Duration::from_millis(50);
/// Default playback multiplier.
const DEFAULT_PLAYBACK_SPEED: f64 = 1.0;
/// Smallest accepted playback multiplier.
///
/// This prevents `Set PlaybackSpeed 0` from creating infinite frame delays while still allowing
/// very slow playback for debugging.
const MIN_PLAYBACK_SPEED: f64 = 0.01;
/// Default loop offset.
const DEFAULT_LOOP_OFFSET: f64 = 0.0;
/// Default timeout for wait commands.
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
/// Default wait pattern matching Betamax's VHS-style prompt.
const DEFAULT_WAIT_PATTERN: &str = ">$";
/// Minimum effective frame rate used in timing calculations.
const MIN_EFFECTIVE_FRAMERATE: u16 = 1;
/// Cursor blink uses a two-phase on/off cycle.
const CURSOR_BLINK_PHASES: usize = 2;

/// Startup execution settings and the presentation configuration for one tape run.
///
/// Values are produced by [`Settings::from_tape_in`]. Callers should not construct this type
/// manually because `columns` and `rows` are derived from several pixel/text settings after all
/// tape commands have been applied.
#[derive(Debug, Clone)]
pub(super) struct Settings {
    /// Shell command and optional explicit arguments from `Set Shell`.
    pub(super) shell: Vec<OsString>,
    /// Output geometry, terminal appearance, and decoration configuration.
    pub(super) presentation: Presentation,
    /// Terminal grid columns derived from canvas width, padding, and text metrics.
    pub(super) columns: u16,
    /// Terminal grid rows derived from canvas height, padding, and text metrics.
    pub(super) rows: u16,
    /// Capture cadence in frames per second.
    pub(super) framerate: u16,
    /// Playback multiplier applied to output frame delays, not to PTY execution timing.
    pub(super) playback_speed: f64,
    /// Animated-output rotation offset.
    ///
    /// Values in `0.0..=1.0` are treated as a fraction of the frame count. Other values are
    /// treated as seconds at the capture framerate.
    pub(super) loop_offset: f64,
    /// Default per-character delay for `Type`.
    pub(super) typing_delay: Duration,
    /// Whether captured frames should blink the cursor according to the output framerate.
    pub(super) cursor_blink: bool,
    /// Default wait pattern used when a wait command does not provide one.
    pub(super) wait_pattern: WaitPattern,
    /// Default timeout for wait commands.
    pub(super) wait_timeout: Duration,
    /// Tape-provided environment overrides applied after Betamax defaults.
    pub(super) env: Vec<(String, String)>,
    /// Pixel or cell sizing chosen by the tape for each axis.
    sizing: Sizing,
}

/// How the tape sized each axis.
///
/// Presets set pixel defaults without claiming an axis, so `Set Rows` can follow any preset. An
/// explicit `Set Width` or `Set Height` conflicts with cells on the same axis, matching VHS.
#[derive(Debug, Clone, Copy, Default)]
struct Sizing {
    width: AxisSizing,
    height: AxisSizing,
}

/// Sizing source for one output axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum AxisSizing {
    /// Fit cells into the preset or default pixel size.
    #[default]
    Default,
    /// Fit cells into an explicit `Set Width` or `Set Height`.
    Pixels,
    /// Grow the output to hold exactly this many cells.
    Cells(u16),
}

impl AxisSizing {
    /// Record a pixel setting, rejecting a cell setting on the same axis.
    fn set_pixels(&mut self, pixels: &str, cells: &str) -> Result<()> {
        if matches!(self, Self::Cells(_)) {
            return Err(both_sizes_error(pixels, cells));
        }
        *self = Self::Pixels;
        Ok(())
    }

    /// Record a cell setting, rejecting a pixel setting on the same axis.
    fn set_cells(&mut self, value: f64, pixels: &str, cells: &str) -> Result<()> {
        if *self == Self::Pixels {
            return Err(both_sizes_error(pixels, cells));
        }
        if value.fract() != 0.0 || !(1.0..=f64::from(u16::MAX)).contains(&value) {
            return Err(miette!(
                "Set {cells} expects a whole number of cells in 1..={}, got {value}",
                u16::MAX
            )
            .into());
        }
        *self = Self::Cells(value as u16);
        Ok(())
    }

    fn cells(self) -> Option<u16> {
        match self {
            Self::Cells(cells) => Some(cells),
            _ => None,
        }
    }
}

/// Explain a pixel and cell setting that size the same axis.
fn both_sizes_error(pixels: &str, cells: &str) -> crate::Error {
    miette!("cannot set both {pixels} and {cells}; {cells} sizes the output to fit the grid").into()
}

impl Settings {
    /// [`Settings::from_tape_in`] without a tape directory.
    #[cfg(test)]
    pub(super) fn from_tape(tape: &Tape) -> Result<Self> {
        Self::from_tape_in(tape, None)
    }

    /// Build execution settings by applying startup commands in tape order.
    ///
    /// The Default preset selects Framed; explicit settings override the selected preset.
    /// After all settings are applied, derive the terminal grid from resolved pixel/text geometry.
    ///
    /// Apply `Set` and `Env`, and reserve caption space when the tape uses captions.
    /// Output paths, waits, key presses, and
    /// runtime commands are intentionally ignored because they are handled by output classification
    /// and command execution. This keeps startup settings deterministic and avoids side effects
    /// while validating tape configuration.
    ///
    /// Relative asset paths such as a `MarginFill` image resolve against the working directory,
    /// then `tape_dir`.
    pub(super) fn from_tape_in(tape: &Tape, tape_dir: Option<&Path>) -> Result<Self> {
        let theme = TerminalTheme::default();
        let mut settings = Self {
            shell: vec![OsString::from(DEFAULT_SHELL)],
            columns: 0,
            rows: 0,
            framerate: DEFAULT_FRAMERATE,
            playback_speed: DEFAULT_PLAYBACK_SPEED,
            loop_offset: DEFAULT_LOOP_OFFSET,
            typing_delay: DEFAULT_TYPING_DELAY,
            presentation: Presentation::new(theme)?,
            cursor_blink: true,
            wait_pattern: WaitPattern::Regex(DEFAULT_WAIT_PATTERN.to_string()),
            wait_timeout: DEFAULT_WAIT_TIMEOUT,
            env: Vec::new(),
            sizing: Sizing::default(),
        };

        let preset = Preset::from_tape(tape)?;
        preset.apply(&mut settings.presentation)?;

        for command in &tape.commands {
            match command {
                Command::Env { key, value } => settings.env.push((key.clone(), value.clone())),
                Command::Set { key, value } if key != "Preset" => settings.apply_set(key, value)?,
                Command::Caption(text) if !text.trim().is_empty() => {
                    settings.presentation.caption_overlay = true;
                }
                _ => {}
            }
        }

        preset.resolve_surfaces(&mut settings.presentation);
        settings.apply_terminal_grid();
        settings.presentation.prepare_backdrop(tape_dir)?;
        Ok(settings)
    }

    /// Apply one `Set` command to the runtime settings.
    ///
    /// Unknown and type-mismatched settings are errors. That makes typoed tapes fail before PTY
    /// startup instead of silently rendering with defaults.
    ///
    /// Numeric conversions currently follow VHS's permissive behavior by truncating `f64` tape
    /// values into integer pixel and frame settings. Validation is deliberately stricter for known
    /// setting names: `Set Width "1200"` is rejected rather than interpreted as a string.
    fn apply_set(&mut self, key: &str, value: &Value) -> Result<()> {
        match (key, value) {
            ("Shell", Value::String(shell)) => {
                let parts = shell_words::split(shell)
                    .map_err(|error| miette!("invalid Shell setting `{shell}`: {error}"))?;
                self.shell = parts.into_iter().map(OsString::from).collect();
            }
            ("Width", Value::Number(width)) => {
                self.sizing.width.set_pixels("Width", "Columns")?;
                self.presentation.width = *width as u32;
            }
            ("Height", Value::Number(height)) => {
                self.sizing.height.set_pixels("Height", "Rows")?;
                self.presentation.height = *height as u32;
            }
            ("Columns", Value::Number(columns)) => {
                self.sizing.width.set_cells(*columns, "Width", "Columns")?;
            }
            ("Rows", Value::Number(rows)) => {
                self.sizing.height.set_cells(*rows, "Height", "Rows")?;
            }
            ("FontSize", Value::Number(font_size)) => {
                self.presentation.text.font_size = *font_size as f32
            }
            ("FontFamily", Value::String(font_family)) => {
                self.presentation.text.font_family = Some(font_family.clone());
            }
            ("LetterSpacing", Value::Number(letter_spacing)) => {
                self.presentation.text.letter_spacing = *letter_spacing as f32;
            }
            ("LineHeight", Value::Number(line_height)) => {
                self.presentation.text.line_height = *line_height as f32
            }
            ("Padding", Value::Number(padding)) => self.presentation.text.padding = *padding as u32,
            ("Framerate", Value::Number(framerate)) => self.framerate = *framerate as u16,
            ("TypingSpeed", Value::Duration(duration)) => self.typing_delay = *duration,
            ("TypingSpeed", Value::Number(seconds)) => {
                self.typing_delay = seconds_duration(*seconds)?
            }
            ("PlaybackSpeed", Value::Number(playback_speed)) => {
                self.playback_speed = (*playback_speed).max(MIN_PLAYBACK_SPEED);
            }
            ("LoopOffset", Value::Number(loop_offset)) => self.loop_offset = *loop_offset,
            ("Margin", Value::Number(margin)) => self.presentation.style.margin = *margin as u32,
            ("MarginFill", Value::String(margin_fill)) => {
                self.presentation.style.margin_fill = Some(margin_fill.clone());
            }
            ("WindowBar", Value::String(window_bar)) => {
                self.presentation.style.window_bar = window_bar.clone()
            }
            ("WindowBarColor", Value::String(color)) => {
                self.presentation.style.window_bar_color_override = Some(color.clone());
            }
            ("WindowBarSize", Value::Number(window_bar_size)) => {
                self.presentation.style.window_bar_size = *window_bar_size as u32;
            }
            ("WindowTitleFromTerminal", Value::Bool(enabled)) => {
                self.presentation.style.window_title.follow_terminal = *enabled;
            }
            ("WindowTitle", Value::String(title)) => {
                self.presentation.style.window_title.text = title.clone();
            }
            ("WindowTitleColor", Value::String(color)) => {
                self.presentation.style.window_title.color = color.clone();
            }
            ("BorderRadius", Value::Number(border_radius)) => {
                self.presentation.style.border_radius = *border_radius as u32;
            }
            ("WindowBorder", Value::Number(width)) => {
                self.presentation.style.border.set_width(*width)?;
            }
            ("WindowBorderColor", Value::String(color)) => {
                self.presentation.style.border.set_color(color)?;
            }
            ("WindowBorderOpacity", Value::Number(opacity)) => {
                self.presentation.style.border.set_opacity(*opacity)?;
            }
            ("CursorBlink", Value::Bool(cursor_blink)) => self.cursor_blink = *cursor_blink,
            ("Shadow", Value::Bool(shadow)) => self.presentation.style.shadow.enabled = *shadow,
            ("ShadowBlur", Value::Number(blur)) => {
                self.presentation.style.shadow.set_blur(*blur)?
            }
            ("ShadowOffset", Value::Number(offset)) => {
                self.presentation.style.shadow.set_offset(*offset)?
            }
            ("ShadowOpacity", Value::Number(opacity)) => {
                self.presentation.style.shadow.set_opacity(*opacity)?
            }
            ("KeyboardOverlay", Value::String(mode)) => {
                self.presentation.keyboard_overlay.mode = parse_keyboard_overlay_mode(mode)?;
            }
            ("KeyboardOverlayLocation", Value::String(location)) => {
                self.presentation.keyboard_overlay.location =
                    parse_keyboard_overlay_location(location)?;
            }
            ("WaitTimeout", Value::Duration(wait_timeout)) => self.wait_timeout = *wait_timeout,
            ("WaitTimeout", Value::Number(seconds)) => {
                self.wait_timeout = seconds_duration(*seconds)?
            }
            ("WaitPattern", Value::String(wait_pattern)) => {
                let wait_pattern = regex_source(wait_pattern);
                Regex::new(&wait_pattern)
                    .map_err(|error| miette!("invalid WaitPattern `{wait_pattern}`: {error}"))?;
                self.wait_pattern = WaitPattern::Regex(wait_pattern);
            }
            ("Theme", Value::String(theme)) => {
                let theme = TerminalTheme::from_name(theme)?;
                self.presentation.style.window_bar_color = theme.background_hex();
                self.presentation.theme = theme;
            }
            (known, value) if known_setting(known) => {
                return Err(miette!(
                    "Set {known} expects {}, got {}",
                    setting_expected_type(known),
                    value_kind(value)
                )
                .into());
            }
            (unknown, _) => return Err(miette!("unsupported Set setting `{unknown}`").into()),
        }
        Ok(())
    }

    /// Derive terminal rows and columns from pixel dimensions.
    ///
    /// `Width` and `Height` describe the final output frame, matching VHS. Margin and window bar
    /// decoration are subtracted first, then text padding is subtracted from the terminal canvas.
    /// Each dimension is clamped to at least one cell so pathological settings still create a valid
    /// PTY.
    ///
    /// `Columns` and `Rows` first grow the output on their axis so the grid fits exactly, then
    /// round up to an even pixel count for video encoders, like VHS. The spare pixel is smaller
    /// than a cell, so the fitted grid is unchanged.
    fn apply_terminal_grid(&mut self) {
        let padding = self.presentation.text.padding.saturating_mul(2);
        if let Some(columns) = self.sizing.width.cells() {
            let canvas = u32::from(columns)
                .saturating_mul(self.presentation.text.cell_width())
                .saturating_add(padding);
            self.presentation.width =
                round_up_to_even(self.presentation.frame_width_for_canvas(canvas));
        }
        if let Some(rows) = self.sizing.height.cells() {
            let canvas = u32::from(rows)
                .saturating_mul(self.presentation.text.cell_height())
                .saturating_add(padding);
            self.presentation.height =
                round_up_to_even(self.presentation.frame_height_for_canvas(canvas));
        }
        let inner_width = self
            .presentation
            .terminal_canvas_width()
            .saturating_sub(self.presentation.text.padding.saturating_mul(2));
        let inner_height = self
            .presentation
            .terminal_canvas_height()
            .saturating_sub(self.presentation.text.padding.saturating_mul(2));
        self.columns = fit_cells(inner_width, self.presentation.text.cell_width());
        self.rows = fit_cells(inner_height, self.presentation.text.cell_height());
    }

    /// Delay stored on each output frame after playback-speed adjustment.
    ///
    /// This duration is used by GIF/video/PNG sequence frame lists. It is not used to sleep while
    /// executing commands; command execution timing is controlled by `capture_interval`,
    /// `typing_delay`, explicit sleeps, and wait timeouts.
    pub(super) fn frame_delay(&self) -> Duration {
        self.playback_delay(self.capture_interval())
    }

    /// Convert elapsed capture time to an output hold without changing PTY execution timing.
    pub(super) fn playback_delay(&self, elapsed: Duration) -> Duration {
        elapsed.div_f64(self.playback_speed)
    }

    /// Real capture cadence used while draining the PTY.
    ///
    /// This deliberately ignores playback speed so faster/slower output changes animation timing
    /// without changing command execution or how many terminal states are sampled.
    pub(super) fn capture_interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.framerate.max(MIN_EFFECTIVE_FRAMERATE)))
    }

    /// Effective video/GIF framerate after playback-speed adjustment.
    ///
    /// The value may be fractional because ffmpeg accepts fractional input rates and because GIF
    /// delay conversion happens later in [`crate::media::write_gif`].
    pub(super) fn output_framerate(&self) -> f64 {
        f64::from(self.framerate.max(MIN_EFFECTIVE_FRAMERATE)) * self.playback_speed
    }

    /// Determine whether the cursor should be visible in a captured frame.
    ///
    /// Betamax simulates blinking at a half-second period based on the configured framerate. If
    /// cursor blinking is disabled, every frame renders the cursor when libghostty-vt reports it as
    /// visible.
    pub(super) fn cursor_visible(&self, frame_index: usize) -> bool {
        if !self.cursor_blink {
            return true;
        }
        let half_period =
            (usize::from(self.framerate.max(MIN_EFFECTIVE_FRAMERATE)) / CURSOR_BLINK_PHASES).max(1);
        (frame_index / half_period).is_multiple_of(CURSOR_BLINK_PHASES)
    }

    /// Rotate animated output frames to change the loop boundary.
    ///
    /// This is applied after the final frame has been appended. Static outputs ignore loop offset.
    /// Fractional offsets in `0.0..=1.0` mean a percentage of the frame list. Other values are
    /// interpreted as seconds at the capture framerate to match VHS's time-based option.
    pub(super) fn apply_loop_offset<T>(&self, frames: &mut [(T, Duration)]) {
        if frames.len() < 2 || self.loop_offset == 0.0 {
            return;
        }
        let len = frames.len();
        let offset = if (0.0..=1.0).contains(&self.loop_offset) {
            (self.loop_offset * len as f64).round() as usize
        } else {
            (self.loop_offset * f64::from(self.framerate.max(MIN_EFFECTIVE_FRAMERATE))).round()
                as usize
        };
        frames.rotate_left(offset % len);
    }
}

/// Fit as many terminal cells as possible in a pixel dimension.
///
/// The result is always at least one cell. Extremely large dimensions saturate to `u16::MAX`,
/// matching the PTY sizing type used by `portable-pty`.
fn fit_cells(pixels: u32, cell_pixels: u32) -> u16 {
    let cells = (pixels / cell_pixels.max(1)).max(1);
    u16::try_from(cells).unwrap_or(u16::MAX)
}

/// Round a pixel dimension up to an even count, which MP4 and WebM encoders require.
fn round_up_to_even(pixels: u32) -> u32 {
    pixels.saturating_add(pixels % 2)
}

/// Return whether a key is a recognized `Set` setting.
///
/// This is separate from `apply_set`'s main match so diagnostics can distinguish unknown setting
/// names from known settings with the wrong value kind.
fn known_setting(key: &str) -> bool {
    matches!(
        key,
        "Shell"
            | "Width"
            | "Height"
            | "Columns"
            | "Rows"
            | "FontSize"
            | "FontFamily"
            | "LetterSpacing"
            | "LineHeight"
            | "Padding"
            | "Framerate"
            | "TypingSpeed"
            | "PlaybackSpeed"
            | "LoopOffset"
            | "Margin"
            | "MarginFill"
            | "WindowBar"
            | "WindowBarColor"
            | "WindowBarSize"
            | "WindowBorder"
            | "WindowBorderColor"
            | "WindowBorderOpacity"
            | "WindowTitleFromTerminal"
            | "WindowTitle"
            | "WindowTitleColor"
            | "BorderRadius"
            | "Shadow"
            | "ShadowBlur"
            | "ShadowOffset"
            | "ShadowOpacity"
            | "CursorBlink"
            | "KeyboardOverlay"
            | "KeyboardOverlayLocation"
            | "WaitTimeout"
            | "WaitPattern"
            | "Theme"
    )
}

/// Return the expected tape value kind for a supported `Set` key.
fn setting_expected_type(key: &str) -> &'static str {
    match key {
        "Shell" | "FontFamily" | "MarginFill" | "WindowBar" | "WindowBarColor" | "WaitPattern"
        | "Theme" | "WindowBorderColor" | "WindowTitle" | "WindowTitleColor" => "string",
        "Width"
        | "Height"
        | "Columns"
        | "Rows"
        | "FontSize"
        | "LetterSpacing"
        | "LineHeight"
        | "Padding"
        | "Framerate"
        | "PlaybackSpeed"
        | "LoopOffset"
        | "Margin"
        | "WindowBarSize"
        | "WindowBorder"
        | "WindowBorderOpacity"
        | "BorderRadius"
        | "ShadowBlur"
        | "ShadowOffset"
        | "ShadowOpacity" => "number",
        "TypingSpeed" => "duration or number of seconds",
        "CursorBlink" | "Shadow" | "WindowTitleFromTerminal" => "bool",
        "KeyboardOverlay" => "Off, Keys, Input, or All",
        "KeyboardOverlayLocation" => "CaptionRow, TopLeft, TopRight, BottomLeft, or BottomRight",
        "WaitTimeout" => "duration or number of seconds",
        _ => "known value",
    }
}

/// Convert a bare-number duration setting to seconds, matching VHS.
fn seconds_duration(seconds: f64) -> Result<Duration> {
    Duration::try_from_secs_f64(seconds)
        .map_err(|_| miette!("duration must be zero or positive: `{seconds}`").into())
}

/// Name the parsed tape value kind for diagnostics.
fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Duration(_) => "duration",
        Value::Bool(_) => "bool",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::keyboard_overlay::{KeyboardOverlayLocation, KeyboardOverlayMode};
    #[test]
    fn uses_vhs_typography_defaults() {
        let tape = Tape::parse("Set Preset Default-0.1\n").unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.presentation.width, 1200);
        assert_eq!(settings.presentation.height, 600);
        assert_eq!(settings.framerate, 50);
        assert_eq!(settings.typing_delay, Duration::from_millis(50));
        assert_eq!(settings.presentation.text.font_size, 22.0);
        assert_eq!(
            settings.presentation.text.font_family.as_deref(),
            Some("JetBrains Mono")
        );
        assert_eq!(settings.presentation.text.letter_spacing, 1.0);
        assert_eq!(settings.presentation.text.line_height, 1.0);
        assert_eq!(settings.presentation.text.padding, 60);
        assert_eq!(settings.presentation.theme.name, "Aardvark Blue");
        assert_eq!(settings.presentation.style.margin, 0);
        assert_eq!(settings.presentation.style.margin_fill, None);
        assert_eq!(settings.presentation.style.window_bar, "");
        assert_eq!(settings.presentation.style.window_bar_size, 30);
        assert_eq!(settings.presentation.style.window_bar_color, "#102040");
        assert_eq!(settings.presentation.style.border_radius, 0);
        assert_eq!(
            settings.presentation.keyboard_overlay.mode,
            KeyboardOverlayMode::Off
        );
        assert_eq!(
            settings.presentation.keyboard_overlay.location,
            KeyboardOverlayLocation::CaptionRow
        );
    }

    #[test]
    fn shell_defaults_to_bash_regardless_of_host_shell() {
        let tape = Tape::parse("Type \"ls\"\n").unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.shell, [OsString::from("bash")]);
    }

    #[test]
    fn accepts_aardvark_blue_theme() {
        let tape = Tape::parse(
            r#"Set Preset Default-0.1
Set Theme "Aardvark Blue""#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.presentation.theme.name, "Aardvark Blue");
    }

    fn grid(source: &str) -> Settings {
        Settings::from_tape(&Tape::parse(source).unwrap()).unwrap()
    }

    fn grid_error(source: &str) -> String {
        Settings::from_tape(&Tape::parse(source).unwrap())
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn bare_typing_speed_and_wait_timeout_are_seconds() {
        for (source, typing_delay) in [
            ("Set TypingSpeed 0.1", Duration::from_millis(100)),
            ("Set TypingSpeed 0", Duration::ZERO),
            ("Set TypingSpeed 100ms", Duration::from_millis(100)),
        ] {
            let settings = Settings::from_tape(&Tape::parse(source).unwrap()).unwrap();
            assert_eq!(settings.typing_delay, typing_delay, "{source:?}");
        }
        let settings = Settings::from_tape(&Tape::parse("Set WaitTimeout 2.5").unwrap()).unwrap();
        assert_eq!(settings.wait_timeout, Duration::from_millis(2500));
    }

    #[test]
    fn rejects_negative_bare_duration_settings() {
        for source in ["Set TypingSpeed -0.1", "Set WaitTimeout -1"] {
            assert!(
                grid_error(source).contains("must be zero or positive"),
                "{source:?}"
            );
        }
        assert!(grid_error("Set TypingSpeed fast")
            .contains("Set TypingSpeed expects duration or number of seconds"));
    }

    #[test]
    fn columns_and_rows_size_the_grid_exactly() {
        for source in [
            "",
            "Set Preset Bare",
            "Set Preset Inset",
            "Set Preset Default-0.1",
            "Set FontSize 31\nSet LineHeight 1.3\nSet LetterSpacing 1.5",
            "Set Padding 17\nSet Margin 9",
            "Set WindowBar Rings\nSet WindowBarSize 41",
            "Set KeyboardOverlay Keys",
        ] {
            let settings = grid(&format!(
                "{source}\nSet Columns 83\nSet Rows 21\nCaption hi"
            ));
            assert_eq!((settings.columns, settings.rows), (83, 21), "{source:?}");
            assert_eq!(settings.presentation.width % 2, 0, "{source:?}");
            assert_eq!(settings.presentation.height % 2, 0, "{source:?}");
        }
    }

    #[test]
    fn columns_add_padding_and_margin_to_the_output_width() {
        let settings = grid("Set Preset Bare\nSet Padding 10\nSet Columns 80");
        let cell_width = settings.presentation.text.cell_width();

        let width = 80 * cell_width + 2 * 10;
        assert_eq!(settings.presentation.width, width + width % 2);
        assert_eq!(settings.presentation.height, 720);
    }

    #[test]
    fn one_cell_axis_leaves_the_other_axis_in_pixels() {
        let only_rows = grid("Set Width 900\nSet Rows 12");
        let pixels = grid("Set Width 900");
        assert_eq!(only_rows.rows, 12);
        assert_eq!(only_rows.columns, pixels.columns);
        assert_eq!(only_rows.presentation.width, 900);

        let only_columns = grid("Set Height 500\nSet Columns 40");
        assert_eq!(only_columns.columns, 40);
        assert_eq!(only_columns.presentation.height, 500);
    }

    #[test]
    fn pixel_and_cell_sizes_on_one_axis_conflict_in_either_order() {
        for (source, message) in [
            (
                "Set Width 900\nSet Columns 80",
                "cannot set both Width and Columns",
            ),
            (
                "Set Columns 80\nSet Width 900",
                "cannot set both Width and Columns",
            ),
            (
                "Set Height 500\nSet Rows 20",
                "cannot set both Height and Rows",
            ),
            (
                "Set Rows 20\nSet Height 500",
                "cannot set both Height and Rows",
            ),
        ] {
            assert!(grid_error(source).contains(message), "{source:?}");
        }
    }

    #[test]
    fn rejects_cell_counts_that_are_not_positive_whole_numbers() {
        for source in [
            "Set Columns 0",
            "Set Rows -1",
            "Set Rows 2.5",
            "Set Columns 65536",
        ] {
            assert!(
                grid_error(source).contains("whole number of cells"),
                "{source:?}"
            );
        }
        assert!(grid_error("Set Rows wide").contains("Set Rows expects number"));
    }

    #[test]
    fn unknown_settings_are_errors() {
        let tape = Tape::parse("Set Preset Default-0.1\nSet Wdith 900").unwrap();
        let error = Settings::from_tape(&tape).unwrap_err().to_string();

        assert!(error.contains("unsupported Set setting `Wdith`"));
    }

    #[test]
    fn type_mismatched_settings_are_errors() {
        let tape = Tape::parse(
            r#"Set Preset Default-0.1
Set Width "wide""#,
        )
        .unwrap();
        let error = Settings::from_tape(&tape).unwrap_err().to_string();

        assert!(error.contains("Set Width expects number, got string"));
    }

    #[test]
    fn playback_speed_changes_output_delay_not_capture_cadence() {
        for speed in [0.5, 1.0, 2.0] {
            let tape = Tape::parse(&format!(
                "Set Preset Default-0.1\nSet Framerate 50\nSet PlaybackSpeed {speed}"
            ))
            .unwrap();
            let settings = Settings::from_tape(&tape).unwrap();

            assert_eq!(settings.capture_interval(), Duration::from_millis(20));
            assert_eq!(
                settings.frame_delay(),
                Duration::from_secs_f64(0.02 / speed)
            );
            assert_eq!(settings.output_framerate(), 50.0 * speed);
            assert_eq!(
                settings.playback_delay(Duration::from_millis(137)),
                Duration::from_secs_f64(0.137 / speed)
            );
        }
    }
}
