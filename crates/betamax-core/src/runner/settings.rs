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

mod frame_decoration;

use std::env;
use std::ffi::OsString;
use std::time::Duration;

use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap,
};
use miette::miette;
use regex::Regex;

use self::frame_decoration::{parse_hex_color, rgb, PixelRect, SolidFrame};
use crate::ghostty::{TerminalTheme, TextSettings};
use crate::media::Frame;
use crate::tape::{Command, Key, KeyCode, Tape, Value, WaitPattern};
use crate::wait::regex_source;
use crate::Result;

/// Default final output width in pixels.
const DEFAULT_WIDTH: u32 = 1200;
/// Default final output height in pixels.
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
/// Minimum effective frame rate used in timing calculations.
const MIN_EFFECTIVE_FRAMERATE: u16 = 1;
/// Cursor blink uses a two-phase on/off cycle.
const CURSOR_BLINK_PHASES: usize = 2;
/// Caption font size relative to the tape font size.
const CAPTION_FONT_SCALE: f32 = 0.9;
/// Minimum readable caption font size.
const MIN_CAPTION_FONT_SIZE: f32 = 14.0;
/// Caption line height relative to caption font size.
const CAPTION_LINE_HEIGHT: f32 = 1.2;
/// Vertical caption padding relative to caption font size.
const CAPTION_VERTICAL_PADDING: f32 = 0.25;
/// Maximum number of recent overlay events drawn over the output frame.
const KEYBOARD_OVERLAY_MAX_CHIPS: usize = 5;
/// Maximum number of overlay rows drawn over the output frame.
const KEYBOARD_OVERLAY_MAX_ROWS: usize = 1;
/// Font size used by the compact keyboard overlay.
const KEYBOARD_OVERLAY_FONT_SIZE: f32 = 18.0;
/// Line height used by the compact keyboard overlay.
const KEYBOARD_OVERLAY_LINE_HEIGHT: f32 = 25.0;
/// Approximate label glyph width for row wrapping.
const KEYBOARD_OVERLAY_CHAR_WIDTH: u32 = 11;
/// Horizontal inset around the overlay HUD.
const KEYBOARD_OVERLAY_INSET_X: u32 = 14;
/// Vertical inset reserved above keyboard chips in the presentation row.
const KEYBOARD_OVERLAY_INSET_Y: u32 = 12;
/// Horizontal padding inside one key chip.
const KEYBOARD_OVERLAY_CHIP_PAD_X: u32 = 8;
/// Vertical text adjustment inside one key chip.
const KEYBOARD_OVERLAY_TEXT_OFFSET_Y: u32 = 1;
/// Gap between key chips.
const KEYBOARD_OVERLAY_CHIP_GAP: u32 = 8;
/// Inset used when keyboard chips are anchored inside the terminal canvas.
const KEYBOARD_OVERLAY_CORNER_INSET: u32 = 18;
/// Maximum number of displayed characters from one `Type` command.
const KEYBOARD_OVERLAY_TYPE_MAX_CHARS: usize = 26;
/// Alpha used for individual key chips.
const KEYBOARD_OVERLAY_CHIP_ALPHA: u8 = 235;
/// Radius used for rounded keyboard overlay keycap corners.
const KEYBOARD_OVERLAY_CHIP_CORNER_RADIUS: u32 = 3;
/// Minimum horizontal presentation-row inset without a margin, relative to caption font size.
const PRESENTATION_OVERLAY_MIN_INSET_SCALE: f32 = 0.5;
/// Gap between caption text and right-aligned keyboard chips.
const PRESENTATION_OVERLAY_GAP: u32 = 16;

