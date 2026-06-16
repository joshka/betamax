//! Key command encoding.
//!
//! Tape key names are logical keys such as `Enter`, `Ctrl+C`, or `Shift+Tab`. The PTY needs bytes.
//! This module delegates to libghostty-vt's key encoder first so modifier behavior tracks Ghostty,
//! then supplies fallbacks for common terminal control sequences that the encoder may leave empty.

use libghostty_vt::key::{
    Action as GhosttyKeyAction, Encoder as GhosttyKeyEncoder, Event as GhosttyKeyEvent,
    Key as GhosttyKey, Mods as GhosttyMods,
};
use miette::miette;

use crate::tape::{Key, KeyCode, KeyModifiers};
use crate::trace::ByteSample;
use crate::Result;

/// Encode a parsed tape key into bytes suitable for writing to the PTY.
///
/// `Type` commands bypass this function and write text directly. This function is only for named
/// key commands, where terminal key encoding rules and modifiers matter.
pub(crate) fn key_bytes(key: &Key) -> Result<Vec<u8>> {
    let Key::Press { key, modifiers } = *key;
    let span = tracing::trace_span!(
        "ghostty_key_encode",
        key = ?key,
        modifiers = ?modifiers,
    );
    let _enter = span.enter();
    tracing::trace!("creating libghostty-vt key event");
    let mut event =
        GhosttyKeyEvent::new().map_err(|error| miette!("failed to create key event: {error}"))?;
    event
        .set_action(GhosttyKeyAction::Press)
        .set_key(ghostty_key(key))
        .set_mods(ghostty_mods(modifiers));
    if let Some(text) = key_text(key, modifiers) {
        event.set_utf8(Some(text));
    }

    tracing::trace!("creating libghostty-vt key encoder");
    let mut encoder = GhosttyKeyEncoder::new()
        .map_err(|error| miette!("failed to create key encoder: {error}"))?;
    encoder.set_alt_esc_prefix(true);
    let mut bytes = Vec::new();
    tracing::trace!("encoding key with libghostty-vt");
    encoder
        .encode_to_vec(&event, &mut bytes)
        .map_err(|error| miette!("failed to encode key: {error}"))?;
    if bytes.is_empty() {
        tracing::trace!("libghostty-vt returned no key bytes; using fallback");
        bytes = fallback_key_bytes(key, modifiers);
    }
    tracing::trace!(
        bytes = bytes.len(),
        sample = %ByteSample(&bytes),
        "encoded key bytes",
    );
    Ok(bytes)
}

/// Convert the tape key code to libghostty-vt's key enum.
///
/// Unsupported future key codes intentionally become `Unidentified`; the fallback path may still
/// know how to encode some of them.
fn ghostty_key(key: KeyCode) -> GhosttyKey {
    match key {
        KeyCode::Backspace => GhosttyKey::Backspace,
        KeyCode::Delete => GhosttyKey::Delete,
        KeyCode::Down => GhosttyKey::ArrowDown,
        KeyCode::End => GhosttyKey::End,
        KeyCode::Enter => GhosttyKey::Enter,
        KeyCode::Escape => GhosttyKey::Escape,
        KeyCode::Function(1) => GhosttyKey::F1,
        KeyCode::Function(2) => GhosttyKey::F2,
        KeyCode::Function(3) => GhosttyKey::F3,
        KeyCode::Function(4) => GhosttyKey::F4,
        KeyCode::Function(5) => GhosttyKey::F5,
        KeyCode::Function(6) => GhosttyKey::F6,
        KeyCode::Function(7) => GhosttyKey::F7,
        KeyCode::Function(8) => GhosttyKey::F8,
        KeyCode::Function(9) => GhosttyKey::F9,
        KeyCode::Function(10) => GhosttyKey::F10,
        KeyCode::Function(11) => GhosttyKey::F11,
        KeyCode::Function(12) => GhosttyKey::F12,
        KeyCode::Function(13) => GhosttyKey::F13,
        KeyCode::Function(14) => GhosttyKey::F14,
        KeyCode::Function(15) => GhosttyKey::F15,
        KeyCode::Function(16) => GhosttyKey::F16,
        KeyCode::Function(17) => GhosttyKey::F17,
        KeyCode::Function(18) => GhosttyKey::F18,
        KeyCode::Function(19) => GhosttyKey::F19,
        KeyCode::Function(20) => GhosttyKey::F20,
        KeyCode::Function(21) => GhosttyKey::F21,
        KeyCode::Function(22) => GhosttyKey::F22,
        KeyCode::Function(23) => GhosttyKey::F23,
        KeyCode::Function(24) => GhosttyKey::F24,
        KeyCode::Function(25) => GhosttyKey::F25,
        KeyCode::Function(_) => GhosttyKey::Unidentified,
        KeyCode::Home => GhosttyKey::Home,
        KeyCode::Insert => GhosttyKey::Insert,
        KeyCode::Left => GhosttyKey::ArrowLeft,
        KeyCode::PageDown => GhosttyKey::PageDown,
        KeyCode::PageUp => GhosttyKey::PageUp,
        KeyCode::Right => GhosttyKey::ArrowRight,
        KeyCode::Space => GhosttyKey::Space,
        KeyCode::Tab => GhosttyKey::Tab,
        KeyCode::Up => GhosttyKey::ArrowUp,
        KeyCode::Char(ch) => ghostty_char_key(ch),
    }
}

