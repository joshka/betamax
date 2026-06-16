//! Tape execution orchestration.
//!
//! The runner is the boundary between the parsed tape language and the side-effecting world:
//! spawning a PTY, feeding bytes into libghostty-vt, deciding when to capture frames, and writing
//! requested artifacts. Lower-level concerns such as key byte encoding, shell normalization, output
//! classification, and wait matching live in small modules so this file can focus on sequencing.
//!
//! # Examples
//!
//! ```no_run
//! use betamax_core::runner::{RunOptions, Runner};
//! use betamax_core::Tape;
//!
//! # fn main() -> betamax_core::Result<()> {
//! let tape = Tape::parse(
//!     r#"
//! Output /tmp/betamax-state.json
//! Set Shell "bash"
//! Type "printf 'ready\n'"
//! Enter
//! Wait+Screen "ready"
//! "#,
//! )?;
//! let artifacts = Runner::new(RunOptions::default()).run_artifacts(&tape)?;
//!
//! assert_eq!(artifacts.output_paths.len(), 1);
//! assert!(artifacts
//!     .final_state
//!     .unwrap()
//!     .viewport_text
//!     .contains("ready"));
//! # Ok(())
//! # }
//! ```

use std::thread;
use std::time::Duration;

use miette::miette;

use crate::Result;

mod artifacts;
mod capture;
mod options;
mod pty;
mod requirements;
mod settings;

#[doc(inline)]
pub use artifacts::RunArtifacts;
use capture::{append_final_gif_frame, append_visible_frame, capture_frame, CaptureState};
#[doc(inline)]
pub use options::RunOptions;
use pty::PtySession;
use requirements::validate_required_programs;
use settings::Settings;

use crate::ghostty::{CaptureRequest, GhosttyFrameCapture, PixelSize, TerminalGrid};
use crate::key::key_bytes;
use crate::media::{
    write_gif, write_json, write_mp4, write_png, write_png_sequence, write_webm, Frame,
};
use crate::output::{classify_outputs, Outputs};
use crate::tape::{Command, Key, KeyCode, Tape, Value, WaitPattern, WaitTarget};

/// Terminal session used by the runner's capture path.
///
/// This trait is the narrow testability seam around terminal rendering and state capture. The
/// runner still owns PTY spawning, command timing, filesystem writes, and media encoding, but tests
/// and future integrations can replace the capture backend without depending on libghostty-vt.
pub trait TerminalSession {
    /// Feed raw PTY bytes into the terminal model.
    fn write_vt(&mut self, bytes: &[u8]);

    /// Capture one raw terminal frame with caller-controlled cursor visibility.
    ///
    /// # Errors
    ///
    /// Returns an error when the terminal backend cannot render a frame.
    fn capture_frame_with_cursor(&mut self, cursor_visible: bool) -> Result<Frame>;

    /// Return visible terminal text for wait matching.
    ///
    /// # Errors
    ///
    /// Returns an error when the terminal backend cannot expose visible text.
    fn screen_text(&mut self) -> Result<String>;

    /// Return structured terminal state for JSON output and snapshot assertions.
    ///
    /// # Errors
    ///
    /// Returns an error when the terminal backend cannot expose structured state.
    fn terminal_state(&mut self) -> Result<crate::ghostty::TerminalState>;

    /// Take reply bytes the emulator wants written back to the PTY master,
    /// e.g. the response to a cursor position query (`ESC[6n`). Default is
    /// empty for backends that don't model device queries.
    fn take_pending_pty_reply(&mut self) -> Vec<u8> {
        Vec::new()
    }
}

/// Opens terminal sessions for captured tape runs.
pub trait FrameCapture {
    /// Live terminal session returned by this backend.
    type Session: TerminalSession;

    /// Open a capture session for a derived terminal request.
    ///
    /// # Errors
    ///
    /// Returns an error when the capture backend cannot initialize the requested terminal.
    fn open(&mut self, request: CaptureRequest) -> Result<Self::Session>;
}

