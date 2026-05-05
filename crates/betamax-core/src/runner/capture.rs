//! Frame collection helpers for captured runs.

use std::time::Duration;

use super::settings::Settings;
use crate::media::Frame;
use crate::runner::TerminalSession;
use crate::Result;

/// Mutable frame-collection state for one captured run.
///
/// Visibility is a recording concern, not a terminal-state concern. Hidden commands still mutate
/// the PTY and libghostty-vt session; they only prevent frames from being appended to animated
/// outputs.
#[derive(Debug)]
pub(super) struct CaptureState {
    /// Whether frames should currently be appended to animated outputs.
    ///
    /// Hidden periods still update terminal state; they only suppress frame collection.
    pub(super) visible: bool,
    /// Captured frames and their output delays.
    ///
    /// The frame pixels represent the raw terminal render before final margin/window decoration.
    /// Decoration is applied once at the end so all outputs share the same styling path.
    pub(super) frames: Vec<(Frame, Duration)>,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self {
            visible: true,
            frames: Vec::new(),
        }
    }
}

/// Capture one raw terminal frame with runner-controlled cursor blinking.
///
/// Cursor blink is owned by runner settings rather than the renderer so GIF/video output can keep a
/// stable half-second cadence even if a future renderer exposes a different cursor policy.
pub(super) fn capture_frame(
    terminal: &mut impl TerminalSession,
    settings: &Settings,
    frame_index: usize,
) -> Result<Frame> {
    terminal.capture_frame_with_cursor(settings.cursor_visible(frame_index))
}

/// Append the final still frame for animated outputs when it differs from the last visible frame.
///
/// If the tape ended hidden, this intentionally avoids exposing cleanup-shell output. If no frames
/// were captured at all, the final frame is still added so animated outputs are non-empty.
pub(super) fn append_final_gif_frame(capture: &mut CaptureState, frame: Frame, delay: Duration) {
    if capture.visible
        && capture
            .frames
            .last()
            .map(|(last_frame, _)| !frames_equal(last_frame, &frame))
            .unwrap_or(true)
    {
        capture.frames.push((frame.clone(), delay));
    }
    if capture.frames.is_empty() {
        capture.frames.push((frame, delay));
    }
}

/// Compare frames byte-for-byte.
///
/// This compares the stored byte representation rather than normalized RGBA output. The runner only
/// uses it after decoration, where frames are guaranteed to have the same packed RGBA format.
pub(super) fn frames_equal(left: &Frame, right: &Frame) -> bool {
    left.width == right.width
        && left.height == right.height
        && left.stride == right.stride
        && left.format == right.format
        && left.pixels == right.pixels
}