/// Map a single character to a Ghostty physical/logical key when possible.
///
/// Unknown Unicode characters are returned as `Unidentified`. Plain text input should normally use
/// `Type`; single-character key commands mostly exist for modified ASCII keys.
fn ghostty_char_key(ch: char) -> GhosttyKey {
    match ch.to_ascii_uppercase() {
        'A' => GhosttyKey::A,
        'B' => GhosttyKey::B,
        'C' => GhosttyKey::C,
        'D' => GhosttyKey::D,
        'E' => GhosttyKey::E,
        'F' => GhosttyKey::F,
        'G' => GhosttyKey::G,
        'H' => GhosttyKey::H,
        'I' => GhosttyKey::I,
        'J' => GhosttyKey::J,
        'K' => GhosttyKey::K,
        'L' => GhosttyKey::L,
        'M' => GhosttyKey::M,
        'N' => GhosttyKey::N,
        'O' => GhosttyKey::O,
        'P' => GhosttyKey::P,
        'Q' => GhosttyKey::Q,
        'R' => GhosttyKey::R,
        'S' => GhosttyKey::S,
        'T' => GhosttyKey::T,
        'U' => GhosttyKey::U,
        'V' => GhosttyKey::V,
        'W' => GhosttyKey::W,
        'X' => GhosttyKey::X,
        'Y' => GhosttyKey::Y,
        'Z' => GhosttyKey::Z,
        '0' => GhosttyKey::Digit0,
        '1' => GhosttyKey::Digit1,
        '2' => GhosttyKey::Digit2,
        '3' => GhosttyKey::Digit3,
        '4' => GhosttyKey::Digit4,
        '5' => GhosttyKey::Digit5,
        '6' => GhosttyKey::Digit6,
        '7' => GhosttyKey::Digit7,
        '8' => GhosttyKey::Digit8,
        '9' => GhosttyKey::Digit9,
        '-' => GhosttyKey::Minus,
        '=' => GhosttyKey::Equal,
        '[' => GhosttyKey::BracketLeft,
        ']' => GhosttyKey::BracketRight,
        '\\' => GhosttyKey::Backslash,
        ';' => GhosttyKey::Semicolon,
        '\'' => GhosttyKey::Quote,
        ',' => GhosttyKey::Comma,
        '.' => GhosttyKey::Period,
        '/' => GhosttyKey::Slash,
        '`' => GhosttyKey::Backquote,
        _ => GhosttyKey::Unidentified,
    }
}

/// Convert tape modifiers into libghostty-vt modifier flags.
fn ghostty_mods(modifiers: KeyModifiers) -> GhosttyMods {
    let mut mods = GhosttyMods::empty();
    if modifiers.alt {
        mods |= GhosttyMods::ALT;
    }
    if modifiers.ctrl {
        mods |= GhosttyMods::CTRL;
    }
    if modifiers.shift {
        mods |= GhosttyMods::SHIFT;
    }
    mods
}

/// Provide UTF-8 text for unmodified printable keys.
///
/// Ctrl and Alt combinations intentionally omit text because the encoder or fallback should produce
/// terminal control sequences instead of literal characters.
fn key_text(key: KeyCode, modifiers: KeyModifiers) -> Option<String> {
    match key {
        KeyCode::Char(ch) if !modifiers.ctrl && !modifiers.alt => Some(ch.to_string()),
        KeyCode::Space if !modifiers.ctrl && !modifiers.alt => Some(" ".to_string()),
        _ => None,
    }
}

/// Encode common keys when libghostty-vt does not produce bytes.
///
/// These fallbacks cover the terminal cases users expect from tapes: control-letter bytes, Alt as
/// ESC prefix, Enter carriage return, tabs, backspace, and Escape. Unknown combinations return an
/// empty byte vector rather than guessing an incorrect sequence.
fn fallback_key_bytes(key: KeyCode, modifiers: KeyModifiers) -> Vec<u8> {
    match (key, modifiers) {
        (
            KeyCode::Char(ch),
            KeyModifiers {
                ctrl: true, alt, ..
            },
        ) => {
            let mut bytes = if ch.is_ascii_alphabetic() {
                vec![(ch.to_ascii_uppercase() as u8).saturating_sub(b'@')]
            } else {
                Vec::new()
            };
            if alt {
                bytes.insert(0, 0x1b);
            }
            bytes
        }
        (KeyCode::Char(ch), KeyModifiers { alt: true, .. }) => {
            let mut bytes = vec![0x1b];
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            bytes
        }
        (KeyCode::Enter, _) => b"\r".to_vec(),
        (KeyCode::Tab, KeyModifiers { shift: true, .. }) => b"\x1b[Z".to_vec(),
        (KeyCode::Tab, _) => b"\t".to_vec(),
        (KeyCode::Backspace, _) => b"\x7f".to_vec(),
        (KeyCode::Escape, _) => b"\x1b".to_vec(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_encoder_handles_common_keys_and_modifiers() {
        assert_eq!(
            key_bytes(&Key::Press {
                key: KeyCode::Enter,
                modifiers: KeyModifiers::default(),
            })
            .unwrap(),
            b"\r"
        );
        assert_eq!(
            key_bytes(&Key::Press {
                key: KeyCode::Tab,
                modifiers: KeyModifiers {
                    shift: true,
                    ..KeyModifiers::default()
                },
            })
            .unwrap(),
            b"\x1b[Z"
        );
        assert_eq!(
            key_bytes(&Key::Press {
                key: KeyCode::Char('c'),
                modifiers: KeyModifiers {
                    ctrl: true,
                    ..KeyModifiers::default()
                },
            })
            .unwrap(),
            b"\x03"
        );
    }
}