/// Idle period used to let an interactive shell settle before tape commands begin.
///
/// Without this initial drain, examples can start typing before the configured prompt has rendered.
/// The value is intentionally a short idle timeout rather than a fixed sleep: startup output is
/// consumed as it arrives, and command execution begins once the shell has been quiet for this
/// long.
const SHELL_STARTUP_IDLE: Duration = Duration::from_millis(250);
/// Final idle drain after all tape commands have run.
///
/// This catches command output that arrives just after the last scripted input without adding a
/// noticeable tail to generated GIFs or videos.
const FINAL_COMMAND_IDLE: Duration = Duration::from_millis(100);
/// Idle drain before point-in-time state or screenshot commands.
///
/// Inline checkpoints should reflect output produced by the preceding command, but they should not
/// wait as long as shell startup because they often appear in the middle of visible recordings.
const CHECKPOINT_IDLE: Duration = Duration::from_millis(50);

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_DIM: &str = "\x1b[2m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";

/// Runs parsed tapes.
///
/// The default `Runner` uses [`GhosttyFrameCapture`] for in-process terminal state and raster
/// frames. The capture type is generic internally so tests and future library users can swap in a
/// compatible implementation without changing tape parsing.
pub struct Runner<C = GhosttyFrameCapture> {
    /// User-selected run behavior that is independent of the tape contents.
    options: RunOptions,
    /// Capture implementation used when any command or output needs rendered terminal state.
    capture: C,
}

impl<C> std::fmt::Debug for Runner<C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Runner")
            .field("options", &self.options)
            .field("capture", &std::any::type_name::<C>())
            .finish()
    }
}

impl Runner<GhosttyFrameCapture> {
    /// Construct a runner backed by the built-in libghostty-vt raster capture implementation.
    pub fn new(options: RunOptions) -> Self {
        Self {
            options,
            capture: GhosttyFrameCapture,
        }
    }
}

impl<C> Runner<C> {
    /// Construct a runner with a caller-provided capture backend.
    ///
    /// This is primarily useful for tests and future embedders that want to replace terminal
    /// rendering/state capture while still exercising Betamax's tape execution loop.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use betamax_core::ghostty::{
    ///     CaptureRequest, StateCursor, StateSpan, StateStyle, TerminalState,
    /// };
    /// use betamax_core::media::{Frame, PixelFormat};
    /// use betamax_core::{FrameCapture, Result, RunOptions, Runner, Tape, TerminalSession};
    ///
    /// #[derive(Debug)]
    /// struct FakeCapture;
    ///
    /// impl FrameCapture for FakeCapture {
    ///     type Session = FakeSession;
    ///
    ///     fn open(&mut self, request: CaptureRequest) -> Result<Self::Session> {
    ///         assert!(request.canvas.width > 0);
    ///         Ok(FakeSession::default())
    ///     }
    /// }
    ///
    /// #[derive(Debug, Default)]
    /// struct FakeSession {
    ///     text: String,
    /// }
    ///
    /// impl TerminalSession for FakeSession {
    ///     fn write_vt(&mut self, bytes: &[u8]) {
    ///         self.text.push_str(&String::from_utf8_lossy(bytes));
    ///     }
    ///
    ///     fn capture_frame_with_cursor(&mut self, _cursor_visible: bool) -> Result<Frame> {
    ///         Ok(Frame {
    ///             width: 1,
    ///             height: 1,
    ///             stride: 4,
    ///             format: PixelFormat::Rgba8,
    ///             pixels: vec![0, 0, 0, 255],
    ///         })
    ///     }
    ///
    ///     fn screen_text(&mut self) -> Result<String> {
    ///         Ok(self.text.clone())
    ///     }
    ///
    ///     fn terminal_state(&mut self) -> Result<TerminalState> {
    ///         Ok(TerminalState {
    ///             size: [1, 1],
    ///             total_rows: 1,
    ///             scrollback_rows: 0,
    ///             title: String::new(),
    ///             working_directory: String::new(),
    ///             cursor: StateCursor {
    ///                 x: 0,
    ///                 y: 0,
    ///                 visible: false,
    ///             },
    ///             default_style: StateStyle::default(),
    ///             styles: Vec::new(),
    ///             viewport_text: self.text.clone(),
    ///             scrollback_text: String::new(),
    ///             viewport: vec![vec![StateSpan::Text(self.text.clone())]],
    ///             scrollback: Vec::new(),
    ///         })
    ///     }
    /// }
    ///
    /// # fn main() -> betamax_core::Result<()> {
    /// let tape = Tape::parse(r#"Output /tmp/state.json Type "ok" Wait+Screen "ok""#)?;
    /// let mut runner = Runner::with_capture(RunOptions::default(), FakeCapture);
    /// let _artifacts = runner.run_artifacts(&tape)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_capture(options: RunOptions, capture: C) -> Self {
        Self { options, capture }
    }
}

