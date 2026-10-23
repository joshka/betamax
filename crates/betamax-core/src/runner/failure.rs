//! Failure bundles: what a failed tape leaves behind so it can be debugged without a rerun.
//!
//! When a tape command fails, the runner writes one directory holding the terminal as it was at
//! the failure (screenshot, state JSON, cells) and a report naming the command, its tape line, how
//! long it ran, what it was waiting for, and whether the shell was still running. A missing
//! `Require` program fails before the shell starts, so its bundle holds only the report.
//!
//! Bundles go under the root chosen with [`Runner::with_failure_dir`](super::Runner), else
//! `BETAMAX_FAILURE_DIR`, else `target/betamax-failures`, in a new directory per failure so
//! repeated or parallel runs never overwrite each other.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::capture::{capture_frame, CaptureState};
use super::pty::ShellStatus;
use super::settings::Settings;
use super::{describe_command, TerminalSession};
use crate::ghostty::TerminalState;
use crate::media::{write_json, write_png};
use crate::tape::{Command, Tape, WaitPattern};
use crate::wait::wait_target_name;

/// Environment variable naming the bundle root when the caller did not choose one.
const FAILURE_DIR_ENV: &str = "BETAMAX_FAILURE_DIR";
/// Bundle root used when neither the caller nor the environment chooses one.
const DEFAULT_FAILURE_DIR: &str = "target/betamax-failures";
/// Version of the `report.json` layout, raised when a field changes meaning or is removed.
const REPORT_SCHEMA: u32 = 1;
/// Commands listed in the report, ending with the failed one.
const RECENT_COMMANDS: usize = 6;
/// Scrollback lines copied into `report.txt`; `actual.json` holds all of them.
const SCROLLBACK_EXCERPT_LINES: usize = 20;

const SCREENSHOT_FILE: &str = "actual.png";
const STATE_FILE: &str = "actual.json";
const CELLS_FILE: &str = "cells.json";
const ERROR_FILE: &str = "diff.txt";
const REPORT_TEXT_FILE: &str = "report.txt";
const REPORT_JSON_FILE: &str = "report.json";

/// Resolve the bundle root: the caller's choice, then `BETAMAX_FAILURE_DIR`, then the default.
pub(super) fn failure_root(explicit: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os(FAILURE_DIR_ENV).map(PathBuf::from))
        .unwrap_or_else(|| DEFAULT_FAILURE_DIR.into())
}

/// A failed tape command and the context the report describes.
pub(super) struct Failure<'a> {
    /// Tape that failed, used for command text, locations, and recent commands.
    pub(super) tape: &'a Tape,
    /// Zero-based index of the failed command.
    pub(super) index: usize,
    /// Error returned by the command.
    pub(super) error: &'a crate::Error,
    /// How long the failed command ran, or `None` if it failed before the shell started.
    pub(super) elapsed: Option<Duration>,
    /// Time since the shell started.
    pub(super) run_elapsed: Option<Duration>,
    /// The resolved pattern, target, and timeout of a failed `Wait`.
    pub(super) wait: Option<WaitReport>,
    /// Whether the shell was still running at the failure.
    pub(super) shell: ShellStatus,
}

impl Failure<'_> {
    /// Message for the returned error: where the command is, what it was, how long it ran, and
    /// why it failed, then where the bundle went.
    pub(super) fn message(&self, bundle: &str) -> String {
        let command = &self.tape.commands[self.index];
        let mut message = format!(
            "{}: `{}` failed",
            self.location(),
            describe_command(command)
        );
        if let Some(elapsed) = self.elapsed {
            write!(message, " after {}", seconds(elapsed)).unwrap();
        }
        write!(
            message,
            " (tape command {} of {}): {}\nfailure bundle: {bundle}",
            self.index + 1,
            self.tape.commands.len(),
            self.error_text()
        )
        .unwrap();
        message
    }

    fn location(&self) -> String {
        location(self.tape, self.index)
    }

    /// The error with its causes, such as the I/O error behind an unreadable baseline.
    fn error_text(&self) -> String {
        let mut text = self.error.to_string();
        let mut source = std::error::Error::source(self.error);
        while let Some(cause) = source {
            write!(text, ": {cause}").unwrap();
            source = cause.source();
        }
        text
    }
}

/// Wait details resolved from the tape's defaults, so the report shows what was really awaited.
#[derive(Debug, Clone, Serialize)]
pub(super) struct WaitReport {
    /// `screen` or `current line`.
    target: &'static str,
    /// The pattern the wait matched against.
    pattern: PatternReport,
    /// Effective timeout in milliseconds.
    timeout_ms: u128,
}

impl WaitReport {
    /// Resolve `command`'s wait settings, or `None` if it is not a `Wait`.
    pub(super) fn for_command(command: &Command, settings: &Settings) -> Option<Self> {
        let Command::Wait {
            target,
            pattern,
            timeout,
        } = command
        else {
            return None;
        };
        let pattern = match pattern.as_ref().unwrap_or(&settings.wait_pattern) {
            WaitPattern::Contains(text) => PatternReport::Text(text.clone()),
            WaitPattern::Regex(regex) => PatternReport::Regex(regex.clone()),
        };
        Some(Self {
            target: wait_target_name(*target),
            pattern,
            timeout_ms: timeout.unwrap_or(settings.wait_timeout).as_millis(),
        })
    }

