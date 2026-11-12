//! PTY process management and output draining.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use miette::{miette, Context, IntoDiagnostic};
use nix::sys::termios::LocalFlags;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use super::capture::{
    append_visible_frame, capture_frame, collect_title_changes, extend_unchanged_frame,
    CaptureState,
};
use super::settings::Settings;
use crate::runner::TerminalSession;
use crate::shell::{apply_terminal_environment, ensure_shell_exists, ShellLaunch};
use crate::tape::{WaitPattern, WaitTarget};
use crate::trace::ByteSample;
use crate::wait::{wait_pattern_name, wait_target_name, wait_target_text, WaitMatcher};
use crate::Result;

/// PTY reader buffer size.
///
/// Eight kilobytes is large enough for typical terminal bursts while keeping each channel message
/// cheap to allocate and copy.
const PTY_READ_BUFFER_SIZE: usize = 8 * 1024;
/// Longest a synchronized update holds back captures.
///
/// Ghostty ends a synchronized update after one second even when the program never sends the
/// end sequence, so a program that exits or stalls mid-update cannot freeze the screen.
const MAX_SYNC_HOLD: Duration = Duration::from_secs(1);

/// How often startup checks whether the shell has left canonical mode, which produces no output.
const STARTUP_POLL: Duration = Duration::from_millis(1);

/// Most PTY reads an input command applies before acting.
///
/// Reads are at most [`PTY_READ_BUFFER_SIZE`] bytes, so this covers several megabytes of backlog
/// while still returning under output that never stops.
const MAX_QUEUED_CHUNKS: usize = 1024;

/// How [`PtySession::poll_frames`] ended.
enum PollOutcome {
    Matched,
    /// The shell closed the terminal; carries its status for the error message.
    Closed(String),
    TimedOut,
}

/// PTY process plus non-blocking output reader.
///
/// `portable-pty` exposes blocking readers. Betamax moves reads onto a background thread and uses a
/// channel so the runner can express "drain until idle", "drain for this duration", and "wait while
/// sampling frames" without blocking permanently on a shell that is waiting for input.
pub(super) struct PtySession {
    /// Control handle retained for runtime resize, released with this session.
    master: Box<dyn portable_pty::MasterPty + Send>,
    /// Writable PTY master side.
    writer: Box<dyn Write + Send>,
    /// Reader-thread channel carrying raw bytes from the PTY.
    reader: Receiver<Vec<u8>>,
    /// Child process handle kept alive for the session lifetime.
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Whether the reader thread has finished, so no more output can arrive.
    output_closed: bool,
    /// When captures first found the program inside its current synchronized update.
    sync_hold_started: Option<Instant>,
}

/// Whether the shell is still running, reported in failure bundles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ShellStatus {
    /// The tape failed before the shell was spawned.
    NotStarted,
    /// The shell is still running.
    Running,
    /// The shell exited with a status code.
    Exited(u32),
    /// The shell was terminated by a signal.
    Signaled(String),
}

impl std::fmt::Display for ShellStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted => formatter.write_str("not started"),
            Self::Running => formatter.write_str("running"),
            Self::Exited(code) => write!(formatter, "exited with status {code}"),
            Self::Signaled(signal) => write!(formatter, "terminated by {signal}"),
        }
    }
}