impl<C> Runner<C>
where
    C: FrameCapture,
{
    /// Execute a tape and discard returned artifacts.
    ///
    /// This is the CLI-oriented entry point. Library callers that need the final terminal state or
    /// the resolved output paths should use [`Runner::run_artifacts`].
    ///
    /// # Errors
    ///
    /// Returns an error when tape settings are invalid, required programs are missing, PTY or shell
    /// startup fails, terminal capture fails, requested media/state outputs cannot be written, or a
    /// parsed-but-unimplemented command is executed.
    pub fn run(&mut self, tape: &Tape) -> Result<()> {
        self.run_artifacts(tape).map(|_| ())
    }

    /// Execute a tape and return metadata about the run.
    ///
    /// If the tape has no capture-dependent commands or outputs, this can run without libghostty-vt
    /// frame capture and returns `final_state: None`. Any GIF/PNG/video/state output, screenshot,
    /// state command, or wait command requires capture and returns the final terminal state.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use betamax_core::{RunOptions, Runner, Tape};
    ///
    /// # fn main() -> betamax_core::Result<()> {
    /// let tape = Tape::parse(
    ///     r#"
    /// Output /tmp/betamax-state.json
    /// Type "printf 'ready\n'"
    /// Enter
    /// Wait+Screen "ready"
    /// "#,
    /// )?;
    /// let artifacts = Runner::new(RunOptions::default()).run_artifacts(&tape)?;
    ///
    /// assert!(artifacts.final_state.is_some());
    /// assert_eq!(artifacts.output_paths.len(), 1);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when tape settings are invalid, required programs are missing, PTY or shell
    /// startup fails, terminal capture fails, requested media/state outputs cannot be written, a
    /// wait times out, or a parsed-but-unimplemented command is executed.
    pub fn run_artifacts(&mut self, tape: &Tape) -> Result<RunArtifacts> {
        if self.options.publish {
            return Err(miette!("publish is not implemented in the Ghostty-first runner").into());
        }

        let outputs = classify_outputs(tape)?;
        validate_required_programs(tape)?;

        if outputs.requires_capture() {
            return self.run_with_capture(tape, outputs);
        }

        self.run_without_capture(tape)
    }

    /// Run a tape through the full PTY -> libghostty-vt -> raster pipeline.
    ///
    /// This path is selected whenever a requested artifact or command needs the terminal renderer.
    /// It captures frames only while [`Command::Show`] is active, but it always continues to feed
    /// PTY bytes into the terminal so hidden cleanup commands can affect final state without
    /// appearing in GIF/video output.
    fn run_with_capture(&mut self, tape: &Tape, outputs: Outputs) -> Result<RunArtifacts> {
        let settings = Settings::from_tape(tape)?;
        tracing::debug!(
            columns = settings.columns,
            rows = settings.rows,
            width = settings.width,
            height = settings.height,
            commands = tape.commands.len(),
            "starting captured tape run",
        );
        tracing::debug!("opening Ghostty frame capture");
        let mut terminal = self.capture.open(CaptureRequest {
            canvas: PixelSize::new(settings.width, settings.height),
            grid: TerminalGrid::new(settings.columns, settings.rows),
            text: settings.text.clone(),
            theme: settings.theme.clone(),
        })?;
        tracing::trace!("opened Ghostty frame capture");
        tracing::debug!("spawning PTY session");
        let mut session = PtySession::spawn(&settings)?;
        tracing::trace!("spawned PTY session");
        let mut capture = CaptureState::default();
        let mut clipboard = String::new();
        tracing::debug!("draining PTY shell startup output");
        session.drain_into(&mut terminal, SHELL_STARTUP_IDLE)?;
        tracing::trace!("drained PTY shell startup output");

        let command_count = tape.commands.len();
        for (index, command) in tape.commands.iter().enumerate() {
            self.progress_command(index + 1, command_count, command);
            let span = tracing::debug_span!(
                "tape_command",
                index = index + 1,
                count = command_count,
                kind = command_kind(command),
            );
            let _enter = span.enter();
            self.run_capture_command(
                command,
                &settings,
                &mut session,
                &mut terminal,
                &mut capture,
                &mut clipboard,
            )?;
        }

        tracing::debug!("draining final PTY output");
        session.drain_into(&mut terminal, FINAL_COMMAND_IDLE)?;
        tracing::trace!("drained final PTY output");
        let frame = settings.decorate_frame(&capture_frame(
            &mut terminal,
            &settings,
            capture.frames.len(),
        )?)?;
        settings.decorate_frames(&mut capture.frames)?;
        if !outputs.gifs.is_empty()
            || !outputs.mp4s.is_empty()
            || !outputs.webms.is_empty()
            || !outputs.frame_dirs.is_empty()
        {
            append_final_gif_frame(&mut capture, frame.clone(), settings.frame_delay());
            settings.apply_loop_offset(&mut capture.frames);
        }

        let output_paths = outputs.paths();
        tracing::debug!("reading final terminal state");
        let final_state = terminal.terminal_state()?;
        tracing::trace!("read final terminal state");

        for path in outputs.pngs {
            self.progress(
                ANSI_YELLOW,
                format_args!("generating png {}", path.display()),
            );
            write_png(&path, &frame)?;
            self.progress(ANSI_GREEN, format_args!("wrote {}", path.display()));
        }
        for path in outputs.states {
            self.progress(
                ANSI_YELLOW,
                format_args!("writing state {}", path.display()),
            );
            write_json(&path, &final_state)?;
            self.progress(ANSI_GREEN, format_args!("wrote {}", path.display()));
        }
        for path in outputs.gifs {
            self.progress(
                ANSI_YELLOW,
                format_args!("combining frames into gif {}", path.display()),
            );
            write_gif(&path, &capture.frames)?;
            self.progress(ANSI_GREEN, format_args!("wrote {}", path.display()));
        }
        for path in outputs.frame_dirs {
            self.progress(
                ANSI_YELLOW,
                format_args!("writing frame sequence {}", path.display()),
            );
            write_png_sequence(&path, &capture.frames)?;
            self.progress(ANSI_GREEN, format_args!("wrote {}", path.display()));
        }
        for path in outputs.mp4s {
            self.progress(ANSI_YELLOW, format_args!("encoding mp4 {}", path.display()));
            write_mp4(&path, &capture.frames, settings.output_framerate())?;
            self.progress(ANSI_GREEN, format_args!("wrote {}", path.display()));
        }
        for path in outputs.webms {
            self.progress(
                ANSI_YELLOW,
                format_args!("encoding webm {}", path.display()),
            );
            write_webm(&path, &capture.frames, settings.output_framerate())?;
            self.progress(ANSI_GREEN, format_args!("wrote {}", path.display()));
        }

        Ok(RunArtifacts {
            final_state: Some(final_state),
            output_paths,
        })
    }

    /// Execute one command while frame capture is available.
    ///
    /// This method is intentionally command-focused rather than media-focused. It mutates the PTY,
    /// terminal, frame buffer, and tape-local clipboard as needed; final artifact writing happens
    /// only after the tape has finished so all outputs are derived from the same terminal state.
    fn run_capture_command(
        &self,
        command: &Command,
        settings: &Settings,
        session: &mut PtySession,
        terminal: &mut C::Session,
        capture: &mut CaptureState,
        clipboard: &mut String,
    ) -> Result<()> {
        tracing::trace!(
            kind = command_kind(command),
            "running captured tape command"
        );
        match command {
            Command::Sleep(duration) => {
                session.drain_for(terminal, *duration, settings, capture)?;
            }
            Command::Type { text, delay } => {
                let delay = delay.unwrap_or(settings.typing_delay);
                for ch in text.chars() {
                    let mut buf = [0u8; 4];
                    session.write_all(ch.encode_utf8(&mut buf).as_bytes())?;
                    session.drain_for(terminal, delay, settings, capture)?;
                }
            }
            Command::Key { key, delay, count } => {
                for _ in 0..*count {
                    let bytes = key_bytes(key)?;
                    session.write_all(&bytes)?;
                    session.drain_for(
                        terminal,
                        delay.unwrap_or(settings.frame_delay()),
                        settings,
                        capture,
                    )?;
                }
            }
            Command::Wait {
                target,
                pattern,
                timeout,
            } => {
                let timeout = timeout.unwrap_or(settings.wait_timeout);
                session.wait_for(
                    terminal,
                    *target,
                    pattern.as_ref().unwrap_or(&settings.wait_pattern),
                    timeout,
                    settings,
                    capture,
                )?;
            }
            Command::Copy(text) => *clipboard = text.clone(),
            Command::Paste => {
                session.write_all(clipboard.as_bytes())?;
                session.drain_for(terminal, settings.frame_delay(), settings, capture)?;
            }
            Command::Screenshot(path) => {
                session.drain_into(terminal, CHECKPOINT_IDLE)?;
                write_png(
                    path,
                    &settings.decorate_frame(&capture_frame(
                        terminal,
                        settings,
                        capture.frames.len(),
                    )?)?,
                )?;
            }
            Command::State(path) => {
                session.drain_into(terminal, CHECKPOINT_IDLE)?;
                write_json(path, &terminal.terminal_state()?)?;
            }
            Command::Hide => capture.visible = false,
            Command::Show => {
                capture.visible = true;
                append_visible_frame(
                    capture,
                    capture_frame(terminal, settings, capture.frames.len())?,
                    settings.frame_delay(),
                );
            }
            Command::Env { .. }
            | Command::Output(_)
            | Command::Require(_)
            | Command::Set { .. } => {}
            Command::Source(path) => {
                return Err(
                    miette!("Source is parsed but not executed yet: {}", path.display()).into(),
                );
            }
        }

        Ok(())
    }

    /// Run a tape that does not require terminal state or rendered frames.
    ///
    /// This path exists for command validation and future non-capture workflows. It still spawns
    /// the configured shell and sends input, but commands that inherently need terminal state
    /// fail with a targeted error if output classification did not already route the tape to
    /// capture.
    fn run_without_capture(&mut self, tape: &Tape) -> Result<RunArtifacts> {
        let settings = Settings::from_tape(tape)?;
        tracing::debug!(
            commands = tape.commands.len(),
            "starting non-captured tape run",
        );
        let mut session = PtySession::spawn(&settings)?;
        let mut clipboard = String::new();
        session.drain_output(SHELL_STARTUP_IDLE)?;
        let command_count = tape.commands.len();
        for (index, command) in tape.commands.iter().enumerate() {
            self.progress_command(index + 1, command_count, command);
            let span = tracing::debug_span!(
                "tape_command",
                index = index + 1,
                count = command_count,
                kind = command_kind(command),
            );
            let _enter = span.enter();
            match command {
                Command::Sleep(duration) => thread::sleep(*duration),
                Command::Type { text, delay } => session.type_text(text, *delay)?,
                Command::Key { key, delay, count } => {
                    for _ in 0..*count {
                        let bytes = key_bytes(key)?;
                        session.write_all(&bytes)?;
                        if let Some(delay) = delay {
                            thread::sleep(*delay);
                        }
                    }
                }
                Command::Copy(text) => clipboard = text.clone(),
                Command::Paste => session.write_all(clipboard.as_bytes())?,
                Command::Env { .. }
                | Command::Hide
                | Command::Output(_)
                | Command::Require(_)
                | Command::Set { .. }
                | Command::Show => {}
                Command::Wait { .. } => {
                    return Err(miette!(
                        "Wait is parsed but not executed until libghostty-vt screen-state matching is wired"
                    ).into());
                }
                Command::Source(path) => {
                    return Err(miette!(
                        "Source is parsed but not executed yet: {}",
                        path.display()
                    )
                    .into());
                }
                Command::Screenshot(path) => {
                    return Err(miette!(
                        "Screenshot requires Ghostty frame capture, which is not implemented yet: {}",
                        path.display()
                    ).into());
                }
                Command::State(path) => {
                    return Err(miette!(
                        "State requires Ghostty terminal state, which is not available without capture: {}",
                        path.display()
                    ).into());
                }
            }
        }

        self.progress(
            ANSI_GREEN,
            format_args!("tape executed without capture outputs"),
        );

        Ok(RunArtifacts {
            final_state: None,
            output_paths: Vec::new(),
        })
    }

    fn progress_command(&self, index: usize, count: usize, command: &Command) {
        let (style, label) = match command {
            Command::Output(_)
            | Command::Require(_)
            | Command::Set { .. }
            | Command::Env { .. } => (ANSI_DIM, "setup"),
            Command::Sleep(_) | Command::Wait { .. } => (ANSI_YELLOW, "wait"),
            Command::Screenshot(_) | Command::State(_) => (ANSI_YELLOW, "capture"),
            Command::Hide | Command::Show => (ANSI_DIM, "display"),
            _ => (ANSI_CYAN, "run"),
        };
        self.progress(
            style,
            format_args!(
                "{ANSI_BOLD}{label}{ANSI_RESET}{style} {index}/{count}: {}",
                describe_command(command)
            ),
        );
    }

    fn progress(&self, style: &str, args: std::fmt::Arguments<'_>) {
        if !self.options.quiet {
            println!("{style}{args}{ANSI_RESET}");
        }
    }
}