/// Shell command and all derived execution, rendering, timing, and styling settings.
///
/// Values are produced by [`Settings::from_tape`]. Callers should not construct this type manually
/// because `columns` and `rows` are derived from several pixel/text settings after all tape
/// commands have been applied.
#[derive(Debug, Clone)]
pub(super) struct Settings {
    /// Shell command and optional explicit arguments from `Set Shell`.
    pub(super) shell: Vec<OsString>,
    /// Final output width after margin/window decoration.
    pub(super) width: u32,
    /// Final output height after margin/window decoration.
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
    /// Whether the tape uses presentation captions.
    pub(super) caption_overlay: bool,
    /// Optional input-sequence overlay for review media.
    pub(super) keyboard_overlay: KeyboardOverlaySettings,
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

/// Presentation-only input labels drawn over generated media.
///
/// The labels are derived from tape commands and are never fed back into PTY execution. This keeps
/// review media decoration separate from validation semantics and terminal sizing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct KeyboardOverlaySettings {
    /// Which input commands are eligible for the overlay.
    pub(super) mode: KeyboardOverlayMode,
    /// Where the overlay is drawn in the final output frame.
    pub(super) location: KeyboardOverlayLocation,
}

impl KeyboardOverlaySettings {
    /// Return whether a frame should draw the overlay.
    fn visible(&self, labels: &[String]) -> bool {
        self.mode != KeyboardOverlayMode::Off && !labels.is_empty()
    }
}

/// Input event filtering used by the keyboard overlay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum KeyboardOverlayMode {
    /// Do not draw any keyboard overlay.
    #[default]
    Off,
    /// Draw only explicit key commands such as `Ctrl+P`, arrows, `Enter`, and `Escape`.
    Keys,
    /// Draw key commands and short typed input that reads like user intent.
    Input,
    /// Draw every visible input event, including long `Type` commands summarized for space.
    All,
}

