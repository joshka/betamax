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

use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap,
};
use miette::miette;
use regex::Regex;

use crate::ghostty::{TerminalTheme, TextSettings};
use crate::media::{Frame, PixelFormat};
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
/// Caption font size relative to the tape font size.
const CAPTION_FONT_SCALE: f32 = 0.9;
/// Minimum readable caption font size.
const MIN_CAPTION_FONT_SIZE: f32 = 14.0;
/// Caption line height relative to caption font size.
const CAPTION_LINE_HEIGHT: f32 = 1.2;
/// Vertical caption padding relative to caption font size.
const CAPTION_VERTICAL_PADDING: f32 = 0.25;
/// Denominator for 8-bit alpha blending.
const ALPHA_DENOMINATOR: u16 = 255;
/// Sub-pixel samples per axis for anti-aliased rounded-rectangle edges.
const ROUNDED_RECT_AA_SAMPLES: u32 = 4;
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
/// Maximum number of displayed characters from one `Type` command.
const KEYBOARD_OVERLAY_TYPE_MAX_CHARS: usize = 26;
/// Alpha used for individual key chips.
const KEYBOARD_OVERLAY_CHIP_ALPHA: u8 = 235;
/// Radius used for rounded keyboard overlay keycap corners.
const KEYBOARD_OVERLAY_CHIP_CORNER_RADIUS: u32 = 3;
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
        self.caption_overlay_height()
            .max(self.keyboard_overlay_height())
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
        let keyboard_avoid_width = if self.keyboard_overlay.visible(keyboard_overlay_labels) {
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
                self.keyboard_overlay_bottom_y(),
                self.keyboard_overlay_right_x(),
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
            let keyboard_row_height = KEYBOARD_OVERLAY_LINE_HEIGHT.ceil() as u32;
            let keyboard_text_y = self
                .keyboard_overlay_bottom_y()
                .saturating_sub(keyboard_row_height)
                .saturating_add(KEYBOARD_OVERLAY_TEXT_OFFSET_Y);
            keyboard_text_y.min(
                self.keyboard_overlay_bottom_y()
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

    /// Bottom edge for keyboard overlay chips in final output-frame coordinates.
    fn keyboard_overlay_bottom_y(&self) -> u32 {
        self.height.saturating_sub(self.effective_margin())
    }

    /// Right edge for keyboard chips after optical compensation for rounded terminal corners.
    fn keyboard_overlay_right_x(&self) -> u32 {
        self.presentation_overlay_right_x()
            .saturating_sub(self.presentation_overlay_optical_inset_x())
    }

    /// Left edge for captions after optical compensation for rounded terminal corners.
    fn caption_overlay_left_x(&self) -> u32 {
        self.presentation_overlay_left_x()
            .saturating_add(self.presentation_overlay_optical_inset_x())
            .min(self.presentation_overlay_right_x())
    }

    /// Horizontal optical inset applied to presentation overlays near rounded terminal corners.
    fn presentation_overlay_optical_inset_x(&self) -> u32 {
        self.style.border_radius / 2
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelRect {
    /// Left edge in output pixels.
    x: u32,
    /// Top edge in output pixels.
    y: u32,
    /// Width in output pixels.
    width: u32,
    /// Height in output pixels.
    height: u32,
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
        let Some((x0, y0, x1, y1)) = self.clipped_rect(x as i32, y as i32, width, height) else {
            return;
        };
        for yy in y0..y1 {
            for xx in x0..x1 {
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

    /// Fill a clipped rectangle with alpha blending.
    ///
    /// Alpha fills are composited onto an opaque frame instead of introducing transparent output
    /// pixels, because the media encoders currently treat rendered frames as opaque RGBA.
    fn fill_rect_alpha(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: libghostty_vt::style::RgbColor,
        alpha: u8,
    ) {
        let Some((x0, y0, x1, y1)) = self.clipped_rect(x as i32, y as i32, width, height) else {
            return;
        };
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.blend_pixel(xx, yy, color.r, color.g, color.b, alpha);
            }
        }
    }

    /// Fill a clipped rounded rectangle with alpha blending.
    ///
    /// Rounded corners are computed by skipping pixels outside corner circles; interior pixels
    /// receive normal alpha blending.
    fn fill_rounded_rect_alpha(
        &mut self,
        rect: PixelRect,
        color: libghostty_vt::style::RgbColor,
        alpha: u8,
        radius: u32,
    ) {
        let Some((x0, y0, x1, y1)) =
            self.clipped_rect(rect.x as i32, rect.y as i32, rect.width, rect.height)
        else {
            return;
        };
        let radius = radius.min(rect.width / 2).min(rect.height / 2) as i32;
        if radius <= 0 {
            self.fill_rect_alpha(rect.x, rect.y, rect.width, rect.height, color, alpha);
            return;
        }
        for yy in y0..y1 {
            let top_corner = yy < rect.y.saturating_add(radius as u32);
            let bottom_corner = yy >= y1.saturating_sub(radius as u32);
            for xx in x0..x1 {
                let left_corner = xx < rect.x.saturating_add(radius as u32);
                let right_corner = xx >= x1.saturating_sub(radius as u32);
                if (left_corner || right_corner) && (top_corner || bottom_corner) {
                    let cx = if left_corner {
                        rect.x as i32 + radius
                    } else {
                        x1 as i32 - radius - 1
                    };
                    let cy = if top_corner {
                        rect.y as i32 + radius
                    } else {
                        y1 as i32 - radius - 1
                    };
                    let coverage = rounded_corner_coverage(
                        xx,
                        yy,
                        cx,
                        cy,
                        radius as f32,
                        ROUNDED_RECT_AA_SAMPLES,
                    );
                    if coverage <= 0.0 {
                        continue;
                    }
                    let corner_alpha =
                        (f32::from(alpha) * coverage).round().clamp(0.0, 255.0) as u8;
                    self.blend_pixel(xx, yy, color.r, color.g, color.b, corner_alpha);
                    continue;
                }
                self.blend_pixel(xx, yy, color.r, color.g, color.b, alpha);
            }
        }
    }

    /// Alpha-blend a glyph rectangle into the frame.
    ///
    /// `cosmic_text` reports glyph rectangles in signed coordinates relative to the text origin.
    /// Clipping here lets shaped glyphs extend beyond the overlay bounds without panicking.
    fn blend_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) {
        let [r, g, b, a] = color.as_rgba();
        if a == 0 {
            return;
        }
        let Some((x0, y0, x1, y1)) = self.clipped_rect(x, y, width, height) else {
            return;
        };
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.blend_pixel(xx, yy, r, g, b, a);
            }
        }
    }

    /// Alpha-blend a glyph rectangle clipped to a caller-provided rectangle.
    fn blend_rect_clipped(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: Color,
        clip: PixelRect,
    ) {
        let [r, g, b, a] = color.as_rgba();
        if a == 0 {
            return;
        }
        let Some((x0, y0, x1, y1)) = self.clipped_rect(x, y, width, height) else {
            return;
        };
        let Some((clip_x0, clip_y0, clip_x1, clip_y1)) =
            self.clipped_rect(clip.x as i32, clip.y as i32, clip.width, clip.height)
        else {
            return;
        };
        let x0 = x0.max(clip_x0);
        let y0 = y0.max(clip_y0);
        let x1 = x1.min(clip_x1);
        let y1 = y1.min(clip_y1);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.blend_pixel(xx, yy, r, g, b, a);
            }
        }
    }

    /// Draw the configured input sequence as compact key chips over the bottom of the frame.
    fn draw_keyboard_overlay(
        &mut self,
        labels: &[String],
        text_settings: &TextSettings,
        bottom_y: u32,
        right_x: u32,
    ) {
        let labels = recent_keyboard_overlay_labels(labels);
        let rows = keyboard_overlay_rows(&labels, self.width);
        if rows.is_empty() {
            return;
        }

        let row_height = KEYBOARD_OVERLAY_LINE_HEIGHT.ceil() as u32;
        let row_width = rows
            .iter()
            .map(|row| keyboard_overlay_row_width(row))
            .max()
            .unwrap_or(0);
        let panel_right_x = right_x.min(self.width);
        let panel_width = row_width.min(panel_right_x);
        let panel_height = KEYBOARD_OVERLAY_INSET_Y
            .saturating_add(row_height.saturating_mul(rows.len() as u32))
            .min(self.height);
        let panel_x = panel_right_x.saturating_sub(panel_width);
        let panel_y = bottom_y.min(self.height).saturating_sub(panel_height);
        let mut text = OverlayTextRenderer::new(text_settings.font_family.clone());
        let mut y = panel_y.saturating_add(KEYBOARD_OVERLAY_INSET_Y);
        for row in rows {
            let row_width = keyboard_overlay_row_width(&row);
            let mut x = panel_x.saturating_add(panel_width.saturating_sub(row_width));
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
                let coverage =
                    rounded_corner_coverage(xx, yy, cx, cy, radius as f32, ROUNDED_RECT_AA_SAMPLES);
                if coverage <= 0.0 {
                    let offset = self.offset(xx, yy);
                    self.pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&[
                        fill.r,
                        fill.g,
                        fill.b,
                        OPAQUE_ALPHA,
                    ]);
                } else if coverage < 1.0 {
                    let alpha = ((1.0 - coverage) * f32::from(OPAQUE_ALPHA))
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    self.blend_pixel(xx, yy, fill.r, fill.g, fill.b, alpha);
                }
            }
        }
    }

    /// Return the byte offset for a pixel in the tightly packed RGBA buffer.
    fn offset(&self, x: u32, y: u32) -> usize {
        ((y as usize * self.width as usize) + x as usize) * BYTES_PER_PIXEL
    }

    /// Return a clipped rectangle as `(x0, y0, x1, y1)`.
    fn clipped_rect(
        &self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<(u32, u32, u32, u32)> {
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let x1 = x.saturating_add(width as i32).clamp(0, self.width as i32) as u32;
        let y1 = y.saturating_add(height as i32).clamp(0, self.height as i32) as u32;
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
    }

    /// Alpha-blend one pixel over the existing frame color.
    fn blend_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        let offset = self.offset(x, y);
        let alpha = u16::from(a);
        let inv_alpha = ALPHA_DENOMINATOR - alpha;
        self.pixels[offset] = ((u16::from(r) * alpha + u16::from(self.pixels[offset]) * inv_alpha)
            / ALPHA_DENOMINATOR) as u8;
        self.pixels[offset + 1] = ((u16::from(g) * alpha
            + u16::from(self.pixels[offset + 1]) * inv_alpha)
            / ALPHA_DENOMINATOR) as u8;
        self.pixels[offset + 2] = ((u16::from(b) * alpha
            + u16::from(self.pixels[offset + 2]) * inv_alpha)
            / ALPHA_DENOMINATOR) as u8;
        self.pixels[offset + 3] = OPAQUE_ALPHA;
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

/// Estimate how much of one pixel is inside a rounded-rectangle corner circle.
fn rounded_corner_coverage(x: u32, y: u32, cx: i32, cy: i32, radius: f32, samples: u32) -> f32 {
    if samples == 0 {
        return 0.0;
    }
    let sample_step = 1.0 / samples as f32;
    let radius_sq = radius * radius;
    let mut covered = 0;
    for sample_y in 0..samples {
        for sample_x in 0..samples {
            let px = x as f32 + (sample_x as f32 + 0.5) * sample_step;
            let py = y as f32 + (sample_y as f32 + 0.5) * sample_step;
            let dx = px - cx as f32;
            let dy = py - cy as f32;
            if dx * dx + dy * dy <= radius_sq {
                covered += 1;
            }
        }
    }
    covered as f32 / (samples * samples) as f32
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

    fn pixel(frame: &SolidFrame, x: u32, y: u32) -> [u8; 4] {
        let offset = frame.offset(x, y);
        [
            frame.pixels[offset],
            frame.pixels[offset + 1],
            frame.pixels[offset + 2],
            frame.pixels[offset + 3],
        ]
    }

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
    fn caption_glyph_blending_is_clipped_to_the_caption_rect() {
        let mut frame = SolidFrame::new(10, 10, rgb(0, 0, 0)).unwrap();

        frame.blend_rect_clipped(
            0,
            0,
            10,
            10,
            Color::rgb(255, 0, 0),
            PixelRect {
                x: 2,
                y: 3,
                width: 4,
                height: 2,
            },
        );

        assert_eq!(pixel(&frame, 1, 3), [0, 0, 0, 255]);
        assert_ne!(pixel(&frame, 2, 3), [0, 0, 0, 255]);
        assert_ne!(pixel(&frame, 5, 4), [0, 0, 0, 255]);
        assert_eq!(pixel(&frame, 6, 4), [0, 0, 0, 255]);
        assert_eq!(pixel(&frame, 2, 5), [0, 0, 0, 255]);
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
        assert_eq!(settings.keyboard_overlay_right_x(), 304);
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
        assert_eq!(settings.keyboard_overlay_right_x(), 300);
        assert_eq!(layout.text_x, 20);
    }
}