fn describe_command(command: &Command) -> String {
    match command {
        Command::Output(path) => format!("Output {}", path.display()),
        Command::Require(program) => format!("Require {program}"),
        Command::Set { key, value } => format!("Set {key} {}", describe_value(value)),
        Command::Sleep(duration) => format!("Sleep {}", describe_duration(*duration)),
        Command::Type { text, delay } => match delay {
            Some(delay) => format!(
                "Type@{} \"{}\"",
                describe_duration(*delay),
                describe_text(text)
            ),
            None => format!("Type \"{}\"", describe_text(text)),
        },
        Command::Wait {
            target,
            pattern,
            timeout,
        } => {
            let target = match target {
                WaitTarget::Line => "Line",
                WaitTarget::Screen => "Screen",
            };
            let pattern = pattern
                .as_ref()
                .map(describe_wait_pattern)
                .unwrap_or_else(|| "default prompt".to_string());
            match timeout {
                Some(timeout) => {
                    format!("Wait+{target}@{} {pattern}", describe_duration(*timeout))
                }
                None => format!("Wait+{target} {pattern}"),
            }
        }
        Command::Key { key, delay, count } => {
            let key = describe_key(key);
            let suffix = delay
                .map(|delay| format!("@{}", describe_duration(delay)))
                .unwrap_or_default();
            if *count == 1 {
                format!("{key}{suffix}")
            } else {
                format!("{key}{suffix} {count}")
            }
        }
        Command::Hide => "Hide".to_string(),
        Command::Show => "Show".to_string(),
        Command::Env { key, .. } => format!("Env {key} <value>"),
        Command::Copy(text) => format!("Copy \"{}\"", describe_text(text)),
        Command::Paste => "Paste".to_string(),
        Command::Source(path) => format!("Source {}", path.display()),
        Command::Screenshot(path) => format!("Screenshot {}", path.display()),
        Command::State(path) => format!("State {}", path.display()),
    }
}

