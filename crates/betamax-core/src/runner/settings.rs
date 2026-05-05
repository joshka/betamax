//! Runtime settings and frame decoration for the runner.
//!
//! The tape parser keeps `Set` commands as loosely typed syntax. This module is the place where
//! those syntax values become executable runner configuration: shell argv, terminal dimensions,
//! typography, timing, theme, wait defaults, environment overrides, and VHS-style output chrome.
//! Keeping this logic outside `runner/mod.rs` makes the runner loop easier to audit because all
//! setting validation and default behavior has one home.
//!
//! Settings in this file are intentionally crate-private. They describe the current CLI/runtime
//! contract rather than a stable library API. If Betamax later exposes a reusable library surface,
//! this module is a good candidate to split into public builder types plus private derived values.

use std::env;
use std::ffi::OsString;
use std::time::Duration;

use miette::miette;
use regex::Regex;

use crate::ghostty::{TerminalTheme, TextSettings};
use crate::media::{Frame, PixelFormat};
use crate::tape::{Command, Tape, Value, WaitPattern};
use crate::wait::regex_source;
use crate::Result;

/// Default raw terminal canvas width in pixels.
const DEFAULT_WIDTH: u32 = 1200;
/// Default raw terminal canvas height in pixels.
const DEFAULT_HEIGHT: u32 = 600;
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
/// Default synthetic window bar height in pixels.
const DEFAULT_WINDOW_BAR_SIZE: u32 = 30;
/// Number of bytes in one RGBA pixel.
const BYTES_PER_PIXEL: usize = 4;
/// Fully opaque alpha byte.
const OPAQUE_ALPHA: u8 = 0xff;
/// Window-button radius divisor relative to bar height.
const WINDOW_BUTTON_RADIUS_DIVISOR: u32 = 6;
/// Minimum window-button radius in pixels.
const MIN_WINDOW_BUTTON_RADIUS: u32 = 2;
/// Spacing between synthetic window buttons as a multiple of radius.
const WINDOW_BUTTON_GAP_RADIUS_MULTIPLIER: u32 = 3;
/// Minimum effective frame rate used in timing calculations.
const MIN_EFFECTIVE_FRAMERATE: u16 = 1;
/// Cursor blink uses a two-phase on/off cycle.
const CURSOR_BLINK_PHASES: usize = 2;

/// Shell command and all derived execution, rendering, timing, and styling settings.
///
/// Values are produced by [`Settings::from_tape`]. Callers should not construct this type manually
/// because `columns` and `rows` are derived from several pixel/text settings after all tape
/// commands have been applied.
#[derive(Debug, Clone)]
pub(super) struct Settings {
    /// Shell command and optional explicit arguments from `Set Shell`.
    pub(super) shell: Vec<OsString>,
    /// Output canvas width before margin/window decoration.
    pub(super) width: u32,
    /// Output canvas height before margin/window decoration.
    pub(super) height: u32,
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
    /// Font, cell, and terminal padding settings.
    pub(super) text: TextSettings,
    /// Loaded terminal color theme.
    pub(super) theme: TerminalTheme,
    /// Outer frame decoration settings.
    pub(super) style: StyleSettings,
    /// Whether captured frames should blink the cursor according to the output framerate.
    pub(super) cursor_blink: bool,
    /// Default wait pattern used when a wait command does not provide one.
    pub(super) wait_pattern: WaitPattern,
    /// Default timeout for wait commands.
    pub(super) wait_timeout: Duration,
    /// Tape-provided environment overrides applied after Betamax defaults.
    pub(super) env: Vec<(String, String)>,
}

/// VHS-style frame decoration around the raw terminal capture.
///
/// These values do not affect PTY size or libghostty-vt state. They are applied after a raw
/// terminal frame has been rendered, so screenshots, GIFs, videos, and PNG sequences all share the
/// same decoration behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StyleSettings {
    /// Outer margin around the captured terminal, in pixels.
    pub(super) margin: u32,
    /// Margin fill color as `#RRGGBB`; invalid values fall back to the theme background.
    pub(super) margin_fill: String,
    /// Window button mode. Empty disables the bar; non-empty draws traffic-light buttons.
    pub(super) window_bar: String,
    /// Height of the synthetic window bar when enabled.
    pub(super) window_bar_size: u32,
    /// Window bar fill color as `#RRGGBB`; invalid values fall back to the theme background.
    pub(super) window_bar_color: String,
    /// Radius for masking the captured terminal plus window bar.
    pub(super) border_radius: u32,
}