/// Placement for keyboard overlay chips.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum KeyboardOverlayLocation {
    /// Draw the overlay in the presentation row below the terminal.
    #[default]
    CaptionRow,
    /// Draw the overlay inside the terminal's upper-left corner.
    TopLeft,
    /// Draw the overlay inside the terminal's upper-right corner.
    TopRight,
    /// Draw the overlay inside the terminal's lower-left corner.
    BottomLeft,
    /// Draw the overlay inside the terminal's lower-right corner.
    BottomRight,
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
            caption_overlay: false,
            keyboard_overlay: KeyboardOverlaySettings::default(),
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
                Command::Caption(text) if !text.trim().is_empty() => {
                    settings.caption_overlay = true;
                }
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
            ("KeyboardOverlay", Value::String(mode)) => {
                self.keyboard_overlay.mode = parse_keyboard_overlay_mode(mode)?;
            }
            ("KeyboardOverlayLocation", Value::String(location)) => {
                self.keyboard_overlay.location = parse_keyboard_overlay_location(location)?;
            }
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

    /// Width of the terminal canvas before margin and window-bar decoration.
    pub(super) fn terminal_canvas_width(&self) -> u32 {
        self.width
            .saturating_sub(self.effective_margin().saturating_mul(2))
    }

    /// Height of the terminal canvas before margin and window-bar decoration.
    pub(super) fn terminal_canvas_height(&self) -> u32 {
        self.height
            .saturating_sub(self.effective_margin().saturating_mul(2))
            .saturating_sub(self.window_bar_height())
            .saturating_sub(self.presentation_overlay_height())
    }

    /// VHS only applies margin when a margin fill is configured.
    fn effective_margin(&self) -> u32 {
        if self.style.margin_fill.is_empty() {
            0
        } else {
            self.style.margin
        }
    }

    /// Height reserved above the terminal canvas for the synthetic window bar.
    fn window_bar_height(&self) -> u32 {
        if self.style.window_bar.is_empty() {
            0
        } else {
            self.style.window_bar_size
        }
    }

    /// Height reserved below the terminal canvas for presentation-only overlays.
    fn presentation_overlay_height(&self) -> u32 {
        let keyboard_overlay_height =
            if self.keyboard_overlay.location == KeyboardOverlayLocation::CaptionRow {
                self.keyboard_overlay_height()
            } else {
                0
            };
        self.caption_overlay_height().max(keyboard_overlay_height)
    }

    /// Height reserved for captions when any non-empty caption appears in the tape.
    fn caption_overlay_height(&self) -> u32 {
        if !self.caption_overlay {
            return 0;
        }
        let font_size = self.caption_font_size();
        let line_height = (font_size * CAPTION_LINE_HEIGHT).ceil() as u32;
        let vertical_padding = (font_size * CAPTION_VERTICAL_PADDING).ceil() as u32;
        line_height.saturating_add(vertical_padding.saturating_mul(2))
    }

    /// Height reserved for the largest keyboard overlay panel the configured mode can draw.
    fn keyboard_overlay_height(&self) -> u32 {
        if self.keyboard_overlay.mode == KeyboardOverlayMode::Off {
            return 0;
        }
        let row_height = KEYBOARD_OVERLAY_LINE_HEIGHT.ceil() as u32;
        KEYBOARD_OVERLAY_INSET_Y
            .saturating_add(row_height.saturating_mul(KEYBOARD_OVERLAY_MAX_ROWS as u32))
    }

    /// Font size used by caption rendering and layout reservation.
    fn caption_font_size(&self) -> f32 {
        (self.text.font_size * CAPTION_FONT_SCALE).max(MIN_CAPTION_FONT_SIZE)
    }

    /// Left edge for presentation content, aligned with the terminal frame.
    fn presentation_overlay_left_x(&self) -> u32 {
        self.effective_margin().min(self.width)
    }

    /// Right edge for presentation content, aligned with the terminal frame.
    fn presentation_overlay_right_x(&self) -> u32 {
        self.width.saturating_sub(self.effective_margin())
    }

    /// Derive terminal rows and columns from pixel dimensions.
    ///
    /// `Width` and `Height` describe the final output frame, matching VHS. Margin and window bar
    /// decoration are subtracted first, then text padding is subtracted from the terminal canvas.
    /// Each dimension is clamped to at least one cell so pathological settings still create a valid
    /// PTY.
    fn apply_terminal_grid(&mut self) {
        let inner_width = self
            .terminal_canvas_width()
            .saturating_sub(self.text.padding.saturating_mul(2));
        let inner_height = self
            .terminal_canvas_height()
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

    /// Decorate a single raw terminal frame with caption text and keyboard labels.
    pub(super) fn decorate_frame_with_overlays(
        &self,
        frame: &Frame,
        caption: Option<&str>,
        keyboard_overlay_labels: &[String],
    ) -> Result<Frame> {
        let mut caption_renderer = CaptionRenderer::new();
        self.decorate_frame_with_overlay_renderer(
            frame,
            caption,
            keyboard_overlay_labels,
            &mut caption_renderer,
        )
    }

    /// Decorate captured frames with one reusable caption renderer.
    ///
    /// Animated outputs may render hundreds of frames with the same caption text and font. Keeping
    /// one renderer here preserves the shaping and glyph caches for the batch without making cache
    /// lifetime part of `Settings`.
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
    ) -> Result<Frame> {
        let margin = self.effective_margin();
        let bar_height = self.window_bar_height();
        let fill = parse_hex_color(&self.style.margin_fill).unwrap_or(self.theme.background);
        let bar = parse_hex_color(&self.style.window_bar_color).unwrap_or(self.theme.background);
        let mut output = SolidFrame::new(self.width, self.height, fill)?;
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
            output.draw_keyboard_overlay(
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
        output: &mut SolidFrame,
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

        let font_size = self.caption_font_size();
        let line_height = (font_size * CAPTION_LINE_HEIGHT).ceil();
        let vertical_padding = (font_size * CAPTION_VERTICAL_PADDING).ceil() as u32;
        let height = self.presentation_overlay_height().min(self.height);
        let caption_height = self.caption_overlay_height().min(height);

        let y = self
            .height
            .saturating_sub(self.effective_margin())
            .saturating_sub(height);
        let centered_text_y = y
            .saturating_add(height.saturating_sub(caption_height) / 2)
            .saturating_add(vertical_padding);
        let text_y = if avoid_right_width > 0 {
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
        let margin = self.effective_margin();
        let terminal_left_x = margin.min(self.width);
        let terminal_right_x = self.width.saturating_sub(margin);
        let terminal_top_y = margin
            .saturating_add(self.window_bar_height())
            .min(self.height);
        let terminal_bottom_y = terminal_top_y
            .saturating_add(self.terminal_canvas_height())
            .min(self.height.saturating_sub(margin));

        match self.keyboard_overlay.location {
            KeyboardOverlayLocation::CaptionRow => KeyboardOverlayAnchor {
                horizontal: HorizontalAnchor::Right,
                vertical: VerticalAnchor::Bottom,
                left_x: self.presentation_overlay_left_x(),
                top_y: self
                    .height
                    .saturating_sub(margin)
                    .saturating_sub(self.presentation_overlay_height().min(self.height)),
                right_x: self
                    .presentation_overlay_right_x()
                    .saturating_sub(self.presentation_overlay_inset_x()),
                bottom_y: self.height.saturating_sub(margin),
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
        if self.effective_margin() > 0 {
            return optical_inset;
        }
        let min_inset =
            (self.caption_font_size() * PRESENTATION_OVERLAY_MIN_INSET_SCALE).ceil() as u32;
        self.text.padding.max(optical_inset).max(min_inset)
    }

    /// Return the keyboard overlay label for a visible input command, if enabled.
    pub(super) fn keyboard_overlay_label(&self, command: &Command) -> Option<String> {
        keyboard_overlay_label(command, self.keyboard_overlay.mode)
    }
}

#[derive(Debug, Clone, Copy)]
struct CaptionLayout {
    text_x: u32,
    text_y: u32,
    text_width: u32,
    text_height: u32,
    font_size: f32,
    line_height: f32,
}

#[derive(Debug, Clone, Copy)]
struct KeyboardOverlayAnchor {
    horizontal: HorizontalAnchor,
    vertical: VerticalAnchor,
    left_x: u32,
    top_y: u32,
    right_x: u32,
    bottom_y: u32,
    inset_x: u32,
    inset_y: u32,
}

impl KeyboardOverlayAnchor {
    fn width(self) -> u32 {
        self.right_x.saturating_sub(self.left_x)
    }

    fn inner_left_x(self) -> u32 {
        self.left_x.saturating_add(self.inset_x).min(self.right_x)
    }

    fn inner_right_x(self) -> u32 {
        self.right_x.saturating_sub(self.inset_x).max(self.left_x)
    }

    fn inner_top_y(self) -> u32 {
        self.top_y.saturating_add(self.inset_y).min(self.bottom_y)
    }

    fn inner_bottom_y(self) -> u32 {
        self.bottom_y.saturating_sub(self.inset_y).max(self.top_y)
    }
}

#[derive(Debug, Clone, Copy)]
enum HorizontalAnchor {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
enum VerticalAnchor {
    Top,
    Bottom,
}

impl CaptionLayout {
    /// Return the caption text bounds as a clipping rectangle.
    fn text_rect(self) -> PixelRect {
        PixelRect {
            x: self.text_x,
            y: self.text_y,
            width: self.text_width,
            height: self.text_height,
        }
    }
}

/// Text renderer for caption overlays.
///
/// This owns `cosmic_text`'s mutable shaping and raster caches. The renderer is kept outside
/// `Settings` so decoration remains a pure operation from caller input to media frame, while
/// callers that render many frames can still reuse the expensive font state.
struct CaptionRenderer {
    /// Font database and shaping context.
    font_system: FontSystem,
    /// Glyph raster cache.
    swash_cache: SwashCache,
}

impl CaptionRenderer {
    /// Create a caption renderer with reusable font and glyph caches.
    fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    /// Draw caption text into the given layout.
    ///
    /// Caption text is pre-truncated and drawn without wrapping. That keeps the presentation row
    /// stable and avoids hiding terminal content or colliding with keyboard labels.
    fn draw(
        &mut self,
        target: &mut SolidFrame,
        caption: &str,
        layout: CaptionLayout,
        text_settings: &TextSettings,
        color: libghostty_vt::style::RgbColor,
    ) {
        let family = text_settings
            .font_family
            .as_deref()
            .map(Family::Name)
            .unwrap_or(Family::Monospace);
        let attrs = Attrs::new().family(family).weight(Weight::BOLD);
        let metrics = Metrics::new(layout.font_size, layout.line_height);
        let caption = self.truncate_caption(caption, layout, metrics, &attrs);

        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        buffer.set_size(
            Some(layout.text_width as f32),
            Some(layout.text_height as f32),
        );
        buffer.set_wrap(Wrap::None);
        buffer.set_text(&caption, &attrs, Shaping::Advanced, None);
        buffer.draw(
            &mut self.swash_cache,
            Color::rgb(color.r, color.g, color.b),
            |glyph_x, glyph_y, width, height, glyph_color| {
                target.blend_rect_clipped(
                    layout.text_x as i32 + glyph_x,
                    layout.text_y as i32 + glyph_y,
                    width,
                    height,
                    glyph_color,
                    layout.text_rect(),
                );
            },
        );
    }

    /// Truncate caption text with the same shaping stack used for drawing.
    fn truncate_caption(
        &mut self,
        caption: &str,
        layout: CaptionLayout,
        metrics: Metrics,
        attrs: &Attrs<'_>,
    ) -> String {
        const ELLIPSIS: &str = "...";

        if self.caption_text_width(caption, layout, metrics, attrs) <= layout.text_width as f32 {
            return caption.to_string();
        }
        if self.caption_text_width(ELLIPSIS, layout, metrics, attrs) > layout.text_width as f32 {
            return ELLIPSIS.to_string();
        }

        let char_ends = caption
            .char_indices()
            .map(|(index, ch)| index + ch.len_utf8())
            .collect::<Vec<_>>();
        let mut low = 0;
        let mut high = char_ends.len();
        while low < high {
            let mid = (low + high).div_ceil(2);
            let candidate = format!("{}{}", &caption[..char_ends[mid - 1]], ELLIPSIS);
            if self.caption_text_width(&candidate, layout, metrics, attrs)
                <= layout.text_width as f32
            {
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        if low == 0 {
            return ELLIPSIS.to_string();
        }
        format!("{}{}", &caption[..char_ends[low - 1]], ELLIPSIS)
    }

    /// Measure a single caption line after font fallback and shaping.
    fn caption_text_width(
        &mut self,
        caption: &str,
        layout: CaptionLayout,
        metrics: Metrics,
        attrs: &Attrs<'_>,
    ) -> f32 {
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        buffer.set_size(None, Some(layout.text_height as f32));
        buffer.set_wrap(Wrap::None);
        buffer.set_text(caption, attrs, Shaping::Advanced, None);
        buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0, f32::max)
    }
}

impl SolidFrame {
    /// Draw the configured input sequence as compact key chips over the bottom of the frame.
    fn draw_keyboard_overlay(
        &mut self,
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
        let panel_left_x = anchor.inner_left_x().min(self.width);
        let panel_right_x = anchor.inner_right_x().min(self.width);
        let panel_width = row_width.min(panel_right_x.saturating_sub(panel_left_x));
        if panel_width == 0 {
            return;
        }
        let row_block_height = row_height.saturating_mul(rows.len() as u32);
        let panel_x = match anchor.horizontal {
            HorizontalAnchor::Left => panel_left_x,
            HorizontalAnchor::Right => panel_right_x.saturating_sub(panel_width),
        };
        let panel_top_y = anchor.inner_top_y().min(self.height);
        let panel_bottom_y = anchor.inner_bottom_y().min(self.height);
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
                self.fill_rounded_rect_alpha(
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
                    self,
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
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    /// Draw one overlay label, clipped to the provided width.
    fn draw(&mut self, target: &mut SolidFrame, text: &str, x: u32, y: u32, width: u32) {
        if width == 0 {
            return;
        }

        let metrics = Metrics::new(KEYBOARD_OVERLAY_FONT_SIZE, KEYBOARD_OVERLAY_LINE_HEIGHT);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        let family = self
            .font_family
            .as_deref()
            .map(Family::Name)
            .unwrap_or(Family::Monospace);
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

/// Fit as many terminal cells as possible in a pixel dimension.
///
/// The result is always at least one cell. Extremely large dimensions saturate to `u16::MAX`,
/// matching the PTY sizing type used by `portable-pty`.
fn fit_cells(pixels: u32, cell_pixels: u32) -> u16 {
    let cells = (pixels / cell_pixels.max(1)).max(1);
    u16::try_from(cells).unwrap_or(u16::MAX)
}

/// Parse the presentation mode accepted by `Set KeyboardOverlay`.
fn parse_keyboard_overlay_mode(mode: &str) -> Result<KeyboardOverlayMode> {
    match mode.to_ascii_lowercase().as_str() {
        "off" => Ok(KeyboardOverlayMode::Off),
        "keys" => Ok(KeyboardOverlayMode::Keys),
        "input" => Ok(KeyboardOverlayMode::Input),
        "all" => Ok(KeyboardOverlayMode::All),
        _ => Err(
            miette!("Set KeyboardOverlay expects Off, Keys, Input, or All, got `{mode}`").into(),
        ),
    }
}

/// Parse the placement accepted by `Set KeyboardOverlayLocation`.
fn parse_keyboard_overlay_location(location: &str) -> Result<KeyboardOverlayLocation> {
    match location.to_ascii_lowercase().as_str() {
        "captionrow" | "caption-row" | "caption_row" => {
            Ok(KeyboardOverlayLocation::CaptionRow)
        }
        "topleft" | "top-left" | "top_left" => Ok(KeyboardOverlayLocation::TopLeft),
        "topright" | "top-right" | "top_right" => Ok(KeyboardOverlayLocation::TopRight),
        "bottomleft" | "bottom-left" | "bottom_left" => Ok(KeyboardOverlayLocation::BottomLeft),
        "bottomright" | "bottom-right" | "bottom_right" => {
            Ok(KeyboardOverlayLocation::BottomRight)
        }
        _ => Err(miette!(
            "Set KeyboardOverlayLocation expects CaptionRow, TopLeft, TopRight, BottomLeft, or BottomRight, got `{location}`"
        )
        .into()),
    }
}

/// Return the overlay label for one input-producing command.
fn keyboard_overlay_label(command: &Command, mode: KeyboardOverlayMode) -> Option<String> {
    match command {
        Command::Type { text, .. } if mode == KeyboardOverlayMode::All && !text.is_empty() => {
            Some(describe_overlay_type(text))
        }
        Command::Type { text, .. }
            if mode == KeyboardOverlayMode::Input && is_short_input_text(text) =>
        {
            Some(describe_overlay_input_text(text))
        }
        Command::Key { key, count, .. } if mode != KeyboardOverlayMode::Off => {
            let key = describe_overlay_key(key);
            if *count == 1 {
                Some(key)
            } else {
                Some(format!("{key} x{count}"))
            }
        }
        Command::Paste if matches!(mode, KeyboardOverlayMode::Input | KeyboardOverlayMode::All) => {
            Some("Paste".to_string())
        }
        _ => None,
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
fn keyboard_overlay_panel_width(labels: &[String], width: u32) -> u32 {
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

/// Describe typed text compactly for the overlay.
fn describe_overlay_text(text: &str) -> String {
    let mut output = String::new();
    let mut chars = text.chars();
    for ch in chars.by_ref().take(KEYBOARD_OVERLAY_TYPE_MAX_CHARS) {
        match ch {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            ch => output.push(ch),
        }
    }
    if chars.next().is_some() {
        output.push_str("...");
    }
    output
}

/// Describe one `Type` command for the overlay.
fn describe_overlay_type(text: &str) -> String {
    if is_short_input_text(text) {
        return describe_overlay_input_text(text);
    }
    format!("Type {} chars", text.chars().count())
}

/// Describe short typed input as a key label.
fn describe_overlay_input_text(text: &str) -> String {
    let text = describe_overlay_text(text);
    if text.chars().count() == 1 {
        text
    } else {
        format!("\"{text}\"")
    }
}

/// Return whether typed text is short enough to show as user intent in `Input` mode.
fn is_short_input_text(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= KEYBOARD_OVERLAY_TYPE_MAX_CHARS
        && !text.chars().any(|ch| matches!(ch, '\n' | '\r' | ';'))
}

/// Describe a key press compactly for the overlay.
fn describe_overlay_key(key: &Key) -> String {
    let Key::Press { key, modifiers } = key;
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if modifiers.alt {
        parts.push("Alt".to_string());
    }
    if modifiers.shift {
        parts.push("Shift".to_string());
    }
    parts.push(describe_overlay_key_code(*key));
    parts.join("+")
}

/// Describe the logical key code used by a key press.
fn describe_overlay_key_code(key: KeyCode) -> String {
    match key {
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Escape => "Escape".to_string(),
        KeyCode::Function(number) => format!("F{number}"),
        KeyCode::Home => "Home".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Space => "Space".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Char(ch) => ch.to_string(),
    }
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
        "Shell" | "FontFamily" | "MarginFill" | "WindowBar" | "WaitPattern" | "Theme" => "string",
        "Width" | "Height" | "FontSize" | "LetterSpacing" | "LineHeight" | "Padding"
        | "Framerate" | "PlaybackSpeed" | "LoopOffset" | "Margin" | "WindowBarSize"
        | "BorderRadius" => "number",
        "TypingSpeed" => "duration",
        "CursorBlink" => "bool",
        "KeyboardOverlay" => "Off, Keys, Input, or All",
        "KeyboardOverlayLocation" => "CaptionRow, TopLeft, TopRight, BottomLeft, or BottomRight",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_truncation_uses_shaped_width_and_three_dots() {
        let mut renderer = CaptionRenderer::new();
        let layout = CaptionLayout {
            text_x: 0,
            text_y: 0,
            text_width: 150,
            text_height: 28,
            font_size: 20.0,
            line_height: 24.0,
        };
        let metrics = Metrics::new(layout.font_size, layout.line_height);
        let attrs = Attrs::new().family(Family::Monospace).weight(Weight::BOLD);
        let caption = "The bottom terminal row remains visible beside the key chips";

        let truncated = renderer.truncate_caption(caption, layout, metrics, &attrs);

        assert!(truncated.ends_with("..."));
        assert!(truncated.len() < caption.len());
        assert!(
            renderer.caption_text_width(&truncated, layout, metrics, &attrs)
                <= layout.text_width as f32
        );
    }

    #[test]
    fn caption_truncation_uses_full_ellipsis_when_space_is_tiny() {
        let mut renderer = CaptionRenderer::new();
        let layout = CaptionLayout {
            text_x: 0,
            text_y: 0,
            text_width: 1,
            text_height: 28,
            font_size: 20.0,
            line_height: 24.0,
        };
        let metrics = Metrics::new(layout.font_size, layout.line_height);
        let attrs = Attrs::new().family(Family::Monospace).weight(Weight::BOLD);

        assert_eq!(
            renderer.truncate_caption("overflow", layout, metrics, &attrs),
            "..."
        );
    }

    #[test]
    fn caption_layout_shares_terminal_frame_edges_with_keyboard_overlay() {
        let tape = Tape::parse(
            r#"
            Set Width 320
            Set Height 200
            Set Margin 16
            Set KeyboardOverlay Input
            Caption "Long caption"
            Type "echo done"
            "#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let labels = vec!["\"echo done\"".to_string()];
        let avoid_width = keyboard_overlay_panel_width(&labels, settings.width)
            .saturating_add(PRESENTATION_OVERLAY_GAP);
        let layout = settings.caption_layout(avoid_width).unwrap();

        assert_eq!(layout.text_x, 16);
        assert_eq!(
            layout.text_width,
            settings
                .width
                .saturating_sub(32)
                .saturating_sub(avoid_width)
        );
        assert_eq!(settings.presentation_overlay_right_x(), 304);
        assert_eq!(settings.keyboard_overlay_anchor().right_x, 304);
        assert_eq!(settings.caption_overlay_left_x(), 16);
    }

    #[test]
    fn presentation_overlay_edges_track_rounded_corner_optics() {
        let tape = Tape::parse(
            r#"
            Set Width 320
            Set Height 200
            Set Margin 16
            Set BorderRadius 8
            Set KeyboardOverlay Input
            Caption "Long caption"
            Type "echo done"
            "#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let labels = vec!["\"echo done\"".to_string()];
        let avoid_width = keyboard_overlay_panel_width(&labels, settings.width)
            .saturating_add(PRESENTATION_OVERLAY_GAP);
        let layout = settings.caption_layout(avoid_width).unwrap();

        assert_eq!(settings.presentation_overlay_right_x(), 304);
        assert_eq!(settings.caption_overlay_left_x(), 20);
        assert_eq!(settings.keyboard_overlay_anchor().right_x, 300);
        assert_eq!(layout.text_x, 20);
    }

    #[test]
    fn presentation_overlay_without_margin_uses_terminal_padding() {
        let tape = Tape::parse(
            r#"
            Set Width 320
            Set Height 200
            Set Padding 20
            Set KeyboardOverlay Input
            Caption "Long caption"
            Type "echo done"
            "#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.caption_overlay_left_x(), 20);
        assert_eq!(settings.caption_layout(0).unwrap().text_x, 20);
        assert_eq!(settings.keyboard_overlay_anchor().right_x, 300);
    }

    #[test]
    fn presentation_overlay_without_margin_or_padding_keeps_minimum_inset() {
        let tape = Tape::parse(
            r#"
            Set Width 320
            Set Height 200
            Set FontSize 20
            Set Padding 0
            Caption "Long caption"
            "#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        // Half of the 18 px caption font (0.9 x FontSize 20).
        assert_eq!(settings.caption_overlay_left_x(), 9);
    }

    #[test]
    fn presentation_overlay_with_margin_ignores_terminal_padding() {
        let tape = Tape::parse(
            r#"
            Set Width 320
            Set Height 200
            Set Margin 16
            Set Padding 32
            Set BorderRadius 12
            Caption "Long caption"
            "#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.caption_overlay_left_x(), 22);
    }

    #[test]
    fn corner_keyboard_overlay_does_not_reserve_presentation_row() {
        let tape = Tape::parse(
            r#"
            Set Width 320
            Set Height 200
            Set Margin 16
            Set KeyboardOverlay Input
            Set KeyboardOverlayLocation BottomRight
            Type "echo done"
            "#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.presentation_overlay_height(), 0);
        assert_eq!(
            settings.terminal_canvas_height(),
            settings.height.saturating_sub(32)
        );
    }

    #[test]
    fn corner_keyboard_overlay_nestles_inside_terminal_edges() {
        let tape = Tape::parse(
            r#"
            Set Width 320
            Set Height 200
            Set Margin 16
            Set WindowBar Colorful
            Set WindowBarSize 30
            Set KeyboardOverlay Input
            Set KeyboardOverlayLocation BottomLeft
            Type "echo done"
            "#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let anchor = settings.keyboard_overlay_anchor();

        assert!(matches!(anchor.horizontal, HorizontalAnchor::Left));
        assert!(matches!(anchor.vertical, VerticalAnchor::Bottom));
        assert_eq!(anchor.inner_left_x(), 34);
        assert_eq!(anchor.inner_bottom_y(), 166);
    }
}