impl PtySession {
    /// Spawn the configured shell in a PTY sized to the derived terminal grid.
    ///
    /// The PTY reader runs on a background thread so command execution can drain output with idle
    /// timeouts instead of blocking forever on reads.
    pub(super) fn spawn(settings: &Settings) -> Result<Self> {
        let shell = ShellLaunch::from_argv(&settings.shell);
        ensure_shell_exists(&shell.argv[0], &settings.env)?;
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: settings.rows,
                cols: settings.columns,
                pixel_width: settings.presentation.terminal_canvas_width() as u16,
                pixel_height: settings.presentation.terminal_canvas_height() as u16,
            })
            .map_err(|error| miette!("failed to open PTY: {error}"))?;

        let mut command = CommandBuilder::from_argv(shell.argv);
        // portable-pty falls back to $HOME without an explicit cwd.
        let cwd = std::env::current_dir()
            .into_diagnostic()
            .wrap_err("failed to read the current directory")?;
        command.cwd(cwd);
        apply_terminal_environment(&mut command, shell.kind, &settings.env);

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| miette!("failed to spawn shell in PTY: {error}"))
            .wrap_err("failed to spawn shell in PTY")?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| miette!("failed to open PTY writer: {error}"))?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| miette!("failed to open PTY reader: {error}"))?;
        let (output_tx, output_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = [0u8; PTY_READ_BUFFER_SIZE];
            while let Ok(len) = reader.read(&mut buf) {
                if len == 0 {
                    break;
                }
                if output_tx.send(buf[..len].to_vec()).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer,
            reader: output_rx,
            child,
            output_closed: false,
            sync_hold_started: None,
        })
    }

    /// Report whether the shell is still running without waiting for it.
    pub(super) fn shell_status(&mut self) -> ShellStatus {
        match self.child.try_wait() {
            Ok(Some(status)) => match status.signal() {
                Some(signal) => ShellStatus::Signaled(signal.to_string()),
                None => ShellStatus::Exited(status.exit_code()),
            },
            Ok(None) | Err(_) => ShellStatus::Running,
        }
    }

    /// Describe the shell after its output closed: its exit status, once the process is reaped.
    fn closed_shell_status(&mut self) -> ShellStatus {
        // The PTY closes as the shell exits; give the kernel a moment to make the status visible.
        let deadline = Instant::now() + Duration::from_millis(200);
        loop {
            let status = self.shell_status();
            if status != ShellStatus::Running || Instant::now() >= deadline {
                return status;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Change kernel winsize and notify the child through the normal PTY resize mechanism.
    pub(super) fn resize(&self, columns: u16, rows: u16, settings: &Settings) -> Result<()> {
        self.master
            .resize(PtySize {
                cols: columns,
                rows,
                // Zero means unspecified when the optional pixel dimensions exceed PTY limits.
                pixel_width: u16::try_from(
                    u32::from(columns) * settings.presentation.text.cell_width(),
                )
                .unwrap_or(0),
                pixel_height: u16::try_from(
                    u32::from(rows) * settings.presentation.text.cell_height(),
                )
                .unwrap_or(0),
            })
            .map_err(|error| {
                miette!("failed to resize PTY to {columns} columns by {rows} rows: {error}")
            })?;
        Ok(())
    }

    /// Type text without capture support.
    ///
    /// Capture runs implement typing inline because they need to drain and sample frames after each
    /// character.
    pub(super) fn type_text(&mut self, text: &str, delay: Option<Duration>) -> Result<()> {
        let delay = delay.unwrap_or(Duration::ZERO);
        for ch in text.chars() {
            let mut buf = [0u8; 4];
            self.write_all(ch.encode_utf8(&mut buf).as_bytes())?;
            if !delay.is_zero() {
                thread::sleep(delay);
            }
        }
        Ok(())
    }

    /// Write raw input bytes to the PTY.
    pub(super) fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() == 1 {
            tracing::trace!(
                bytes = bytes.len(),
                sample = %ByteSample(bytes),
                "writing bytes to PTY",
            );
        } else {
            tracing::trace!(
                bytes = bytes.len(),
                sample = %ByteSample(bytes),
                "writing byte sample to PTY",
            );
            tracing::debug!(bytes = bytes.len(), "writing bytes to PTY",);
        }
        Ok(self.writer.write_all(bytes).into_diagnostic()?)
    }

    /// Apply shell startup output until the shell is ready for typing, for at most `limit`.
    ///
    /// A new PTY starts in canonical mode, where the kernel echoes input and handles keys such as
    /// `Ctrl+C` itself. Typing then would echo ahead of the prompt, or signal a shell that has not
    /// set up its handlers yet. Line editors leave canonical mode once they are ready, after
    /// drawing the prompt; zsh draws it several milliseconds before, so output alone is not a
    /// signal. Startup output is not recorded, so nothing waits for it to stop: output after the
    /// shell is ready is recorded as it arrives.
    pub(super) fn drain_startup(
        &mut self,
        terminal: &mut impl TerminalSession,
        limit: Duration,
    ) -> Result<()> {
        let deadline = Instant::now() + limit;
        while !self.line_editor_ready() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            self.read_once_into(terminal, remaining.min(STARTUP_POLL))?;
        }
        self.apply_queued(terminal)
    }

    /// Whether the program on the PTY has left canonical mode, as a line editor does when it is
    /// ready to draw what is typed.
    fn line_editor_ready(&self) -> bool {
        self.master
            .get_termios()
            .is_some_and(|termios| !termios.local_flags.contains(LocalFlags::ICANON))
    }

    /// Check absence after each received chunk and at the end of a bounded observation period.
    pub(super) fn observe_absence(
        &mut self,
        terminal: &mut impl TerminalSession,
        text: &str,
        duration: Duration,
    ) -> Result<()> {
        let started = Instant::now();
        crate::assertion::check_absent(terminal, text)?;
        while started.elapsed() < duration {
            let wait = duration
                .saturating_sub(started.elapsed())
                .min(Duration::from_millis(20));
            self.read_once_into(terminal, wait)?;
            crate::assertion::check_absent(terminal, text)?;
        }
        Ok(())
    }

    fn read_once_into(
        &mut self,
        terminal: &mut impl TerminalSession,
        wait: Duration,
    ) -> Result<bool> {
        match self.reader.recv_timeout(wait) {
            Ok(bytes) => {
                self.feed(terminal, &bytes)?;
                Ok(true)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(false),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // A completed child has stable final output. Avoid spinning on a closed channel.
                self.output_closed = true;
                thread::sleep(wait);
                Ok(false)
            }
        }
    }

    /// Read one chunk, waiting up to `wait`, then the chunks already queued behind it.
    ///
    /// Queued chunks are drained for at most `budget`, so output arriving faster than it parses is
    /// still sampled. Callers check the screen once per burst rather than once per read; a program
    /// sending megabytes of image data would otherwise leave the recording seconds behind.
    fn read_burst_into(
        &mut self,
        terminal: &mut impl TerminalSession,
        wait: Duration,
        budget: Duration,
    ) -> Result<bool> {
        if !self.read_once_into(terminal, wait)? {
            return Ok(false);
        }
        let started = Instant::now();
        while started.elapsed() < budget {
            let Ok(bytes) = self.reader.try_recv() else {
                break;
            };
            self.feed(terminal, &bytes)?;
        }
        Ok(true)
    }

    /// Whether a due capture may render the screen now, rather than repeat the last frame.
    ///
    /// While a program holds a synchronized update (mode 2026), a terminal keeps showing the
    /// frame it drew before the update began, so the capture does the same. Like Ghostty, the
    /// hold ends after [`MAX_SYNC_HOLD`] even if the update never finishes. Programs without
    /// synchronized output get no hold: a frame caught mid-write is what a terminal shows too.
    ///
    /// The mode is checked between reads, so an update that ends and is followed by the start of
    /// the next within one read is never captured. libghostty 1.0's `on_render_hold` reports the
    /// exact byte where the mode changes; see the roadmap.
    fn may_render(&mut self, terminal: &impl TerminalSession) -> bool {
        if !terminal.synchronized_update_active() {
            self.sync_hold_started = None;
            return true;
        }
        self.sync_hold_started
            .get_or_insert_with(Instant::now)
            .elapsed()
            >= MAX_SYNC_HOLD
    }

    /// Write PTY bytes into the terminal and forward any reply it produces.
    fn feed(&mut self, terminal: &mut impl TerminalSession, bytes: &[u8]) -> Result<()> {
        terminal.write_vt(bytes);
        let reply = terminal.take_pending_pty_reply();
        if !reply.is_empty() {
            self.write_all(&reply)?;
        }
        Ok(())
    }

    /// Apply output that has already arrived, without waiting for more.
    ///
    /// Input commands (`Mouse`, `Resize`, scrolling) and title boundaries (`WindowTitle`, `Show`)
    /// act on the terminal as it is now. Waiting for the PTY to go quiet would never return while
    /// an app redraws continuously; tapes that need a particular screen use `Wait`.
    ///
    /// At most [`MAX_QUEUED_CHUNKS`] are applied, so output arriving faster than it parses cannot
    /// keep the queue from emptying; the rest is left for the next command.
    pub(super) fn apply_queued(&mut self, terminal: &mut impl TerminalSession) -> Result<()> {
        for _ in 0..MAX_QUEUED_CHUNKS {
            let Ok(bytes) = self.reader.try_recv() else {
                break;
            };
            log_pty_drain("applying queued PTY bytes", &bytes);
            self.feed(terminal, &bytes)?;
        }
        Ok(())
    }

    /// Apply output for one frame interval, then return whatever the terminal shows.
    ///
    /// This is the hold a renderer at the capture rate gives: output that lands within the frame
    /// is shown, and output still arriving is not waited for. A frame caught mid-update (a torn
    /// write or a half-sent image from an app without synchronized output) is captured as it is,
    /// because a real terminal would show it too.
    ///
    /// A frame that ends inside a synchronized update keeps reading until the update finishes,
    /// for at most [`MAX_SYNC_HOLD`]. A terminal would still show the previous frame, but only
    /// the half-written screen exists here, and the finished update is what the terminal shows
    /// next.
    pub(super) fn read_for_frame(
        &mut self,
        terminal: &mut impl TerminalSession,
        interval: Duration,
    ) -> Result<()> {
        let deadline = Instant::now() + interval;
        while !self.output_closed {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                if self.may_render(terminal) {
                    break;
                }
                self.read_burst_into(terminal, interval, interval)?;
            } else {
                self.read_burst_into(terminal, remaining, remaining)?;
            }
        }
        Ok(())
    }

    /// Discard shell startup output until the shell is ready for typing, for at most `limit`.
    ///
    /// This is used only for tapes that do not need screenshots, waits, or state output, so no
    /// terminal model is available to interpret the bytes. See [`Self::drain_startup`].
    pub(super) fn drain_startup_output(&mut self, limit: Duration) {
        let deadline = Instant::now() + limit;
        while !self.line_editor_ready() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            if self
                .reader
                .recv_timeout(remaining.min(STARTUP_POLL))
                .is_ok()
            {
                for _ in self.reader.try_iter().take(MAX_QUEUED_CHUNKS) {}
            }
        }
        for _ in self.reader.try_iter().take(MAX_QUEUED_CHUNKS) {}
    }

    /// Drain PTY output for a fixed duration and append frames at the capture cadence.
    ///
    /// Output still arriving at the end is left for the next command, whose own timing decides
    /// when it is shown; a command boundary is not a reason to wait.
    pub(super) fn drain_for(
        &mut self,
        terminal: &mut impl TerminalSession,
        duration: Duration,
        settings: &Settings,
        capture: &mut CaptureState,
    ) -> Result<()> {
        self.drain_until(terminal, Instant::now() + duration, settings, capture)?;
        collect_title_changes(terminal, settings, capture)?;
        Ok(())
    }

    /// Drain PTY output until `deadline` and append frames at the capture cadence.
    ///
    /// Unlike [`Self::drain_for`], this returns at the deadline without a trailing idle read, so
    /// callers pacing input (such as `Type`) keep their schedule. Captures with no PTY output since
    /// the previous one extend its hold instead of rendering an identical frame; rendering takes
    /// long enough at large sizes that redundant renders would push input past its deadline.
    pub(super) fn drain_until(
        &mut self,
        terminal: &mut impl TerminalSession,
        deadline: Instant,
        settings: &Settings,
        capture: &mut CaptureState,
    ) -> Result<()> {
        let started = Instant::now();
        let capture_interval = settings.capture_interval();
        let mut last_capture_at = *capture.recorded_until.get_or_insert(started);
        // The terminal may have changed since the last capture (input, resize, scrolling), so
        // the first capture in each drain always renders.
        let mut changed = true;
        // A due capture held by a synchronized update renders after the first read that ends it,
        // as a terminal draws as soon as an update finishes.
        let mut held = false;
        while Instant::now() < deadline {
            let now = Instant::now();
            // Hidden periods never capture, so waiting for a capture slot would busy-spin until
            // the deadline and starve the child process of CPU. A held capture waits for output
            // instead of its slot for the same reason.
            let next_due = if !capture.visible {
                deadline
            } else if held {
                now + capture_interval
            } else {
                (last_capture_at + capture_interval).max(now)
            };
            let wait = deadline.min(next_due).saturating_duration_since(now);
            let read = self.read_burst_into(terminal, wait, capture_interval)?;
            changed |= read;
            let due = capture.visible
                && (last_capture_at.elapsed() >= capture_interval || Instant::now() >= deadline);
            collect_title_changes(terminal, settings, capture)?;
            if due {
                held = changed && !self.may_render(terminal);
                // A held capture extends the frame on screen but stays due.
                let captured_at = record_capture(terminal, settings, capture, changed && !held)?;
                if !held {
                    last_capture_at = captured_at;
                    changed = false;
                }
            }
        }
        // A drain that starts after its deadline would leave time before the deadline unrecorded.
        // Nothing records it if the tape ends here, so capture now.
        if capture.visible && last_capture_at < deadline {
            collect_title_changes(terminal, settings, capture)?;
            let render = changed && self.may_render(terminal);
            record_capture(terminal, settings, capture, render)?;
        }
        Ok(())
    }

    /// Wait for terminal text to match a pattern while continuing to capture frames.
    ///
    /// The timeout is checked against wall-clock time. If the pattern never matches, the error
    /// names the target and pattern so failing tapes are diagnosable without inspecting code. If
    /// the shell exits first, the wait fails at once with its exit status instead of running out
    /// the timeout.
    pub(super) fn wait_for(
        &mut self,
        terminal: &mut impl TerminalSession,
        target: WaitTarget,
        pattern: &WaitPattern,
        timeout: Duration,
        settings: &Settings,
        capture: &mut CaptureState,
    ) -> Result<()> {
        let matcher = WaitMatcher::new(pattern)?;
        let outcome = self.poll_frames(terminal, timeout, settings, capture, |terminal| {
            Ok(matcher.is_match(&wait_target_text(terminal, target)?))
        })?;
        match outcome {
            PollOutcome::Matched => Ok(()),
            PollOutcome::Closed(status) => Err(miette!(
                "shell {status} before {} matched {}",
                wait_target_name(target),
                wait_pattern_name(pattern)
            )
            .into()),
            PollOutcome::TimedOut => Err(miette!(
                "timed out waiting for {} to match {}",
                wait_target_name(target),
                wait_pattern_name(pattern)
            )
            .into()),
        }
    }

    /// Pass as soon as an assertion holds on any frame within `timeout`, capturing frames
    /// meanwhile.
    ///
    /// Positive assertions behave like a wait: animation cannot fail them, and a tape does not need
    /// a separate wait before each one. On timeout, the error is the assertion's failure on the
    /// last frame checked.
    pub(super) fn assert_within(
        &mut self,
        terminal: &mut impl TerminalSession,
        assertion: &crate::tape::Assertion,
        timeout: Duration,
        settings: &Settings,
        capture: &mut CaptureState,
    ) -> Result<()> {
        let prepared = crate::assertion::Prepared::new(assertion)?;
        let mut last_failure = None;
        let outcome =
            self.poll_frames(
                terminal,
                timeout,
                settings,
                capture,
                |terminal| match prepared.check(terminal) {
                    Ok(()) => Ok(true),
                    Err(failure) => {
                        last_failure = Some(failure);
                        Ok(false)
                    }
                },
            )?;
        match (outcome, last_failure) {
            (PollOutcome::Matched, _) => Ok(()),
            (_, Some(failure)) => Err(failure),
            (_, None) => Err(miette!("assertion was not checked within {timeout:?}").into()),
        }
    }

    /// Check `matches` against the screen after every burst of output until it holds, the shell
    /// closes the terminal, or `timeout` passes, recording frames at the capture rate throughout.
    ///
    /// The screen is checked once before any read, so a condition that already holds passes at
    /// once.
    fn poll_frames<T: TerminalSession>(
        &mut self,
        terminal: &mut T,
        timeout: Duration,
        settings: &Settings,
        capture: &mut CaptureState,
        mut matches: impl FnMut(&mut T) -> Result<bool>,
    ) -> Result<PollOutcome> {
        let started = Instant::now();
        let capture_interval = settings.capture_interval();
        let mut last_capture_at = *capture.recorded_until.get_or_insert(started);
        if matches(terminal)? {
            return Ok(PollOutcome::Matched);
        }
        while started.elapsed() < timeout {
            let wait = capture_interval.min(timeout.saturating_sub(started.elapsed()));
            self.read_burst_into(terminal, wait, capture_interval)?;
            let due = capture.visible && last_capture_at.elapsed() >= capture_interval;
            collect_title_changes(terminal, settings, capture)?;
            let matched = matches(terminal)?;
            // A capture held by a synchronized update extends the frame on screen but stays due,
            // so it renders after the first read that ends the update.
            if capture.visible && (due || matched) {
                let render = self.may_render(terminal);
                let captured_at = record_capture(terminal, settings, capture, render)?;
                if render {
                    last_capture_at = captured_at;
                }
            }
            if matched {
                return Ok(PollOutcome::Matched);
            }
            if self.output_closed {
                // No more output can arrive, so the condition can never start to hold.
                let status = match self.closed_shell_status() {
                    ShellStatus::Running => "closed the terminal".to_string(),
                    status => status.to_string(),
                };
                return Ok(PollOutcome::Closed(status));
            }
        }
        Ok(PollOutcome::TimedOut)
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        // Failed assertions may leave a child waiting for input or still producing output.
        // Reap it before releasing the control handle so test failures do not leak processes.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn log_pty_drain(message: &'static str, bytes: &[u8]) {
    tracing::trace!(
        bytes = bytes.len(),
        sample = %ByteSample(bytes),
        message,
    );
}