    fn describe(&self) -> String {
        let pattern = match &self.pattern {
            PatternReport::Text(text) => format!("text {text:?}"),
            PatternReport::Regex(regex) => format!("regex /{regex}/"),
        };
        let timeout = seconds(Duration::from_millis(self.timeout_ms as u64));
        format!("{} to match {pattern} within {timeout}", self.target)
    }
}

/// A wait pattern in `report.json`: `{"text": "Ready"}` or `{"regex": "^> $"}`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum PatternReport {
    Text(String),
    Regex(String),
}

/// A directory being filled with one failure's evidence.
pub(super) struct FailureBundle {
    directory: PathBuf,
    files: Files,
    terminal: Option<TerminalState>,
    /// Evidence that could not be written; the bundle keeps whatever else succeeded.
    errors: Vec<String>,
}

impl FailureBundle {
    /// Create a new, uniquely named bundle directory under `root` for the command at `index`.
    ///
    /// # Errors
    ///
    /// Returns a description of the directory that could not be created.
    pub(super) fn create(root: &Path, index: usize) -> Result<Self, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory = root.join(format!(
            "{}-{stamp}-command-{}",
            std::process::id(),
            index + 1
        ));
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("unable to create {}: {error}", directory.display()))?;
        Ok(Self {
            directory,
            files: Files::default(),
            terminal: None,
            errors: Vec::new(),
        })
    }

    /// Save the terminal as it is now: decorated screenshot, state JSON, and cells when the
    /// backend provides them.
    pub(super) fn add_terminal(
        &mut self,
        terminal: &mut impl TerminalSession,
        settings: &Settings,
        capture: &CaptureState,
    ) {
        match terminal.terminal_state() {
            Ok(state) => {
                self.record(
                    STATE_FILE,
                    write_json(&self.directory.join(STATE_FILE), &state),
                );
                self.terminal = Some(state);
            }
            Err(error) => self.errors.push(format!("{STATE_FILE}: {error}")),
        }
        let screenshot = capture_frame(terminal, settings, capture.frames.len())
            .and_then(|frame| {
                settings.presentation.decorate_frame_with_title(
                    &frame,
                    capture.caption.as_deref(),
                    &capture.keyboard_overlay.labels(),
                    capture.window_title.as_deref(),
                )
            })
            .and_then(|frame| write_png(&self.directory.join(SCREENSHOT_FILE), &frame));
        self.record(SCREENSHOT_FILE, screenshot);
        if let Ok(cells) = terminal.viewport_cells() {
            self.record(
                CELLS_FILE,
                write_json(&self.directory.join(CELLS_FILE), &cells),
            );
        }
    }

    /// Write the reports and return the bundle path, noting any evidence that could not be saved.
    pub(super) fn finish(mut self, failure: &Failure<'_>) -> String {
        let error = failure.error_text();
        match std::fs::write(self.directory.join(ERROR_FILE), &error) {
            Ok(()) => self.files.error = Some(ERROR_FILE),
            Err(error) => self.errors.push(format!("{ERROR_FILE}: {error}")),
        }
        self.files.report_text = Some(REPORT_TEXT_FILE);
        self.files.report_json = Some(REPORT_JSON_FILE);
        let report = self.report(failure, error);
        if let Err(error) = std::fs::write(
            self.directory.join(REPORT_TEXT_FILE),
            self.report_text(failure, &report),
        ) {
            self.errors.push(format!("{REPORT_TEXT_FILE}: {error}"));
        }
        if let Err(error) = write_json(&self.directory.join(REPORT_JSON_FILE), &report) {
            self.errors.push(error.to_string());
        }
        if self.errors.is_empty() {
            self.directory.display().to_string()
        } else {
            format!(
                "{} (not all evidence was saved: {})",
                self.directory.display(),
                self.errors.join("; ")
            )
        }
    }

    fn record(&mut self, file: &'static str, result: crate::Result<()>) {
        match result {
            Ok(()) => match file {
                SCREENSHOT_FILE => self.files.screenshot = Some(file),
                STATE_FILE => self.files.state = Some(file),
                CELLS_FILE => self.files.cells = Some(file),
                _ => {}
            },
            Err(error) => self.errors.push(format!("{file}: {error}")),
        }
    }

    fn report(&self, failure: &Failure<'_>, error: String) -> Report {
        let first = (failure.index + 1).saturating_sub(RECENT_COMMANDS);
        Report {
            schema: REPORT_SCHEMA,
            command: CommandReport::new(failure.tape, failure.index),
            error,
            elapsed_ms: failure.elapsed.map(|elapsed| elapsed.as_millis()),
            run_elapsed_ms: failure.run_elapsed.map(|elapsed| elapsed.as_millis()),
            wait: failure.wait.clone(),
            shell: ShellReport::from(&failure.shell),
            recent_commands: (first..=failure.index)
                .map(|index| CommandReport::new(failure.tape, index))
                .collect(),
            files: self.files.clone(),
            artifact_errors: self.errors.clone(),
        }
    }

    fn report_text(&self, failure: &Failure<'_>, report: &Report) -> String {
        let mut text = String::new();
        let command = &report.command;
        write!(
            text,
            "Tape command {} of {} failed at {}",
            command.number,
            failure.tape.commands.len(),
            failure.location()
        )
        .unwrap();
        if let Some(elapsed) = failure.elapsed {
            write!(text, " after {}", seconds(elapsed)).unwrap();
        }
        text.push_str(".\n\n");
        writeln!(text, "Command:  {}", command.text).unwrap();
        if let Some(wait) = &failure.wait {
            writeln!(text, "Waiting:  {}", wait.describe()).unwrap();
        }
        writeln!(text, "Shell:    {}", failure.shell).unwrap();
        if let Some(elapsed) = failure.run_elapsed {
            writeln!(
                text,
                "Run time: {} since the shell started",
                seconds(elapsed)
            )
            .unwrap();
        }
        text.push_str("\nError:\n");
        push_indented(&mut text, &report.error);

        text.push_str("\nRecent commands:\n");
        let width = report
            .recent_commands
            .iter()
            .map(|entry| entry.location.chars().count())
            .max()
            .unwrap_or(0);
        for entry in &report.recent_commands {
            let marker = if entry.number == command.number {
                ">"
            } else {
                " "
            };
            writeln!(text, "{marker} {:width$}  {}", entry.location, entry.text).unwrap();
        }

        if let Some(state) = &self.terminal {
            writeln!(
                text,
                "\nScreen ({}x{}, cursor at column {}, row {}):",
                state.size[0], state.size[1], state.cursor.x, state.cursor.y
            )
            .unwrap();
            push_indented(&mut text, state.viewport_text.trim_end());
            let scrollback: Vec<_> = state.scrollback_text.lines().collect();
            if !scrollback.is_empty() {
                let shown = scrollback.len().min(SCROLLBACK_EXCERPT_LINES);
                writeln!(
                    text,
                    "\nScrollback (last {shown} of {} lines):",
                    scrollback.len()
                )
                .unwrap();
                push_indented(
                    &mut text,
                    &scrollback[scrollback.len() - shown..].join("\n"),
                );
            }
        }

        text.push_str("\nFiles:\n");
        for (file, description) in [
            (report.files.screenshot, "screenshot at the failure"),
            (report.files.state, "state JSON at the failure"),
            (report.files.cells, "visible cells with styles"),
            (report.files.error, "the error message"),
            (report.files.report_json, "this report as JSON"),
        ] {
            if let Some(file) = file {
                writeln!(text, "  {file:13}{description}").unwrap();
            }
        }
        if !report.artifact_errors.is_empty() {
            text.push_str("\nNot saved:\n");
            for error in &report.artifact_errors {
                writeln!(text, "  {error}").unwrap();
            }
        }
        text
    }
}

