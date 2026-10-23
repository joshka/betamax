//! Tracks whether PTY output has stopped partway through a kitty graphics command.
//!
//! Programs replace an image by deleting the old one and transmitting the new one, often as many
//! `m=1` chunks. A real terminal renders between reads but parses a whole read first, so a delete
//! and its replacement usually land in the same repaint. Betamax receives the same bytes in small
//! PTY reads, so the runner asks this tracker before capturing: a frame taken while a command or a
//! chunked transmission is still open would show the image area blank.

const ESC: u8 = 0x1b;
/// Control data is short (`a=T,i=3,f=32,s=1093,v=414,m=1`); longer runs are not kitty commands.
const MAX_CONTROL_BYTES: usize = 256;

/// Scans the raw PTY stream for kitty graphics APC boundaries.
#[derive(Debug, Default)]
pub(in crate::ghostty) struct TransmissionTracker {
    state: State,
    /// Control keys of the APC being scanned, up to the `;` before the payload.
    control: Vec<u8>,
    /// Set after a command with `m=1`: more chunks of the same image follow.
    chunked: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    /// After ESC outside any string.
    Escape,
    /// After `ESC _`, waiting for the `G` that marks a kitty graphics command.
    ApcStart,
    /// Inside a kitty command's control keys.
    Control,
    /// Inside a kitty command's payload, or an APC that is not a kitty command.
    Payload { kitty: bool },
    /// After ESC inside an APC; `\` ends the string.
    PayloadEscape { kitty: bool },
}

impl TransmissionTracker {
    /// Advance over bytes that are about to be written to the terminal.
    pub(in crate::ghostty) fn observe(&mut self, bytes: &[u8]) {
        let mut index = 0;
        while index < bytes.len() {
            match self.state {
                State::Ground => match bytes[index..].iter().position(|&byte| byte == ESC) {
                    Some(offset) => {
                        index += offset;
                        self.state = State::Escape;
                    }
                    None => return,
                },
                State::Escape => {
                    self.state = if bytes[index] == b'_' {
                        State::ApcStart
                    } else {
                        State::Ground
                    };
                }
                State::ApcStart => {
                    self.control.clear();
                    self.state = match bytes[index] {
                        b'G' => State::Control,
                        ESC => State::PayloadEscape { kitty: false },
                        _ => State::Payload { kitty: false },
                    };
                }
                State::Control => match bytes[index] {
                    b';' => self.state = State::Payload { kitty: true },
                    ESC => self.state = State::PayloadEscape { kitty: true },
                    byte if self.control.len() < MAX_CONTROL_BYTES => self.control.push(byte),
                    _ => self.state = State::Payload { kitty: false },
                },
                State::Payload { kitty } => {
                    match bytes[index..].iter().position(|&byte| byte == ESC) {
                        Some(offset) => {
                            index += offset;
                            self.state = State::PayloadEscape { kitty };
                        }
                        None => return,
                    }
                }
                State::PayloadEscape { kitty } => {
                    if bytes[index] == b'\\' {
                        if kitty {
                            self.chunked = self.more_chunks();
                        }
                        self.state = State::Ground;
                    } else {
                        // ESC inside an APC aborts it; the byte starts a new sequence.
                        self.state = State::Escape;
                        continue;
                    }
                }
            }
            index += 1;
        }
    }

    /// Whether the stream is inside a kitty command or between chunks of one transmission.
    pub(in crate::ghostty) fn in_progress(&self) -> bool {
        self.chunked
            || matches!(
                self.state,
                State::ApcStart
                    | State::Control
                    | State::Payload { kitty: true }
                    | State::PayloadEscape { kitty: true }
            )
    }

    fn more_chunks(&self) -> bool {
        self.control
            .split(|&byte| byte == b',')
            .any(|key| key == b"m=1")
    }
}

#[cfg(test)]
mod tests {
    use super::TransmissionTracker;

    fn tracker_after(chunks: &[&[u8]]) -> TransmissionTracker {
        let mut tracker = TransmissionTracker::default();
        for chunk in chunks {
            tracker.observe(chunk);
        }
        tracker
    }

    #[test]
    fn complete_commands_are_not_in_progress() {
        let tracker = tracker_after(&[b"text\x1b_Ga=d,d=I,i=2,q=2\x1b\\\x1b[1mmore"]);
        assert!(!tracker.in_progress());
    }

    #[test]
    fn a_command_split_across_reads_is_in_progress_until_its_terminator() {
        let mut tracker = tracker_after(&[b"\x1b_Ga=T,i=3;AAAA"]);
        assert!(tracker.in_progress());
        tracker.observe(b"AAAA\x1b");
        assert!(tracker.in_progress());
        tracker.observe(b"\\");
        assert!(!tracker.in_progress());
    }

    #[test]
    fn chunked_transmissions_stay_in_progress_until_the_last_chunk() {
        let mut tracker = tracker_after(&[b"\x1b_Ga=T,i=3,f=32,s=1,v=1,m=1;AAAA\x1b\\"]);
        assert!(tracker.in_progress());
        tracker.observe(b"\x1b_Gm=1;AAAA\x1b\\");
        assert!(tracker.in_progress());
        tracker.observe(b"\x1b_Gm=0;AAAA\x1b\\");
        assert!(!tracker.in_progress());
    }

    #[test]
    fn other_strings_and_escapes_are_ignored() {
        let tracker = tracker_after(&[b"\x1b_other\x1b", b"\\\x1b]0;title\x07\x1b[0m"]);
        assert!(!tracker.in_progress());
        let unterminated = tracker_after(&[b"\x1b_other payload"]);
        assert!(!unterminated.in_progress());
    }
}