impl StyleSettings {
    /// Construct style defaults from the current theme.
    ///
    /// The theme background is duplicated into string fields because tape settings can later
    /// replace them with raw user-provided values that are parsed only during frame decoration.
    fn new(theme: &TerminalTheme) -> Self {
        let background = theme.background_hex();
        Self {
            margin: 0,
            margin_fill: background.clone(),
            window_bar: String::new(),
            window_bar_size: DEFAULT_WINDOW_BAR_SIZE,
            window_bar_color: background,
            border_radius: 0,
        }
    }
}

impl Settings {
    /// Build execution settings by applying startup commands in tape order.
    ///
    /// Defaults are intentionally close to VHS output: 1200x600, 50 FPS, 50 ms typing delay, a
    /// prompt-oriented wait pattern, and the Aardvark Blue theme. After all settings are applied
    /// the terminal grid is derived from pixel dimensions and text metrics.
    ///
    /// Only `Set` and `Env` commands are consumed here. Output paths, waits, key presses, and
    /// runtime commands are intentionally ignored because they are handled by output classification
    /// and command execution. This keeps startup settings deterministic and avoids side effects
    /// while validating tape configuration.
    pub(super) fn from_tape(tape: &Tape) -> Result<Self> {
        let theme = TerminalTheme::default();
        let mut settings = Self {
            shell: vec![env::var_os("SHELL").unwrap_or_else(|| OsString::from("sh"))],
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            columns: 0,
            rows: 0,
            framerate: DEFAULT_FRAMERATE,
            playback_speed: DEFAULT_PLAYBACK_SPEED,
            loop_offset: DEFAULT_LOOP_OFFSET,
            typing_delay: DEFAULT_TYPING_DELAY,
            text: TextSettings::default(),
            style: StyleSettings::new(&theme),
            theme,
            cursor_blink: true,
            wait_pattern: WaitPattern::Regex(DEFAULT_WAIT_PATTERN.to_string()),
            wait_timeout: DEFAULT_WAIT_TIMEOUT,
            env: Vec::new(),
        };

        for command in &tape.commands {
            match command {
                Command::Env { key, value } => settings.env.push((key.clone(), value.clone())),
                Command::Set { key, value } => settings.apply_set(key, value)?,
                _ => {}
            }
        }

        settings.apply_terminal_grid();
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
            ("Width", Value::Number(width)) => self.width = *width as u32,
            ("Height", Value::Number(height)) => self.height = *height as u32,
            ("FontSize", Value::Number(font_size)) => self.text.font_size = *font_size as f32,
            ("FontFamily", Value::String(font_family)) => {
                self.text.font_family = Some(font_family.clone());
            }
            ("LetterSpacing", Value::Number(letter_spacing)) => {
                self.text.letter_spacing = *letter_spacing as f32;
            }
            ("LineHeight", Value::Number(line_height)) => {
                self.text.line_height = *line_height as f32
            }
            ("Padding", Value::Number(padding)) => self.text.padding = *padding as u32,
            ("Framerate", Value::Number(framerate)) => self.framerate = *framerate as u16,
            ("TypingSpeed", Value::Duration(duration)) => self.typing_delay = *duration,
            ("PlaybackSpeed", Value::Number(playback_speed)) => {
                self.playback_speed = (*playback_speed).max(MIN_PLAYBACK_SPEED);
            }
            ("LoopOffset", Value::Number(loop_offset)) => self.loop_offset = *loop_offset,
            ("Margin", Value::Number(margin)) => self.style.margin = *margin as u32,
            ("MarginFill", Value::String(margin_fill)) => {
                self.style.margin_fill = margin_fill.clone();
            }
            ("WindowBar", Value::String(window_bar)) => self.style.window_bar = window_bar.clone(),
            ("WindowBarSize", Value::Number(window_bar_size)) => {
                self.style.window_bar_size = *window_bar_size as u32;
            }
            ("BorderRadius", Value::Number(border_radius)) => {
                self.style.border_radius = *border_radius as u32;
            }
            ("CursorBlink", Value::Bool(cursor_blink)) => self.cursor_blink = *cursor_blink,
            ("WaitTimeout", Value::Duration(wait_timeout)) => self.wait_timeout = *wait_timeout,
            ("WaitTimeout", Value::Number(wait_timeout)) => {
                self.wait_timeout = Duration::from_secs_f64(*wait_timeout);
            }
            ("WaitPattern", Value::String(wait_pattern)) => {
                let wait_pattern = regex_source(wait_pattern);
                Regex::new(&wait_pattern)
                    .map_err(|error| miette!("invalid WaitPattern `{wait_pattern}`: {error}"))?;
                self.wait_pattern = WaitPattern::Regex(wait_pattern);
            }
            ("Theme", Value::String(theme)) => {
                let theme = TerminalTheme::from_name(theme)?;
                self.style.window_bar_color = theme.background_hex();
                self.theme = theme;
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
    /// The terminal render area is the canvas minus text padding on each side. Each dimension is
    /// clamped to at least one cell so pathological settings still create a valid PTY.
    fn apply_terminal_grid(&mut self) {
        let inner_width = self
            .width
            .saturating_sub(self.text.padding.saturating_mul(2));
        let inner_height = self
            .height
            .saturating_sub(self.text.padding.saturating_mul(2));
        self.columns = fit_cells(inner_width, self.text.cell_width());
        self.rows = fit_cells(inner_height, self.text.cell_height());
    }

    /// Delay stored on each output frame after playback-speed adjustment.
    ///
    /// This duration is used by GIF/video/PNG sequence frame lists. It is not used to sleep while
    /// executing commands; command execution timing is controlled by `capture_interval`,
    /// `typing_delay`, explicit sleeps, and wait timeouts.
    pub(super) fn frame_delay(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.output_framerate())
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
    pub(super) fn apply_loop_offset(&self, frames: &mut [(Frame, Duration)]) {
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

    /// Apply margin, window-bar, and rounded-corner decoration to all captured frames.
    ///
    /// The function drains and replaces the frame vector so callers cannot accidentally mix raw and
    /// decorated frames. Delays are preserved exactly.
    pub(super) fn decorate_frames(&self, frames: &mut Vec<(Frame, Duration)>) -> Result<()> {
        let mut decorated = Vec::with_capacity(frames.len());
        for (frame, delay) in frames.drain(..) {
            decorated.push((self.decorate_frame(&frame)?, delay));
        }
        *frames = decorated;
        Ok(())
    }

    /// Decorate a single raw terminal frame.
    ///
    /// Invalid color strings are treated as the theme background instead of failing the render.
    /// That keeps old tapes usable while output styling is still gaining validation.
    pub(super) fn decorate_frame(&self, frame: &Frame) -> Result<Frame> {
        let margin = self.style.margin;
        let bar_height = if self.style.window_bar.is_empty() {
            0
        } else {
            self.style.window_bar_size
        };
        let fill = parse_hex_color(&self.style.margin_fill).unwrap_or(self.theme.background);
        let bar = parse_hex_color(&self.style.window_bar_color).unwrap_or(self.theme.background);
        let output_width = frame.width.saturating_add(margin.saturating_mul(2));
        let output_height = frame
            .height
            .saturating_add(bar_height)
            .saturating_add(margin.saturating_mul(2));
        let mut output = SolidFrame::new(output_width, output_height, fill)?;
        output.blit(frame, margin, margin + bar_height)?;
        if bar_height > 0 {
            output.fill_rect(margin, margin, frame.width, bar_height, bar);
            output.draw_window_buttons(
                margin + bar_height / 2,
                margin + bar_height / 2,
                bar_height,
                &self.style.window_bar,
            );
        }
        if self.style.border_radius > 0 {
            output.apply_rounded_rect_mask(
                margin,
                margin,
                frame.width,
                frame.height + bar_height,
                self.style.border_radius,
                fill,
            );
        }
        Ok(output.into_frame())
    }
}

/// Simple RGBA canvas used only for post-render decoration.
///
/// The raw terminal frame may come from any supported [`PixelFormat`], but the decoration path
/// normalizes into tightly packed RGBA because margin fills, window buttons, and rounded-corner
/// masking are easier to reason about in a single byte order.
struct SolidFrame {
    /// Output width in pixels.
    width: u32,
    /// Output height in pixels.
    height: u32,
    /// RGBA8 pixel buffer with tightly packed rows.
    pixels: Vec<u8>,
}

impl SolidFrame {
    /// Allocate a solid-color RGBA frame, checking for integer overflow.
    fn new(width: u32, height: u32, color: libghostty_vt::style::RgbColor) -> Result<Self> {
        let len = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
            .ok_or_else(|| miette!("frame is too large"))?;
        let mut frame = Self {
            width,
            height,
            pixels: vec![0; len],
        };
        frame.fill_rect(0, 0, width, height, color);
        Ok(frame)
    }

    /// Copy a frame into this frame at a pixel offset.
    ///
    /// The source is converted to RGBA first, so callers can pass either RGBA or BGRA frames.
    fn blit(&mut self, frame: &Frame, x: u32, y: u32) -> Result<()> {
        let rgba = frame.rgba()?;
        for row in 0..frame.height {
            for col in 0..frame.width {
                let src = ((row as usize * frame.width as usize) + col as usize) * BYTES_PER_PIXEL;
                let dst = self.offset(x + col, y + row);
                self.pixels[dst..dst + BYTES_PER_PIXEL]
                    .copy_from_slice(&rgba[src..src + BYTES_PER_PIXEL]);
            }
        }
        Ok(())
    }

    /// Fill a clipped rectangle.
    fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: libghostty_vt::style::RgbColor,
    ) {
        let x1 = x.saturating_add(width).min(self.width);
        let y1 = y.saturating_add(height).min(self.height);
        for yy in y..y1 {
            for xx in x..x1 {
                let offset = self.offset(xx, yy);
                self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&[
                    color.r,
                    color.g,
                    color.b,
                    OPAQUE_ALPHA,
                ]);
            }
        }
    }

    /// Draw macOS-style window buttons.
    ///
    /// The current style language treats any non-empty mode as enabled. Modes ending in `Right`
    /// place the buttons on the right; all other values place them on the left.
    fn draw_window_buttons(&mut self, cx: u32, cy: u32, bar_height: u32, mode: &str) {
        let radius = (bar_height / WINDOW_BUTTON_RADIUS_DIVISOR).max(MIN_WINDOW_BUTTON_RADIUS);
        let gap = radius * WINDOW_BUTTON_GAP_RADIUS_MULTIPLIER;
        let right = mode.ends_with("Right");
        let start_x = if right {
            self.width.saturating_sub(cx + gap * 2)
        } else {
            cx
        };
        let colors = [
            rgb(0xff, 0x5f, 0x57),
            rgb(0xff, 0xbd, 0x2e),
            rgb(0x28, 0xc8, 0x40),
        ];
        for (index, color) in colors.into_iter().enumerate() {
            self.fill_circle(start_x + gap * index as u32, cy, radius, color);
        }
    }

    /// Fill a clipped circle using integer distance checks.
    fn fill_circle(
        &mut self,
        cx: u32,
        cy: u32,
        radius: u32,
        color: libghostty_vt::style::RgbColor,
    ) {
        let radius = radius as i32;
        for y in -radius..=radius {
            for x in -radius..=radius {
                if x * x + y * y > radius * radius {
                    continue;
                }
                let px = cx as i32 + x;
                let py = cy as i32 + y;
                if px < 0 || py < 0 || px >= self.width as i32 || py >= self.height as i32 {
                    continue;
                }
                let offset = self.offset(px as u32, py as u32);
                self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&[
                    color.r,
                    color.g,
                    color.b,
                    OPAQUE_ALPHA,
                ]);
            }
        }
    }