fn command_kind(command: &Command) -> &'static str {
    match command {
        Command::Output(_) => "Output",
        Command::Require(_) => "Require",
        Command::Set { .. } => "Set",
        Command::Sleep(_) => "Sleep",
        Command::Type { .. } => "Type",
        Command::Wait { .. } => "Wait",
        Command::Key { .. } => "Key",
        Command::Hide => "Hide",
        Command::Show => "Show",
        Command::Env { .. } => "Env",
        Command::Copy(_) => "Copy",
        Command::Paste => "Paste",
        Command::Screenshot(_) => "Screenshot",
        Command::State(_) => "State",
        Command::Source(_) => "Source",
    }
}

fn describe_value(value: &Value) -> String {
    match value {
        Value::String(value) => format!("\"{}\"", describe_text(value)),
        Value::Number(value) => value.to_string(),
        Value::Duration(value) => describe_duration(*value),
        Value::Bool(value) => value.to_string(),
    }
}

fn describe_wait_pattern(pattern: &WaitPattern) -> String {
    match pattern {
        WaitPattern::Contains(text) => format!("\"{}\"", describe_text(text)),
        WaitPattern::Regex(regex) => format!("/{}/", describe_text(regex)),
    }
}

fn describe_key(key: &Key) -> String {
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
    parts.push(match key {
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
    });
    parts.join("+")
}

