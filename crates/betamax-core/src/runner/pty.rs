//! PTY process management and output draining.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use miette::{miette, Context, IntoDiagnostic};
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
/// Tail idle drain after a fixed-duration wait finishes.
///
/// This catches output emitted near the end of a `Sleep` without extending the visible sleep long
/// enough to be noticeable.
const POST_DURATION_IDLE: Duration = Duration::from_millis(20);
/// Output pause that lets a due capture proceed while a program is still writing.
///
/// kitty waits this long for more input before repainting (its default `input_delay`), so an
/// update written in several reads, such as an image delete followed by its replacement, lands in
/// one frame.
const CAPTURE_QUIET: Duration = Duration::from_millis(3);
/// Longest a capture is skipped because an image transmission is still open.
///
/// A slow runner can take longer than a capture interval to parse a large chunked image; skipping
/// those captures keeps the image area from recording blank. The cap keeps a program that never
/// finishes a transmission from freezing the recording.
const MAX_TRANSMISSION_DEFER: Duration = Duration::from_secs(1);

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

    /// Drain PTY output into the terminal until no bytes arrive for `idle`.
    ///
    /// Returns whether any bytes were observed. After the first blocking receive succeeds, all
    /// immediately available chunks are drained without additional waits so bursty output stays
    /// grouped into the same terminal update. Replies from the emulator (e.g. CPR responses)
    /// are forwarded to the master writer so the child unblocks on its host queries.
    pub(super) fn drain_into(
        &mut self,
        terminal: &mut impl TerminalSession,
        idle: Duration,
    ) -> Result<bool> {
        let mut saw_output = false;
        while let Ok(bytes) = self.reader.recv_timeout(idle) {
            log_pty_drain("draining PTY bytes into terminal", &bytes);
            terminal.write_vt(&bytes);
            saw_output = true;
            while let Ok(bytes) = self.reader.try_recv() {
                log_pty_drain("draining buffered PTY bytes into terminal", &bytes);
                terminal.write_vt(&bytes);
            }
            let reply = terminal.take_pending_pty_reply();
            if !reply.is_empty() {
                tracing::trace!(
                    bytes = reply.len(),
                    sample = %ByteSample(&reply),
                    "forwarding terminal reply to PTY",
                );
                self.writer.write_all(&reply).into_diagnostic()?;
            }
        }
        Ok(saw_output)
    }

    /// Consume only bytes already queued, preserving a bounded tape-command boundary.
    pub(super) fn drain_queued(&mut self, terminal: &mut impl TerminalSession) -> Result<()> {
        let queued: Vec<_> = self.reader.try_iter().take(128).collect();
        if queued.len() == 128 {
            return Err(
                miette!("PTY queue too busy at title boundary; synchronize the app first").into(),
            );
        }
        for bytes in queued {
            terminal.write_vt(&bytes);
            let reply = terminal.take_pending_pty_reply();
            if !reply.is_empty() {
                self.write_all(&reply)?;
            }
        }
        Ok(())
    }

    /// Wait for 100ms of PTY quiet, bounded by the tape's wait timeout.
    ///
    /// Unlike a Wait, this does not succeed on an intermediate matching frame.
    pub(super) fn settle_into(
        &mut self,
        terminal: &mut impl TerminalSession,
        timeout: Duration,
    ) -> Result<()> {
        let started = Instant::now();
        let idle = Duration::from_millis(100);
        let mut last_output = started;
        loop {
            if started.elapsed() >= timeout {
                return Err(miette!("terminal output did not settle within {timeout:?}").into());
            }
            let remaining_idle = idle.saturating_sub(last_output.elapsed());
            if remaining_idle.is_zero() {
                // A delayed runner is not evidence of quiet if output is already buffered.
                if self.read_once_into(terminal, Duration::ZERO)? {
                    last_output = Instant::now();
                    continue;
                }
                return Ok(());
            }
            let wait = remaining_idle.min(timeout.saturating_sub(started.elapsed()));
            if self.read_once_into(terminal, wait)? {
                last_output = Instant::now();
            }
        }
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

    /// Hold a due capture until output pauses and no image transmission is open, for up to `limit`.
    ///
    /// Programs without synchronized output replace an image by deleting it and then sending its
    /// successor; capturing in between would record a blank image area. Returns whether the
    /// capture can proceed: `false` when `limit` ran out with a transmission still open.
    fn settle_for_capture(
        &mut self,
        terminal: &mut impl TerminalSession,
        limit: Duration,
    ) -> Result<bool> {
        let started = Instant::now();
        loop {
            let remaining = limit.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Ok(!terminal.image_transmission_in_progress());
            }
            let in_progress = terminal.image_transmission_in_progress();
            let wait = if in_progress {
                remaining
            } else {
                CAPTURE_QUIET.min(remaining)
            };
            if !self.read_burst_into(terminal, wait, remaining)? && !in_progress {
                return Ok(true);
            }
        }
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

    /// Drain PTY output without terminal capture.
    ///
    /// This is used only for tapes that do not need screenshots, waits, or state output. Bytes are
    /// intentionally discarded after they prove the process is alive because no terminal model is
    /// available to interpret them.
    pub(super) fn drain_output(&mut self, idle: Duration) -> Result<bool> {
        let mut saw_output = false;
        while self.reader.recv_timeout(idle).is_ok() {
            saw_output = true;
            while self.reader.try_recv().is_ok() {}
        }
        Ok(saw_output)
    }

    /// Drain PTY output for a fixed duration and append frames at the capture cadence.
    ///
    /// A final short drain catches output that arrives just after the loop exits; this reduces
    /// flicker around command boundaries without extending the visible delay.
    pub(super) fn drain_for(
        &mut self,
        terminal: &mut impl TerminalSession,
        duration: Duration,
        settings: &Settings,
        capture: &mut CaptureState,
    ) -> Result<()> {
        self.drain_until(terminal, Instant::now() + duration, settings, capture)?;
        // Input pacing must not wait for the child to stop producing output. Leave buffered
        // chunks for the next command, whose wait or assertion owns its own deadline.
        self.read_once_into(terminal, POST_DURATION_IDLE)?;
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
        while Instant::now() < deadline {
            let now = Instant::now();
            // Hidden periods never capture, so waiting for a capture slot would busy-spin until
            // the deadline and starve the child process of CPU.
            let next_due = if capture.visible {
                (last_capture_at + capture_interval).max(now)
            } else {
                deadline
            };
            let wait = deadline.min(next_due).saturating_duration_since(now);
            let read = self.read_burst_into(terminal, wait, capture_interval)?;
            changed |= read;
            let mut due = capture.visible
                && (last_capture_at.elapsed() >= capture_interval || Instant::now() >= deadline);
            if due && read && !self.settle_for_capture(terminal, capture_interval)? {
                due = last_capture_at.elapsed() >= MAX_TRANSMISSION_DEFER;
            }
            collect_title_changes(terminal, settings, capture)?;
            if due {
                last_capture_at = record_capture(terminal, settings, capture, changed)?;
                changed = false;
            }
        }
        // A capture deferred for streaming output, or a drain that starts after its deadline,
        // would leave time before the deadline unrecorded. Nothing records it if the tape ends
        // here, so capture now even if output is mid-update.
        if capture.visible && last_capture_at < deadline {
            collect_title_changes(terminal, settings, capture)?;
            record_capture(terminal, settings, capture, changed)?;
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
        let started = Instant::now();
        let capture_interval = settings.capture_interval();
        let mut last_capture_at = *capture.recorded_until.get_or_insert(started);
        while started.elapsed() < timeout {
            let wait = capture_interval.min(timeout.saturating_sub(started.elapsed()));
            let read = self.read_burst_into(terminal, wait, capture_interval)?;
            let mut due = capture.visible && last_capture_at.elapsed() >= capture_interval;
            if due && read && !self.settle_for_capture(terminal, capture_interval)? {
                due = last_capture_at.elapsed() >= MAX_TRANSMISSION_DEFER;
            }
            collect_title_changes(terminal, settings, capture)?;
            let text = wait_target_text(terminal, target)?;
            let matches = matcher.is_match(&text);
            let captured_at = Instant::now();
            let elapsed = captured_at.saturating_duration_since(last_capture_at);
            if capture.visible && (due || matches) {
                let frame_delay = settings.playback_delay(elapsed);
                append_visible_frame(
                    capture,
                    capture_frame(terminal, settings, capture.frames.len())?,
                    frame_delay,
                );
                last_capture_at = captured_at;
                capture.recorded_until = Some(captured_at);
            }
            if matches {
                return Ok(());
            }
            if self.output_closed {
                // No more output can arrive, so the pattern can never match.
                let status = match self.closed_shell_status() {
                    ShellStatus::Running => "closed the terminal".to_string(),
                    status => status.to_string(),
                };
                return Err(miette!(
                    "shell {status} before {} matched {}",
                    wait_target_name(target),
                    wait_pattern_name(pattern)
                )
                .into());
            }
        }

        Err(miette!(
            "timed out waiting for {} to match {}",
            wait_target_name(target),
            wait_pattern_name(pattern)
        )
        .into())
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

        pty.settle_into(&mut terminal, Duration::from_secs(2))
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
    /// placeholder cells. Every recorded frame must show an image, and the recording must keep up.
    #[test]
    fn rapidly_replaced_images_are_never_captured_blank_and_do_not_stall() {
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
        // How many frames land depends on runner speed; a slow runner drains several updates per
        // burst. Only blank frames are a failure.
        assert!(!shown.is_empty(), "no frame showed the image");
        let blank = shown.iter().filter(|pixel| !colors.contains(pixel)).count();
        assert_eq!(blank, 0, "{blank} of {} frames lost the image", shown.len());
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
