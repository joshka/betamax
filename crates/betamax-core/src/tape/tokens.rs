//! Tape line tokenization.
//!
//! Words follow shell-like rules: whitespace separates words, single quotes keep text literally,
//! double quotes and backslashes escape as in a POSIX shell, and `#` at the start of a word
//! comments out the rest of the line.
//!
//! Regex literals are the exception. After a wait command or `Set WaitPattern`, an unquoted word
//! starting with `/` is kept exactly as written up to a closing `/` at the end of a word, as VHS
//! does. Backslashes survive, so `/v[0-9]+\.[0-9]+/` matches a literal dot, and spaces stay
//! inside the pattern, so `/build ok/` is one word. A `/` preceded by an odd number of
//! backslashes does not close the literal.

use std::fmt;
use std::ops::Deref;

/// A quote opened on a tape line was never closed.
#[derive(Debug)]
pub(super) struct UnclosedQuote;

impl fmt::Display for UnclosedQuote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("missing closing quote")
    }
}

/// One word of a tape line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Token {
    /// The word after quotes and escapes are removed.
    pub(super) text: String,
    /// Whether the word used quotes or backslash escapes. Such a word is always text, never a
    /// command, so `Wait+Screen "Enter"` waits for `Enter` instead of pressing it.
    pub(super) quoted: bool,
}

impl Deref for Token {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text
    }
}

/// Split one tape line into words.
pub(super) fn split_line(line: &str) -> Result<Vec<Token>, UnclosedQuote> {
    let mut tokens = Vec::new();
    // Raw spellings of the words so far; a quoted `"Wait"` is not a wait command.
    let mut raw_words = Vec::new();
    let mut rest = line;
    loop {
        rest = rest.trim_start_matches(is_separator);
        if rest.is_empty() || rest.starts_with('#') {
            break;
        }
        let (token, len) = match regex_literal_len(rest).filter(|_| expects_regex(&raw_words)) {
            Some(len) => {
                let token = Token {
                    text: rest[..len].to_string(),
                    quoted: false,
                };
                (token, len)
            }
            None => {
                let (text, len) = shell_word(rest)?;
                let quoted = rest[..len].contains(['\'', '"', '\\']);
                (Token { text, quoted }, len)
            }
        };
        tokens.push(token);
        raw_words.push(&rest[..len]);
        rest = &rest[len..];
    }
    Ok(tokens)
}

fn is_separator(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n')
}

/// Whether the next word is a regex argument: after `Wait`, `Wait+Line`, or `Wait+Screen` (with
/// any `@duration`), or after `Set WaitPattern`.
fn expects_regex(raw_words: &[&str]) -> bool {
    match raw_words {
        [.., "Set", "WaitPattern"] => true,
        [.., last] => {
            let name = last.split_once('@').map_or(*last, |(name, _)| name);
            name == "Wait" || name.starts_with("Wait+")
        }
        [] => false,
    }
}

/// Length of a `/regex/` literal at the start of `text`, if it has a closing `/` at a word end.
///
/// Without one, the word is not a regex literal and normal word rules apply.
fn regex_literal_len(text: &str) -> Option<usize> {
    let body = text.strip_prefix('/')?;
    let mut escaped = false;
    for (index, ch) in body.char_indices() {
        match ch {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '/' => {
                let end = 1 + index + 1;
                if text[end..].chars().next().is_none_or(is_separator) {
                    return Some(end);
                }
            }
            _ => {}
        }
    }
    None
}

