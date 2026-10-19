//! Input HUD configuration and label lifetime in recorded frame time.

mod chips;
mod labels;

use std::time::Duration;

pub(super) use chips::{
    draw_keyboard_overlay, keyboard_overlay_panel_width, HorizontalAnchor, KeyboardOverlayAnchor,
    VerticalAnchor, KEYBOARD_OVERLAY_INSET_Y, KEYBOARD_OVERLAY_LINE_HEIGHT,
    KEYBOARD_OVERLAY_MAX_ROWS, KEYBOARD_OVERLAY_TEXT_OFFSET_Y,
};
use miette::miette;

use crate::tape::Command;
use crate::Result;

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
    pub(super) fn visible(&self, labels: &[String]) -> bool {
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

/// Parse the presentation mode accepted by `Set KeyboardOverlay`.
pub(super) fn parse_keyboard_overlay_mode(mode: &str) -> Result<KeyboardOverlayMode> {
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
pub(super) fn parse_keyboard_overlay_location(location: &str) -> Result<KeyboardOverlayLocation> {
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

/// Inset used when keyboard chips are anchored inside the terminal canvas.
pub(super) const KEYBOARD_OVERLAY_CORNER_INSET: u32 = 18;

/// How long an input label remains visible after being queued.
const KEYBOARD_OVERLAY_LINGER: Duration = Duration::from_millis(1500);

/// Input labels and their remaining lifetime in recorded frame time.
#[derive(Debug, Default)]
pub(super) struct KeyboardOverlayState {
    events: Vec<KeyboardOverlayEvent>,
}

impl KeyboardOverlayState {
    /// Queue a keyboard overlay label for the next captured frames.
    pub(super) fn queue(&mut self, label: String) {
        self.events.push(KeyboardOverlayEvent {
            label,
            remaining: KEYBOARD_OVERLAY_LINGER,
        });
    }

    /// Return active keyboard overlay labels in queue order.
    pub(super) fn labels(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|event| event.label.clone())
            .collect()
    }

    /// Age active keyboard overlay labels by recorded frame time.
    pub(super) fn advance(&mut self, delay: Duration) {
        for event in &mut self.events {
            event.remaining = event.remaining.saturating_sub(delay);
        }
        self.events.retain(|event| !event.remaining.is_zero());
    }
}

/// One keyboard overlay event that is currently visible.
#[derive(Debug, Clone, PartialEq, Eq)]
struct KeyboardOverlayEvent {
    /// Label drawn in the overlay HUD.
    label: String,
    /// Remaining visible time for this label.
    remaining: Duration,
}

impl KeyboardOverlaySettings {
    /// Height reserved for the largest keyboard overlay panel the configured mode can draw.
    pub(super) fn keyboard_overlay_height(&self) -> u32 {
        if self.mode == KeyboardOverlayMode::Off {
            return 0;
        }
        let row_height = KEYBOARD_OVERLAY_LINE_HEIGHT.ceil() as u32;
        KEYBOARD_OVERLAY_INSET_Y
            .saturating_add(row_height.saturating_mul(KEYBOARD_OVERLAY_MAX_ROWS as u32))
    }

    /// Return the display label for an eligible tape input command.
    pub(super) fn label(&self, command: &Command) -> Option<String> {
        labels::keyboard_overlay_label(command, self.mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::settings::Settings;
    use crate::tape::Tape;

    #[test]
    fn invalid_keyboard_overlay_modes_are_errors() {
        let tape = Tape::parse("Set Preset Default-0.1\nSet KeyboardOverlay floating").unwrap();
        let error = Settings::from_tape(&tape).unwrap_err().to_string();

        assert!(error.contains("Set KeyboardOverlay expects Off, Keys, Input, or All"));
    }

    #[test]
    fn keyboard_overlay_location_is_configurable() {
        let tape =
            Tape::parse("Set Preset Default-0.1\nSet KeyboardOverlayLocation BottomRight").unwrap();
        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(
            settings.presentation.keyboard_overlay.location,
            KeyboardOverlayLocation::BottomRight
        );
    }

    #[test]
    fn staggered_labels_expire_in_recorded_time_and_keep_queue_order() {
        let mut state = KeyboardOverlayState::default();
        state.queue("Enter".into());
        state.advance(Duration::from_millis(500));
        state.queue("Ctrl+P".into());
        state.advance(Duration::from_millis(1000));
        assert_eq!(state.labels(), vec!["Ctrl+P"]);
        state.advance(Duration::from_millis(500));
        assert!(state.labels().is_empty());
    }
}