    /// Replace pixels outside a rounded rectangle with the surrounding fill color.
    ///
    /// This is a visual mask, not an alpha mask, because current output formats are opaque.
    fn apply_rounded_rect_mask(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
        fill: libghostty_vt::style::RgbColor,
    ) {
        let x1 = x.saturating_add(width).min(self.width);
        let y1 = y.saturating_add(height).min(self.height);
        let radius = radius.min(width / 2).min(height / 2) as i32;
        for yy in y..y1 {
            for xx in x..x1 {
                let left = xx < x + radius as u32;
                let right = xx >= x1.saturating_sub(radius as u32);
                let top = yy < y + radius as u32;
                let bottom = yy >= y1.saturating_sub(radius as u32);
                if !(left || right) || !(top || bottom) {
                    continue;
                }
                let cx = if left {
                    x as i32 + radius
                } else {
                    x1 as i32 - radius - 1
                };
                let cy = if top {
                    y as i32 + radius
                } else {
                    y1 as i32 - radius - 1
                };
                let dx = xx as i32 - cx;
                let dy = yy as i32 - cy;
                if dx * dx + dy * dy > radius * radius {
                    let offset = self.offset(xx, yy);
                    self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&[
                        fill.r,
                        fill.g,
                        fill.b,
                        OPAQUE_ALPHA,
                    ]);
                }
            }
        }
    }

    /// Return the byte offset for a pixel in the tightly packed RGBA buffer.
    fn offset(&self, x: u32, y: u32) -> usize {
        ((y as usize * self.width as usize) + x as usize) * BYTES_PER_PIXEL
    }

    /// Convert the solid frame into the shared media frame type.
    fn into_frame(self) -> Frame {
        Frame {
            width: self.width,
            height: self.height,
            stride: self.width as usize * BYTES_PER_PIXEL,
            format: PixelFormat::Rgba8,
            pixels: self.pixels,
        }
    }
}