/// `report.json` contents.
#[derive(Debug, Serialize)]
struct Report {
    schema: u32,
    command: CommandReport,
    error: String,
    elapsed_ms: Option<u128>,
    run_elapsed_ms: Option<u128>,
    wait: Option<WaitReport>,
    shell: ShellReport,
    recent_commands: Vec<CommandReport>,
    files: Files,
    artifact_errors: Vec<String>,
}

/// One tape command as the report names it.
#[derive(Debug, Serialize)]
struct CommandReport {
    /// One-based position in the expanded tape.
    number: usize,
    /// Where the command came from, such as `line 6`.
    location: String,
    /// The command as tape text.
    text: String,
}

impl CommandReport {
    fn new(tape: &Tape, index: usize) -> Self {
        Self {
            number: index + 1,
            location: location(tape, index),
            text: describe_command(&tape.commands[index]),
        }
    }
}

/// Shell state in `report.json`: `{"status": "exited", "code": 3}`.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ShellReport {
    NotStarted,
    Running,
    Exited { code: u32 },
    Signaled { signal: String },
}

impl From<&ShellStatus> for ShellReport {
    fn from(status: &ShellStatus) -> Self {
        match status {
            ShellStatus::NotStarted => Self::NotStarted,
            ShellStatus::Running => Self::Running,
            ShellStatus::Exited(code) => Self::Exited { code: *code },
            ShellStatus::Signaled(signal) => Self::Signaled {
                signal: signal.clone(),
            },
        }
    }
}

/// Bundle files that were written, by role.
#[derive(Debug, Clone, Default, Serialize)]
struct Files {
    #[serde(skip_serializing_if = "Option::is_none")]
    screenshot: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cells: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    report_text: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    report_json: Option<&'static str>,
}

fn location(tape: &Tape, index: usize) -> String {
    tape.location(index)
        .map_or_else(|| format!("command {}", index + 1), str::to_string)
}

fn seconds(duration: Duration) -> String {
    format!("{:.2}s", duration.as_secs_f64())
}

fn push_indented(text: &mut String, block: &str) {
    for line in block.lines() {
        if line.is_empty() {
            text.push('\n');
        } else {
            writeln!(text, "  {line}").unwrap();
        }
    }
}