fn describe_duration(duration: Duration) -> String {
    let milliseconds = duration.as_millis();
    if milliseconds.is_multiple_of(1000) {
        format!("{}s", milliseconds / 1000)
    } else {
        format!("{milliseconds}ms")
    }
}

fn describe_text(text: &str) -> String {
    const MAX_CHARS: usize = 72;
    let mut output = String::new();
    let mut chars = text.chars();
    for ch in chars.by_ref().take(MAX_CHARS) {
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
        output.push('…');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::Frame;
    use crate::runner::capture::frames_equal;

    #[test]
    fn uses_vhs_typography_defaults() {
        let tape = Tape::parse("").unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.width, 1200);
        assert_eq!(settings.height, 600);
        assert_eq!(settings.framerate, 50);
        assert_eq!(settings.typing_delay, Duration::from_millis(50));
        assert_eq!(settings.text.font_size, 22.0);
        assert_eq!(settings.text.font_family.as_deref(), Some("JetBrains Mono"));
        assert_eq!(settings.text.letter_spacing, 1.0);
        assert_eq!(settings.text.line_height, 1.0);
        assert_eq!(settings.text.padding, 60);
        assert_eq!(settings.theme.name, "Aardvark Blue");
        assert_eq!(settings.style.margin, 0);
        assert_eq!(settings.style.margin_fill, "#102040");
        assert_eq!(settings.style.window_bar, "");
        assert_eq!(settings.style.window_bar_size, 30);
        assert_eq!(settings.style.window_bar_color, "#102040");
        assert_eq!(settings.style.border_radius, 0);
    }

    #[test]
    fn accepts_aardvark_blue_theme() {
        let tape = Tape::parse(r#"Set Theme "Aardvark Blue""#).unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.theme.name, "Aardvark Blue");
    }

    #[test]
    fn parses_frame_style_settings() {
        let tape = Tape::parse(
            r##"
            Set Margin 12
            Set MarginFill "#674EFF"
            Set WindowBar Colorful
            Set WindowBarSize 40
            Set BorderRadius 8
            "##,
        )
        .unwrap();

        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.style.margin, 12);
        assert_eq!(settings.style.margin_fill, "#674EFF");
        assert_eq!(settings.style.window_bar, "Colorful");
        assert_eq!(settings.style.window_bar_size, 40);
        assert_eq!(settings.style.border_radius, 8);
    }

    #[test]
    fn unknown_settings_are_errors() {
        let tape = Tape::parse("Set Wdith 900").unwrap();
        let error = Settings::from_tape(&tape).unwrap_err().to_string();

        assert!(error.contains("unsupported Set setting `Wdith`"));
    }

    #[test]
    fn type_mismatched_settings_are_errors() {
        let tape = Tape::parse(r#"Set Width "wide""#).unwrap();
        let error = Settings::from_tape(&tape).unwrap_err().to_string();

        assert!(error.contains("Set Width expects number, got string"));
    }

    #[test]
    fn playback_speed_changes_output_delay_not_capture_cadence() {
        let tape = Tape::parse(
            r#"
            Set Framerate 50
            Set PlaybackSpeed 2
            "#,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(settings.capture_interval(), Duration::from_millis(20));
        assert_eq!(settings.frame_delay(), Duration::from_millis(10));
        assert_eq!(settings.output_framerate(), 100.0);
    }

    #[test]
    fn trailing_hide_keeps_cleanup_frame_out_of_gif() {
        let visible_frame = test_frame([255, 0, 0, 255]);
        let cleanup_frame = test_frame([0, 255, 0, 255]);
        let mut capture = CaptureState {
            visible: false,
            frames: vec![(visible_frame.clone(), Duration::from_millis(20))],
        };

        append_final_gif_frame(&mut capture, cleanup_frame, Duration::from_millis(20));

        assert_eq!(capture.frames.len(), 1);
        assert!(frames_equal(&capture.frames[0].0, &visible_frame));
    }

    #[test]
    fn repeated_visible_frames_extend_delay() {
        let frame = test_frame([255, 0, 0, 255]);
        let mut capture = CaptureState::default();

        append_visible_frame(&mut capture, frame.clone(), Duration::from_millis(20));
        append_visible_frame(&mut capture, frame.clone(), Duration::from_millis(20));
        append_final_gif_frame(&mut capture, frame.clone(), Duration::from_millis(20));

        assert_eq!(capture.frames.len(), 1);
        assert_eq!(capture.frames[0].1, Duration::from_millis(60));
        assert!(frames_equal(&capture.frames[0].0, &frame));
    }

    fn test_frame(pixel: [u8; 4]) -> Frame {
        Frame {
            width: 1,
            height: 1,
            stride: 4,
            format: crate::media::PixelFormat::Rgba8,
            pixels: pixel.to_vec(),
        }
    }
}