/// Parse a `#RRGGBB` color.
///
/// Decoration colors are parsed leniently. Invalid values return `None` so callers can fall back
/// to the active theme background rather than failing old tapes that used unsupported color text.
fn parse_hex_color(value: &str) -> Option<libghostty_vt::style::RgbColor> {
    let value = value.trim().strip_prefix('#')?;
    if value.len() != 6 {
        return None;
    }
    Some(rgb(
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ))
}

/// Construct an RGB color.
const fn rgb(r: u8, g: u8, b: u8) -> libghostty_vt::style::RgbColor {
    libghostty_vt::style::RgbColor { r, g, b }
}

/// Fit as many terminal cells as possible in a pixel dimension.
///
/// The result is always at least one cell. Extremely large dimensions saturate to `u16::MAX`,
/// matching the PTY sizing type used by `portable-pty`.
fn fit_cells(pixels: u32, cell_pixels: u32) -> u16 {
    let cells = (pixels / cell_pixels.max(1)).max(1);
    u16::try_from(cells).unwrap_or(u16::MAX)
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
            | "WindowBarSize"
            | "BorderRadius"
            | "CursorBlink"
            | "WaitTimeout"
            | "WaitPattern"
            | "Theme"
    )
}

/// Return the expected tape value kind for a supported `Set` key.
fn setting_expected_type(key: &str) -> &'static str {
    match key {
        "Shell" | "FontFamily" | "MarginFill" | "WindowBar" | "WaitPattern" | "Theme" => "string",
        "Width" | "Height" | "FontSize" | "LetterSpacing" | "LineHeight" | "Padding"
        | "Framerate" | "PlaybackSpeed" | "LoopOffset" | "Margin" | "WindowBarSize"
        | "BorderRadius" => "number",
        "TypingSpeed" => "duration",
        "CursorBlink" => "bool",
        "WaitTimeout" => "duration or number",
        _ => "known value",
    }
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