/// Record the time since the last capture as a frame hold, rendering only if output arrived.
///
/// Returns the capture instant, which becomes the start of the next hold.
fn record_capture(
    terminal: &mut impl TerminalSession,
    settings: &Settings,
    capture: &mut CaptureState,
    changed: bool,
) -> Result<Instant> {
    let captured_at = Instant::now();
    let last_capture_at = capture.recorded_until.unwrap_or(captured_at);
    let frame_delay =
        settings.playback_delay(captured_at.saturating_duration_since(last_capture_at));
    if changed || !extend_unchanged_frame(capture, frame_delay) {
        append_visible_frame(
            capture,
            capture_frame(terminal, settings, capture.frames.len())?,
            frame_delay,
        );
    }
    capture.recorded_until = Some(captured_at);
    Ok(captured_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghostty::{
        CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, TerminalGrid,
        TerminalTheme, TextSettings,
    };
    use crate::tape::Tape;

    #[test]
    fn checkpoint_consumes_queued_output_before_accepting_a_matching_screen() {
        let (mut pty, mut terminal) = queued_output(b"TRANSIENT", b"\x1b[2J\x1b[HFINAL");

        pty.read_for_frame(&mut terminal, Duration::from_millis(20))
            .unwrap();

        let screen = terminal.screen_text().unwrap();
        assert!(screen.contains("FINAL"));
        assert!(!screen.contains("TRANSIENT"));
    }

    #[test]
    fn bounded_absence_checks_output_received_after_the_initial_screen() {
        let (mut pty, mut terminal) = queued_output(b"armed", b"\r\nFORBIDDEN");
        crate::assertion::check_absent(&mut terminal, "FORBIDDEN").unwrap();

        let error = pty
            .observe_absence(&mut terminal, "FORBIDDEN", Duration::from_secs(2))
            .unwrap_err();

        assert!(error.to_string().contains("to be absent"));
        assert!(terminal.screen_text().unwrap().contains("FORBIDDEN"));
    }

    /// A program without synchronized output replaces a placed image many times: it deletes the
    /// old image, sends the new one in kitty chunks across many PTY reads, then redraws the
    /// placeholder cells. The recording must keep up. Frames caught between the delete and the
    /// replacement show no image, as a terminal would.
    #[test]
    fn rapidly_replaced_images_do_not_stall_the_recording() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine as _;

        const UPDATES: u32 = 16;
        const COLUMNS: u32 = 10;
        const ROWS: u32 = 4;
        // Diacritics for placeholder rows 0..ROWS from the kitty table.
        const ROW_DIACRITICS: [char; ROWS as usize] =
            ['\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}'];
        let colors = [[255u8, 0, 0, 255], [0, 0, 255, 255]];

        let tape = Tape::parse(r#"Set Shell "/usr/bin/true""#).unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let mut pty = PtySession::spawn(&settings).unwrap();
        let (sender, reader) = mpsc::channel::<Vec<u8>>();
        pty.reader = reader;
        let text = TextSettings::default();
        let (cell_width, cell_height) = (text.cell_width(), text.cell_height());
        let request = CaptureRequest {
            canvas: PixelSize::new(
                40 * cell_width + 2 * text.padding,
                15 * cell_height + 2 * text.padding,
            ),
            grid: TerminalGrid::new(40, 15),
            text: text.clone(),
            theme: TerminalTheme::default(),
        };
        let mut terminal = GhosttyFrameCapture.open(request).unwrap();

        // An image twice the grid's pixel size, so each update is ~500 KB of base64.
        let (width, height) = (2 * COLUMNS * cell_width, 2 * ROWS * cell_height);
        // Queue every update before the wait starts, so the result does not depend on how fast
        // this thread runs. Each update arrives as a delete plus the first chunk, the remaining
        // chunks in 8 KiB reads, then the last chunk plus the placeholder cells, as a program
        // writing one update would produce.
        for update in 0..UPDATES {
            let id = update + 1;
            let pixels = colors[update as usize % 2].repeat((width * height) as usize);
            let payload = STANDARD.encode(pixels);
            let chunks: Vec<_> = payload.as_bytes().chunks(4096).collect();
            let commands: Vec<Vec<u8>> = chunks
                .iter()
                .enumerate()
                .map(|(index, chunk)| {
                    let more = u8::from(index + 1 < chunks.len());
                    let control = if index == 0 {
                        format!(
                            "a=T,U=1,i={id},f=32,s={width},v={height},c={COLUMNS},r={ROWS},q=2,m={more}"
                        )
                    } else {
                        format!("m={more}")
                    };
                    let mut command = format!("\x1b_G{control};").into_bytes();
                    command.extend_from_slice(chunk);
                    command.extend_from_slice(b"\x1b\\");
                    command
                })
                .collect();
            let (first, rest) = commands.split_first().unwrap();
            let (last, middle) = rest.split_last().unwrap();

            let mut head = Vec::new();
            if update > 0 {
                head.extend(format!("\x1b_Ga=d,d=I,i={},q=2\x1b\\", id - 1).bytes());
            }
            head.extend_from_slice(first);
            sender.send(head).unwrap();
            for read in middle.concat().chunks(PTY_READ_BUFFER_SIZE) {
                sender.send(read.to_vec()).unwrap();
            }
            let mut tail = last.clone();
            for (row, diacritic) in ROW_DIACRITICS.iter().enumerate() {
                let mut line = format!("\x1b[{};1H\x1b[38;2;0;0;{id}m", row + 1);
                for _ in 0..COLUMNS {
                    line.push('\u{10EEEE}');
                    line.push(*diacritic);
                }
                tail.extend(line.bytes());
            }
            tail.extend_from_slice(b"\x1b[39m\x1b[6;1H");
            sender.send(tail).unwrap();
        }
        sender.send(b"DONE".to_vec()).unwrap();

        let mut capture = CaptureState::default();
        let started = Instant::now();
        pty.wait_for(
            &mut terminal,
            WaitTarget::Screen,
            &WaitPattern::Regex("DONE".to_string()),
            Duration::from_secs(20),
            &settings,
            &mut capture,
        )
        .unwrap();
        let elapsed = started.elapsed();
        drop(sender);

        // Checking the screen after every read instead of every burst takes far longer.
        assert!(
            elapsed < Duration::from_secs(5),
            "wait took {elapsed:?} for {UPDATES} image updates"
        );
        let centre = |frame: &crate::media::Frame| {
            let x = text.padding + COLUMNS * cell_width / 2;
            let y = text.padding + ROWS * cell_height / 2;
            let offset = y as usize * frame.stride + x as usize * 4;
            <[u8; 4]>::try_from(&frame.pixels[offset..offset + 4]).unwrap()
        };
        let shown: Vec<_> = capture
            .frames
            .iter()
            .map(|(captured, _)| centre(&captured.frame))
            .skip_while(|pixel| !colors.contains(pixel))
            .collect();
        // How many frames land, and which catch an image mid-replacement, depends on runner
        // speed.
        assert!(!shown.is_empty(), "no frame showed the image");
    }

    #[test]
    fn queued_output_beyond_the_limit_is_left_for_later_without_failing() {
        let (mut pty, mut terminal) = queued_output(b"", b"");
        let (sender, reader) = mpsc::channel();
        pty.reader = reader;
        for chunk in 0..MAX_QUEUED_CHUNKS + 10 {
            sender.send(format!("\r{chunk}").into_bytes()).unwrap();
        }

        pty.apply_queued(&mut terminal).unwrap();

        assert_eq!(pty.reader.try_iter().count(), 10);
    }

    /// Spawn `shell` after the terminal is created, which takes long enough in debug builds to
    /// skew a test's timing.
    ///
    /// Returns when the spawn began, so elapsed times include anything the child did before the
    /// test thread resumed.
    fn spawn_shell(shell: &str) -> (PtySession, GhosttySession, Instant) {
        let (_, terminal) = queued_output(b"", b"");
        let settings =
            Settings::from_tape(&Tape::parse(&format!("Set Shell {shell:?}")).unwrap()).unwrap();
        let started = Instant::now();
        let pty = PtySession::spawn(&settings).unwrap();
        (pty, terminal, started)
    }

    #[test]
    fn startup_ends_when_the_shell_leaves_canonical_mode() {
        let (mut pty, mut terminal, started) =
            spawn_shell("sh -c 'sleep 0.3; printf ready; stty -icanon; sleep 5'");

        pty.drain_startup(&mut terminal, Duration::from_secs(3))
            .unwrap();

        let elapsed = started.elapsed();
        assert!(
            elapsed >= Duration::from_millis(300),
            "startup took {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(2), "startup took {elapsed:?}");
        assert!(terminal.screen_text().unwrap().contains("ready"));
    }

    #[test]
    fn startup_that_stays_in_canonical_mode_ends_at_the_limit() {
        let (mut pty, mut terminal, started) =
            spawn_shell("sh -c 'while :; do printf spinning; sleep 0.005; done'");

        pty.drain_startup(&mut terminal, Duration::from_millis(300))
            .unwrap();

        let elapsed = started.elapsed();
        assert!(
            elapsed >= Duration::from_millis(300),
            "startup took {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(2), "startup took {elapsed:?}");
        assert!(terminal.screen_text().unwrap().contains("spinning"));
    }

    /// The first cell's colour in each recorded frame, with how long the frame shows.
    fn first_cell_colors(capture: &CaptureState) -> Vec<([u8; 4], Duration)> {
        let text = TextSettings::default();
        let x = text.padding + text.cell_width() / 2;
        let y = text.padding + text.cell_height() / 2;
        capture
            .frames
            .iter()
            .map(|(captured, delay)| {
                let frame = &captured.frame;
                let offset = y as usize * frame.stride + x as usize * 4;
                let pixel = <[u8; 4]>::try_from(&frame.pixels[offset..offset + 4]).unwrap();
                (pixel, *delay)
            })
            .collect()
    }

    /// Show `initial`, then send each output at its offset from a signal on the returned sender,
    /// from another thread. Sending the signal just before reading keeps the offsets lined up
    /// with the capture schedule on a slow machine.
    fn paced_output(
        initial: &[u8],
        outputs: Vec<(Duration, &'static [u8])>,
    ) -> (PtySession, GhosttySession, mpsc::Sender<()>) {
        let (mut pty, terminal) = queued_output(initial, b"");
        let (sender, reader) = mpsc::channel();
        pty.reader = reader;
        let (start, started_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = started_rx.recv();
            let started = Instant::now();
            for (at, bytes) in outputs {
                thread::sleep(at.saturating_sub(started.elapsed()));
                let _ = sender.send(bytes.to_vec());
            }
            // Keep the channel open so the session does not see the shell close.
            thread::sleep(Duration::from_secs(5));
        });
        (pty, terminal, start)
    }

    const GREEN_CELLS: &[u8] = b"\x1b[42m  \x1b[0m";
    const SYNC_RED_CELLS: &[u8] = b"\x1b[?2026h\x1b[H\x1b[41m  \x1b[0m";

    fn fast_capture_settings() -> Settings {
        Settings::from_tape(&Tape::parse("Set Framerate 50").unwrap()).unwrap()
    }

    /// Each update starts before a capture is due and ends after it, writing red cells before its
    /// final colour. No frame may show the red, and every update must be recorded: a held capture
    /// renders when its update ends, not at the next capture slot, which would land inside the
    /// next update too.
    #[test]
    fn synchronized_updates_record_the_previous_frame_until_they_finish() {
        // Built first: loading settings can outlast the paced output's first delay.
        let settings = Settings::from_tape(&Tape::parse("Set Framerate 1").unwrap()).unwrap();
        let finished: [&'static [u8]; 3] = [
            b"\x1b[H\x1b[43m  \x1b[0m\x1b[?2026l",
            b"\x1b[H\x1b[44m  \x1b[0m\x1b[?2026l",
            b"\x1b[H\x1b[45m  \x1b[0m\x1b[?2026l",
        ];
        // Captures fall due every second; update k runs from 1000k + 700 ms to 1000k + 1300 ms,
        // so each spans a capture slot with 300 ms to spare on either side, and its result shows
        // for 400 ms before the next update starts. Timers on CI runners can overshoot by over
        // 100 ms; an end and the next start arriving in one read would lose that update.
        let outputs = finished
            .into_iter()
            .zip(0u64..)
            .flat_map(|(end, k)| {
                [
                    (Duration::from_millis(1000 * k + 700), SYNC_RED_CELLS),
                    (Duration::from_millis(1000 * k + 1300), end),
                ]
            })
            .collect();
        let (mut pty, mut terminal, start) = paced_output(GREEN_CELLS, outputs);
        let mut capture = CaptureState::default();

        // The screen before the first update, as a recording would have after `Show`.
        record_capture(&mut terminal, &settings, &mut capture, true).unwrap();
        start.send(()).unwrap();
        pty.drain_for(
            &mut terminal,
            Duration::from_millis(3800),
            &settings,
            &mut capture,
        )
        .unwrap();

        let mut colors: Vec<_> = first_cell_colors(&capture)
            .into_iter()
            .map(|(color, _)| color)
            .collect();
        colors.dedup();
        assert_eq!(
            colors.len(),
            4,
            "expected the first frame and each of 3 updates once, got {colors:?}"
        );
    }

    #[test]
    fn unfinished_synchronized_updates_stop_holding_after_a_second() {
        // Built first: loading settings can outlast the paced output's first delay.
        let settings = fast_capture_settings();
        let (mut pty, mut terminal, start) = paced_output(
            GREEN_CELLS,
            vec![(Duration::from_millis(100), SYNC_RED_CELLS)],
        );
        let mut capture = CaptureState::default();

        record_capture(&mut terminal, &settings, &mut capture, true).unwrap();
        start.send(()).unwrap();
        pty.drain_for(
            &mut terminal,
            Duration::from_millis(1500),
            &settings,
            &mut capture,
        )
        .unwrap();

        let colors = first_cell_colors(&capture);
        let green = colors[0].0;
        assert_ne!(
            colors.last().unwrap().0,
            green,
            "the update was held forever"
        );
        let shown: Duration = colors
            .iter()
            .take_while(|(color, _)| *color == green)
            .map(|(_, delay)| *delay)
            .sum();
        assert!(shown >= MAX_SYNC_HOLD, "the hold ended after {shown:?}");
    }

    #[test]
    fn checkpoints_finish_a_synchronized_update_in_progress() {
        let (mut pty, mut terminal, start) = paced_output(
            b"\x1b[?2026hHALF",
            vec![(Duration::from_millis(100), b"\x1b[HFULL\x1b[?2026l")],
        );

        start.send(()).unwrap();
        pty.read_for_frame(&mut terminal, Duration::from_millis(20))
            .unwrap();

        assert!(terminal.screen_text().unwrap().contains("FULL"));
    }

    fn queued_output(initial: &[u8], output: &[u8]) -> (PtySession, GhosttySession) {
        let tape = Tape::parse(r#"Set Shell "/usr/bin/true""#).unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let mut pty = PtySession::spawn(&settings).unwrap();
        let (sender, reader) = mpsc::channel();
        pty.reader = reader;
        let request = CaptureRequest {
            canvas: PixelSize::new(400, 240),
            grid: TerminalGrid::new(40, 15),
            text: TextSettings::default(),
            theme: TerminalTheme::default(),
        };
        let mut terminal = GhosttyFrameCapture.open(request).unwrap();
        terminal.write_vt(initial);
        sender.send(output.to_vec()).unwrap();
        (pty, terminal)
    }
}