/// Read one shell-quoted word from the start of `text`, returning it and its source length.
///
/// This matches `shell_words::split` for a single word.
fn shell_word(text: &str) -> Result<(String, usize), UnclosedQuote> {
    enum State {
        Unquoted,
        Backslash,
        SingleQuoted,
        DoubleQuoted,
        DoubleQuotedBackslash,
    }

    let mut word = String::new();
    let mut state = State::Unquoted;
    for (index, ch) in text.char_indices() {
        state = match state {
            State::Unquoted => match ch {
                _ if is_separator(ch) => return Ok((word, index)),
                '\'' => State::SingleQuoted,
                '"' => State::DoubleQuoted,
                '\\' => State::Backslash,
                _ => {
                    word.push(ch);
                    State::Unquoted
                }
            },
            State::Backslash => {
                if ch != '\n' {
                    word.push(ch);
                }
                State::Unquoted
            }
            State::SingleQuoted => match ch {
                '\'' => State::Unquoted,
                _ => {
                    word.push(ch);
                    State::SingleQuoted
                }
            },
            State::DoubleQuoted => match ch {
                '"' => State::Unquoted,
                '\\' => State::DoubleQuotedBackslash,
                _ => {
                    word.push(ch);
                    State::DoubleQuoted
                }
            },
            State::DoubleQuotedBackslash => {
                match ch {
                    '\n' => {}
                    '$' | '`' | '"' | '\\' => word.push(ch),
                    _ => {
                        word.push('\\');
                        word.push(ch);
                    }
                }
                State::DoubleQuoted
            }
        };
    }
    match state {
        State::Unquoted => {}
        State::Backslash => word.push('\\'),
        State::SingleQuoted | State::DoubleQuoted | State::DoubleQuotedBackslash => {
            return Err(UnclosedQuote)
        }
    }
    Ok((word, text.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(line: &str) -> Vec<String> {
        split_line(line)
            .unwrap()
            .into_iter()
            .map(|token| token.text)
            .collect()
    }

    #[test]
    fn matches_shell_words_outside_regex_literals() {
        for line in [
            r#"Type "echo hi" Enter Sleep 1s"#,
            r#"Type 'it''s' "a \"b\" \$c \d" e\ f g\\h"#,
            r#"Caption "" Type x # trailing comment"#,
            r##"Type a#b "#" '#'"##,
            r#"Output /tmp/a\ b.gif Type /x\.y/ trailing\"#,
            "Type\t\"tab\"  separated",
            r#"Wait+Screen "/v[0-9]+\.[0-9]+/" Wait '/a\.b/'"#,
            r#"Type "Wait" /a\.b/"#,
            r"Wait /tmp/file Enter",
        ] {
            assert_eq!(split(line), shell_words::split(line).unwrap(), "{line}");
        }
    }

    #[test]
    fn keeps_wait_regex_literals_as_written() {
        assert_eq!(
            split(r"Wait+Screen /v[0-9]+\.[0-9]+/ Enter"),
            [r"Wait+Screen", r"/v[0-9]+\.[0-9]+/", "Enter"]
        );
        assert_eq!(
            split(r"Wait@5s /build ok/ Wait+Line /it's\s+done/"),
            ["Wait@5s", "/build ok/", "Wait+Line", r"/it's\s+done/"]
        );
        assert_eq!(
            split(r"Set WaitPattern /\$ $/"),
            ["Set", "WaitPattern", r"/\$ $/"]
        );
    }

    #[test]
    fn closes_regex_literals_only_at_an_unescaped_slash_ending_a_word() {
        assert_eq!(split(r"Wait /a\/ b/ Enter"), ["Wait", r"/a\/ b/", "Enter"]);
        assert_eq!(split(r"Wait /a\\/ Enter"), ["Wait", r"/a\\/", "Enter"]);
        assert_eq!(
            split("Wait /usr/bin/ Enter"),
            ["Wait", "/usr/bin/", "Enter"]
        );
        assert_eq!(split("Wait /a b/c/"), ["Wait", "/a b/c/"]);
    }

    #[test]
    fn marks_quoted_and_escaped_words() {
        let quoted: Vec<_> = split_line(r#"Wait "Enter" 'q' \x En"ter" Enter Wait /a b/"#)
            .unwrap()
            .into_iter()
            .map(|token| token.quoted)
            .collect();
        assert_eq!(quoted, [false, true, true, true, true, false, false, false]);
    }

    #[test]
    fn reports_unclosed_quotes() {
        for line in [r#"Type "open"#, "Type 'open", r#"Type "a\"#] {
            assert!(split_line(line).is_err(), "{line}");
            assert!(shell_words::split(line).is_err(), "{line}");
        }
    }
}
