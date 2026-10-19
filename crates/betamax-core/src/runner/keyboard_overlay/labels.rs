//! Compact input-intent labels for the keyboard HUD.

use super::KeyboardOverlayMode;
use crate::tape::{Command, Key, KeyCode};

/// Maximum number of displayed characters from one `Type` command.
const KEYBOARD_OVERLAY_TYPE_MAX_CHARS: usize = 26;

/// Return the overlay label for one input-producing command.
pub(super) fn keyboard_overlay_label(
    command: &Command,
    mode: KeyboardOverlayMode,
) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::settings::Settings;
    use crate::tape::Tape;

    #[test]
    fn keyboard_overlay_keys_mode_shows_only_key_commands() {
        let tape = Tape::parse(
            r#"
            Set KeyboardOverlay Keys
            Type "open palette"
            Ctrl+P
            Down 2
            Enter
            Copy "from clipboard"
            Paste
            "#,
        )
        .unwrap();

        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(
            settings.presentation.keyboard_overlay.mode,
            KeyboardOverlayMode::Keys
        );
        assert_eq!(
            tape.commands
                .iter()
                .filter_map(|command| settings.presentation.keyboard_overlay.label(command))
                .collect::<Vec<_>>(),
            vec!["Ctrl+P", "Down x2", "Enter"]
        );
    }

    #[test]
    fn keyboard_overlay_input_mode_shows_short_typed_intent() {
        let tape = Tape::parse(
            r#"
            Set KeyboardOverlay Input
            Type "j"
            Type "echo foo"
            Type "printf 'too implementation-shaped\n'"
            Ctrl+P
            Paste
            "#,
        )
        .unwrap();

        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(
            settings.presentation.keyboard_overlay.mode,
            KeyboardOverlayMode::Input
        );
        assert_eq!(
            tape.commands
                .iter()
                .filter_map(|command| settings.presentation.keyboard_overlay.label(command))
                .collect::<Vec<_>>(),
            vec!["j", "\"echo foo\"", "Ctrl+P", "Paste"]
        );
    }

    #[test]
    fn keyboard_overlay_all_mode_summarizes_every_visible_input() {
        let tape = Tape::parse(
            r#"
            Set KeyboardOverlay All
            Type "echo foo"
            Type "printf 'implementation-shaped but visible\n'"
            Ctrl+P
            Paste
            "#,
        )
        .unwrap();

        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(
            settings.presentation.keyboard_overlay.mode,
            KeyboardOverlayMode::All
        );
        assert_eq!(
            tape.commands
                .iter()
                .filter_map(|command| settings.presentation.keyboard_overlay.label(command))
                .collect::<Vec<_>>(),
            vec!["\"echo foo\"", "Type 44 chars", "Ctrl+P", "Paste",]
        );
    }

    #[test]
    fn keyboard_overlay_labels_hidden_input_only_at_runtime() {
        let tape = Tape::parse(
            r#"
            Set KeyboardOverlay Input
            Type "visible"
            Hide
            Type "secret"
            Enter
            Show
            Down
            "#,
        )
        .unwrap();

        let settings = Settings::from_tape(&tape).unwrap();

        assert_eq!(
            tape.commands
                .iter()
                .filter_map(|command| settings.presentation.keyboard_overlay.label(command))
                .collect::<Vec<_>>(),
            vec!["\"visible\"", "\"secret\"", "Enter", "Down"]
        );
    }
}
