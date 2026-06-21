//! Frame collection helpers for captured runs.

use std::time::Duration;

use super::settings::Settings;
use crate::media::Frame;
use crate::runner::TerminalSession;
use crate::Result;

/// How long a keyboard overlay label remains visible after being queued.
const KEYBOARD_OVERLAY_LINGER: Duration = Duration::from_millis(1500);

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
    pub(super) frames: Vec<(CapturedFrame, Duration)>,
    /// Caption rendered onto later media frames.
    ///
    /// This is presentation metadata only. It does not affect PTY execution, terminal state, or
    /// semantic wait matching.
    pub(super) caption: Option<String>,
    /// Keyboard overlay labels that are currently visible.
    keyboard_overlay_events: Vec<KeyboardOverlayEvent>,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self {
            visible: true,
            frames: Vec::new(),
            caption: None,
            keyboard_overlay_events: Vec::new(),
        }
    }
}

/// Raw captured terminal frame plus presentation metadata for final media decoration.
#[derive(Debug, Clone)]
pub(super) struct CapturedFrame {
    /// Raw terminal frame before final margin/window/caption/overlay decoration.
    pub(super) frame: Frame,
    /// Caption active when this frame was captured.
    pub(super) caption: Option<String>,
    /// Keyboard overlay labels visible when this frame was captured.
    pub(super) keyboard_overlay_labels: Vec<String>,
}

/// One keyboard overlay event that is currently visible.
#[derive(Debug, Clone, PartialEq, Eq)]
struct KeyboardOverlayEvent {
    /// Label drawn in the overlay HUD.
    label: String,
    /// Remaining visible time for this label.
    remaining: Duration,
}

/// Queue a keyboard overlay label for the next captured frames.
pub(super) fn queue_keyboard_overlay_label(capture: &mut CaptureState, label: String) {
    capture.keyboard_overlay_events.push(KeyboardOverlayEvent {
        label,
        remaining: KEYBOARD_OVERLAY_LINGER,
    });
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
    let cursor_visible = settings.cursor_visible(frame_index);
    tracing::trace!(frame_index, cursor_visible, "capturing runner frame",);
    terminal.capture_frame_with_cursor(cursor_visible)
}

/// Append one visible frame or extend the previous frame when pixels and metadata have not changed.
///
/// This preserves wall-clock dwell time for static terminal states. Without delay coalescing, a
/// `Sleep 2s` after the final output only contributes as many nominal frame delays as the renderer
/// can sample during those two seconds, which can make GIFs play much faster than the tape timing.
///
/// Presentation metadata is included in the coalescing key even though it is not terminal pixels
/// yet. A repeated terminal frame with a new caption or keyboard overlay is a visible media change
/// after final decoration. Active keyboard overlays intentionally avoid coalescing so their linger
/// timers can age across recorded frame time.
pub(super) fn append_visible_frame(capture: &mut CaptureState, frame: Frame, delay: Duration) {
    let caption = capture.caption.clone();
    let keyboard_overlay_labels = active_keyboard_overlay_labels(capture);
    if let Some((last_frame, last_delay)) = capture.frames.last_mut() {
        if frames_equal(&last_frame.frame, &frame)
            && last_frame.caption == caption
            && last_frame.keyboard_overlay_labels == keyboard_overlay_labels
            && keyboard_overlay_labels.is_empty()
        {
            *last_delay += delay;
            age_keyboard_overlay_labels(capture, delay);
            return;
        }
    }
    capture.frames.push((
        CapturedFrame {
            frame,
            caption,
            keyboard_overlay_labels,
        },
        delay,
    ));
    age_keyboard_overlay_labels(capture, delay);
}

/// Append the final still frame for animated outputs when it differs from the last visible frame.
///
/// If the tape ended hidden, this intentionally avoids exposing cleanup-shell output. If no frames
/// were captured at all, the final frame is still added so animated outputs are non-empty.
pub(super) fn append_final_gif_frame(capture: &mut CaptureState, frame: Frame, delay: Duration) {
    if capture.visible {
        append_visible_frame(capture, frame.clone(), delay);
    }
    if capture.frames.is_empty() {
        capture.frames.push((
            CapturedFrame {
                frame,
                caption: capture.caption.clone(),
                keyboard_overlay_labels: active_keyboard_overlay_labels(capture),
            },
            delay,
        ));
    }
}

/// Return active keyboard overlay labels in queue order.
pub(super) fn active_keyboard_overlay_labels(capture: &CaptureState) -> Vec<String> {
    capture
        .keyboard_overlay_events
        .iter()
        .map(|event| event.label.clone())
        .collect()
}

/// Age active keyboard overlay labels by recorded frame time.
fn age_keyboard_overlay_labels(capture: &mut CaptureState, delay: Duration) {
    for event in &mut capture.keyboard_overlay_events {
        event.remaining = event.remaining.saturating_sub(delay);
    }
    capture
        .keyboard_overlay_events
        .retain(|event| !event.remaining.is_zero());
}

/// Compare frames byte-for-byte.
///
/// This compares the stored byte representation rather than normalized RGBA output.
///
/// Captured frames from a single renderer path share the same byte order before decoration, and all
/// frames share packed RGBA after decoration.
pub(super) fn frames_equal(left: &Frame, right: &Frame) -> bool {
    left.width == right.width
        && left.height == right.height
        && left.stride == right.stride
        && left.format == right.format
        && left.pixels == right.pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::PixelFormat;

    #[test]
    fn caption_changes_keep_repeated_frames_separate() {
        let frame = test_frame();
        let mut capture = CaptureState {
            caption: Some("First step".to_string()),
            ..Default::default()
        };

        append_visible_frame(&mut capture, frame.clone(), Duration::from_millis(20));
        capture.caption = Some("Second step".to_string());
        append_visible_frame(&mut capture, frame.clone(), Duration::from_millis(20));

        assert_eq!(capture.frames.len(), 2);
        assert_eq!(capture.frames[0].0.caption.as_deref(), Some("First step"));
        assert_eq!(capture.frames[1].0.caption.as_deref(), Some("Second step"));
    }

    fn test_frame() -> Frame {
        Frame {
            width: 1,
            height: 1,
            stride: 4,
            format: PixelFormat::Rgba8,
            pixels: vec![255, 0, 0, 255],
        }
    }
}
