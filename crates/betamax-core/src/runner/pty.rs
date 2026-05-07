//! PTY process management and output draining.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use miette::{miette, Context, IntoDiagnostic};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use super::capture::{append_visible_frame, capture_frame, CaptureState};
use super::settings::Settings;
use crate::runner::TerminalSession;
use crate::shell::{apply_terminal_environment, ShellLaunch};
use crate::tape::{WaitPattern, WaitTarget};
use crate::wait::{wait_pattern_matches, wait_pattern_name, wait_target_name, wait_target_text};
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

/// PTY process plus non-blocking output reader.
///
/// `portable-pty` exposes blocking readers. Betamax moves reads onto a background thread and uses a
/// channel so the runner can express "drain until idle", "drain for this duration", and "wait while
/// sampling frames" without blocking permanently on a shell that is waiting for input.
pub(super) struct PtySession {
    /// Writable PTY master side.
    writer: Box<dyn Write + Send>,
    /// Reader-thread channel carrying raw bytes from the PTY.
    reader: Receiver<Vec<u8>>,
    /// Child process handle kept alive for the session lifetime.
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl PtySession {
    /// Spawn the configured shell in a PTY sized to the derived terminal grid.
    ///
    /// The PTY reader runs on a background thread so command execution can drain output with idle
    /// timeouts instead of blocking forever on reads.
    pub(super) fn spawn(settings: &Settings) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: settings.rows,
                cols: settings.columns,
                pixel_width: settings.width as u16,
                pixel_height: settings.height as u16,
            })
            .map_err(|error| miette!("failed to open PTY: {error}"))?;

        let mut argv = settings.shell.clone();
        if argv.is_empty() {
            argv.push(OsString::from("sh"));
        }
        let shell = ShellLaunch::from_argv(&argv);
        argv = shell.argv;
        let mut command = CommandBuilder::from_argv(argv);
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
            writer,
            reader: output_rx,
            _child: child,
        })
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
        Ok(self.writer.write_all(bytes).into_diagnostic()?)
    }

    /// Drain PTY output into the terminal until no bytes arrive for `idle`.
    ///
    /// Returns whether any bytes were observed. After the first blocking receive succeeds, all
    /// immediately available chunks are drained without additional waits so bursty output stays
    /// grouped into the same terminal update.
    pub(super) fn drain_into(
        &mut self,
        terminal: &mut impl TerminalSession,
        idle: Duration,
    ) -> Result<bool> {
        let mut saw_output = false;
        while let Ok(bytes) = self.reader.recv_timeout(idle) {
            terminal.write_vt(&bytes);
            saw_output = true;
            while let Ok(bytes) = self.reader.try_recv() {
                terminal.write_vt(&bytes);
            }
        }
        Ok(saw_output)
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
        let started = Instant::now();
        let capture_interval = settings.capture_interval();
        let mut last_capture_at = started;
        while started.elapsed() < duration {
            let remaining = duration.saturating_sub(started.elapsed());
            let wait = remaining.min(capture_interval);
            self.drain_into(terminal, wait)?;
            if capture.visible {
                let captured_at = Instant::now();
                let frame_delay = captured_at.saturating_duration_since(last_capture_at);
                append_visible_frame(
                    capture,
                    capture_frame(terminal, settings, capture.frames.len())?,
                    frame_delay,
                );
                last_capture_at = captured_at;
            }
        }
        self.drain_into(terminal, POST_DURATION_IDLE)?;
        Ok(())
    }

    /// Wait for terminal text to match a pattern while continuing to capture frames.
    ///
    /// The timeout is checked against wall-clock time. If the pattern never matches, the error
    /// names the target and pattern so failing tapes are diagnosable without inspecting code.
    pub(super) fn wait_for(
        &mut self,
        terminal: &mut impl TerminalSession,
        target: WaitTarget,
        pattern: &WaitPattern,
        timeout: Duration,
        settings: &Settings,
        capture: &mut CaptureState,
    ) -> Result<()> {
        let started = Instant::now();
        let capture_interval = settings.capture_interval();
        let mut last_capture_at = started;
        while started.elapsed() < timeout {
            self.drain_into(terminal, capture_interval)?;
            if capture.visible {
                let captured_at = Instant::now();
                let frame_delay = captured_at.saturating_duration_since(last_capture_at);
                append_visible_frame(
                    capture,
                    capture_frame(terminal, settings, capture.frames.len())?,
                    frame_delay,
                );
                last_capture_at = captured_at;
            }
            let text = wait_target_text(terminal, target)?;
            if wait_pattern_matches(pattern, &text)? {
                return Ok(());
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
