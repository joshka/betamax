use std::path::PathBuf;
use std::time::Duration;

use miette::miette;
use regex::Regex;

use super::{Command, Key, KeyCode, KeyModifiers, Tape, Value, WaitPattern, WaitTarget};
use crate::Result;

/// Percent divisor used when parsing `Set` values like `50%`.
const PERCENT_DIVISOR: f64 = 100.0;
/// Number of milliseconds in one second.
const MILLIS_PER_SECOND: f64 = 1000.0;
/// Number of seconds in one minute.
const SECONDS_PER_MINUTE: f64 = 60.0;
/// Minimum function-key number accepted by the tape syntax.
const MIN_FUNCTION_KEY: u8 = 1;
/// Maximum function-key number accepted by the tape syntax.
const MAX_FUNCTION_KEY: u8 = 25;

/// Parse a VHS-style tape source string into commands.
///
/// The parser is kept separate from the public command model so the syntax cursor, token
/// lookahead, and validation rules do not obscure what a parsed tape represents.
pub(super) fn parse_tape(source: &str) -> Result<Tape> {
    let mut commands = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let tokens = shell_words::split(trimmed)
            .map_err(|err| miette!("line {line_number}: failed to tokenize tape line: {err}"))?;
        parse_tokens(line_number, &tokens, &mut commands)?;
    }

    validate_command_order(&commands)?;
    Ok(Tape { commands })
}

/// Parse one tokenized tape line into one or more commands.
///
/// The cursor-based loop is what allows command chaining on a single line. Each branch is
/// responsible for consuming only the tokens that belong to that command; if it consumes too much,
/// the next command silently disappears, so the tests focus on chained-command examples.
fn parse_tokens(line_number: usize, tokens: &[String], commands: &mut Vec<Command>) -> Result<()> {
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        let token = &tokens[cursor];
        let (name, delay) = parse_timed_name(token)?;
        cursor += 1;

        match name {
            "Output" => {
                let path = required_token(line_number, tokens, cursor, "Output path")?;
                commands.push(Command::Output(PathBuf::from(path)));
                cursor += 1;
            }
            "Require" => {
                let program = required_token(line_number, tokens, cursor, "required program")?;
                commands.push(Command::Require(program.to_string()));
                cursor += 1;
            }
            "Set" => {
                let key = required_token(line_number, tokens, cursor, "setting name")?;
                let value = required_token(line_number, tokens, cursor + 1, "setting value")?;
                commands.push(Command::Set {
                    key: key.to_string(),
                    value: parse_value(value),
                });
                cursor += 2;
            }
            "Sleep" => {
                let (duration, consumed) = parse_duration_tokens(line_number, tokens, cursor)?;
                commands.push(Command::Sleep(duration));
                cursor += consumed;
                continue;
            }
            "Copy" => {
                let text = required_token(line_number, tokens, cursor, "text to copy")?;
                commands.push(Command::Copy(text.to_string()));
                cursor += 1;
            }
            "Paste" => commands.push(Command::Paste),
            "Type" => {
                let text = required_token(line_number, tokens, cursor, "text to type")?;
                commands.push(Command::Type {
                    text: text.to_string(),
                    delay,
                });
                cursor += 1;
            }
            _ if name == "Wait" || name.starts_with("Wait+") => {
                let target = parse_wait_target(line_number, name)?;
                let pattern = match tokens.get(cursor) {
                    Some(token) if !is_command_token(token) => {
                        cursor += 1;
                        Some(parse_wait_pattern(line_number, token)?)
                    }
                    _ => None,
                };
                commands.push(Command::Wait {
                    target,
                    pattern,
                    timeout: delay,
                });
            }
            "Hide" => commands.push(Command::Hide),
            "Show" => commands.push(Command::Show),
            "Env" => {
                let key = required_token(line_number, tokens, cursor, "environment variable name")?;
                let value = required_token(
                    line_number,
                    tokens,
                    cursor + 1,
                    "environment variable value",
                )?;
                commands.push(Command::Env {
                    key: key.to_string(),
                    value: value.to_string(),
                });
                cursor += 2;
            }
            "Source" => {
                let path = required_token(line_number, tokens, cursor, "source path")?;
                commands.push(Command::Source(PathBuf::from(path)));
                cursor += 1;
            }
            "Screenshot" => {
                let path = required_token(line_number, tokens, cursor, "screenshot path")?;
                commands.push(Command::Screenshot(PathBuf::from(path)));
                cursor += 1;
            }
            "State" => {
                let path = required_token(line_number, tokens, cursor, "state path")?;
                commands.push(Command::State(PathBuf::from(path)));
                cursor += 1;
            }
            _ => {
                if let Some(key) = parse_key(name) {
                    let count = optional_count(tokens.get(cursor))?;
                    if count.is_some() {
                        cursor += 1;
                    }
                    commands.push(Command::Key {
                        key,
                        delay,
                        count: count.unwrap_or(1),
                    });
                } else {
                    return Err(
                        miette!("line {line_number}: unsupported tape command `{name}`").into(),
                    );
                }
            }
        }
    }

    Ok(())
}

/// Return a required token or attach line-number context to the parse error.
fn required_token<'a>(
    line_number: usize,
    tokens: &'a [String],
    index: usize,
    name: &str,
) -> Result<&'a str> {
    Ok(tokens
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| miette!("line {line_number}: missing {name}"))?)
}

/// Split a command token into its command name and optional `@duration` suffix.
///
/// The suffix is used by commands whose delay naturally belongs to the command name, such as
/// `Type@50ms`, `Enter@250ms`, and `Wait@5s`.
fn parse_timed_name(token: &str) -> Result<(&str, Option<Duration>)> {
    let Some((name, duration)) = token.split_once('@') else {
        return Ok((token, None));
    };
    Ok((name, Some(parse_duration(duration)?)))
}

/// Parse a setting value using VHS-compatible loose typing.
///
/// The order is intentional: percentages become normalized numbers before numeric parsing,
/// booleans parse before durations only because duration literals contain units, and strings are
/// the fallback so unknown future settings can still round-trip through the command model.
fn parse_value(value: &str) -> Value {
    if let Some(percent) = value.strip_suffix('%') {
        if let Ok(percent) = percent.parse::<f64>() {
            return Value::Number(percent / PERCENT_DIVISOR);
        }
    }
    if let Ok(number) = value.parse::<f64>() {
        return Value::Number(number);
    }
    if let Ok(value) = value.parse::<bool>() {
        return Value::Bool(value);
    }
    if let Ok(duration) = parse_duration(value) {
        return Value::Duration(duration);
    }
    Value::String(value.to_string())
}

/// Parse the arguments to `Sleep`, supporting both compact and split unit forms.
fn parse_duration_tokens(
    line_number: usize,
    tokens: &[String],
    cursor: usize,
) -> Result<(Duration, usize)> {
    let value = required_token(line_number, tokens, cursor, "duration")?;
    if let Some(unit) = tokens
        .get(cursor + 1)
        .filter(|token| is_duration_unit(token))
    {
        return Ok((parse_duration_with_unit(value, unit)?, 2));
    }
    Ok((parse_duration(value)?, 1))
}

/// Parse a duration literal.
///
/// Bare numbers are seconds. Supported suffixes are milliseconds (`ms`), seconds (`s`), and minutes
/// (`m`). Negative durations are rejected because they would make execution timing ambiguous.
///
/// # Errors
///
/// Returns an error when the value is not numeric, uses an unsupported suffix, or is zero or
/// negative.
pub fn parse_duration(value: &str) -> Result<Duration> {
    if let Some(ms) = value.strip_suffix("ms") {
        return Ok(Duration::from_secs_f64(
            parse_duration_number(value, ms)? / MILLIS_PER_SECOND,
        ));
    }
    if let Some(seconds) = value.strip_suffix('s') {
        let seconds = parse_duration_number(value, seconds)?;
        return Ok(Duration::from_secs_f64(seconds));
    }
    if let Some(minutes) = value.strip_suffix('m') {
        let minutes = parse_duration_number(value, minutes)?;
        return Ok(Duration::from_secs_f64(minutes * SECONDS_PER_MINUTE));
    }
    if let Ok(seconds) = value.parse::<f64>() {
        return Ok(Duration::from_secs_f64(seconds));
    }
    Err(miette!("invalid duration `{value}`").into())
}

/// Parse a duration represented as separate value and unit tokens.
fn parse_duration_with_unit(value: &str, unit: &str) -> Result<Duration> {
    let value = parse_duration_number(value, value)?;
    match unit {
        "ms" | "millisecond" | "milliseconds" => {
            Ok(Duration::from_secs_f64(value / MILLIS_PER_SECOND))
        }
        "s" | "sec" | "secs" | "second" | "seconds" => Ok(Duration::from_secs_f64(value)),
        "m" | "min" | "mins" | "minute" | "minutes" => {
            Ok(Duration::from_secs_f64(value * SECONDS_PER_MINUTE))
        }
        _ => Err(miette!("invalid duration unit `{unit}`").into()),
    }
}

/// Parse and validate the numeric part of a duration.
fn parse_duration_number(original: &str, value: &str) -> Result<f64> {
    let number = value
        .parse::<f64>()
        .map_err(|_| miette!("invalid duration `{original}`"))?;
    if number.is_sign_negative() {
        return Err(miette!("duration must be positive: `{original}`").into());
    }
    Ok(number)
}

/// Return whether a token is a recognized duration unit for split duration forms.
fn is_duration_unit(value: &str) -> bool {
    matches!(
        value,
        "ms" | "millisecond"
            | "milliseconds"
            | "s"
            | "sec"
            | "secs"
            | "second"
            | "seconds"
            | "m"
            | "min"
            | "mins"
            | "minute"
            | "minutes"
    )
}

/// Parse an optional repeat count after a key token.
///
/// Only all-digit tokens are treated as counts. Anything else is left for the main parser to treat
/// as the next command token.
fn optional_count(token: Option<&String>) -> Result<Option<u16>> {
    match token {
        Some(token) if token.chars().all(|ch| ch.is_ascii_digit()) => Ok(token
            .parse::<u16>()
            .map(Some)
            .map_err(|_| miette!("invalid key repeat count `{token}`"))?),
        _ => Ok(None),
    }
}

/// Parse a key command name such as `Enter`, `Ctrl+C`, or `Shift+Tab`.
fn parse_key(name: &str) -> Option<Key> {
    let mut modifiers = KeyModifiers::default();
    let mut key = None;
    for part in name.split('+') {
        match part {
            "Alt" => modifiers.alt = true,
            "Ctrl" => modifiers.ctrl = true,
            "Shift" => modifiers.shift = true,
            part => key = parse_key_code(part),
        }
    }
    key.map(|key| Key::Press { key, modifiers })
}

/// Map a key name to the internal key code.
fn parse_key_code(name: &str) -> Option<KeyCode> {
    match name {
        "Escape" => Some(KeyCode::Escape),
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        "Insert" => Some(KeyCode::Insert),
        "Down" => Some(KeyCode::Down),
        "Enter" => Some(KeyCode::Enter),
        "Space" => Some(KeyCode::Space),
        "Tab" => Some(KeyCode::Tab),
        "Left" => Some(KeyCode::Left),
        "Right" => Some(KeyCode::Right),
        "Up" => Some(KeyCode::Up),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        _ => parse_function_key(name).or_else(|| parse_char_key(name)),
    }
}

/// Parse `F1` through `F25`.
fn parse_function_key(name: &str) -> Option<KeyCode> {
    let number = name.strip_prefix('F')?.parse::<u8>().ok()?;
    (MIN_FUNCTION_KEY..=MAX_FUNCTION_KEY)
        .contains(&number)
        .then_some(KeyCode::Function(number))
}

/// Parse a single-character key token.
fn parse_char_key(name: &str) -> Option<KeyCode> {
    let mut chars = name.chars();
    let ch = chars.next()?;
    chars.next().is_none().then_some(KeyCode::Char(ch))
}

/// Return whether a token could begin a new command.
///
/// This is used while parsing optional arguments such as wait patterns. A false positive here is
/// safer than a false negative because it preserves the next command rather than swallowing it as
/// an argument.
fn is_command_token(token: &str) -> bool {
    let Ok((name, _)) = parse_timed_name(token) else {
        return false;
    };
    matches!(
        name,
        "Output"
            | "Require"
            | "Set"
            | "Sleep"
            | "Type"
            | "Hide"
            | "Show"
            | "Env"
            | "Copy"
            | "Paste"
            | "Source"
            | "Screenshot"
            | "State"
    ) || name == "Wait"
        || name.starts_with("Wait+")
        || parse_key(name).is_some()
}

/// Parse the optional `+Line` or `+Screen` suffix from a wait command name.
fn parse_wait_target(line_number: usize, name: &str) -> Result<WaitTarget> {
    match name.split_once('+').map(|(_, target)| target) {
        None => Ok(WaitTarget::Line),
        Some("Line") => Ok(WaitTarget::Line),
        Some("Screen") => Ok(WaitTarget::Screen),
        Some(target) => {
            Err(miette!("line {line_number}: Wait+ expects Line or Screen, got `{target}`").into())
        }
    }
}

/// Parse a wait pattern token.
///
/// Slash-delimited patterns are compiled immediately as regexes. Other tokens are plain substring
/// matches, which keeps common prompt and output waits easy to write.
fn parse_wait_pattern(line_number: usize, pattern: &str) -> Result<WaitPattern> {
    if pattern.len() >= 2 && pattern.starts_with('/') && pattern.ends_with('/') {
        let pattern = pattern[1..pattern.len() - 1].to_string();
        Regex::new(&pattern)
            .map_err(|error| miette!("line {line_number}: invalid regex `{pattern}`: {error}"))?;
        return Ok(WaitPattern::Regex(pattern));
    }
    Ok(WaitPattern::Contains(pattern.to_string()))
}

/// Enforce that startup-affecting commands appear before runtime commands.
///
/// This keeps PTY setup deterministic: environment variables, required programs, and settings are
/// all known before the shell is spawned or capture is configured.
fn validate_command_order(commands: &[Command]) -> Result<()> {
    let mut saw_runtime_command = false;
    for command in commands {
        match command {
            Command::Set { key, .. } | Command::Env { key, .. } if saw_runtime_command => {
                return Err(
                    miette!("`{key}` must be configured before runtime tape commands").into(),
                );
            }
            Command::Require(program) if saw_runtime_command => {
                return Err(miette!(
                    "`Require {program}` must appear before runtime tape commands"
                )
                .into());
            }
            Command::Output(_)
            | Command::Set { .. }
            | Command::Env { .. }
            | Command::Require(_) => {}
            _ => saw_runtime_command = true,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_commands_on_one_line() {
        let tape = Tape::parse(r#"Type "echo hi" Sleep 500ms Enter"#).unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::Type {
                    text: "echo hi".to_string(),
                    delay: None,
                },
                Command::Sleep(Duration::from_millis(500)),
                Command::Key {
                    key: Key::Press {
                        key: KeyCode::Enter,
                        modifiers: KeyModifiers::default(),
                    },
                    delay: None,
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn parses_vhs_style_duration_forms() {
        let tape = Tape::parse("Sleep 0.5 Sleep 500 ms Sleep 1 s").unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::Sleep(Duration::from_millis(500)),
                Command::Sleep(Duration::from_millis(500)),
                Command::Sleep(Duration::from_secs(1)),
            ]
        );
    }

    #[test]
    fn parses_wait_targets_and_regex_patterns() {
        let tape = Tape::parse(r#"Wait /foo.*/ Wait+Screen@2s "done""#).unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::Wait {
                    target: WaitTarget::Line,
                    pattern: Some(WaitPattern::Regex("foo.*".to_string())),
                    timeout: None,
                },
                Command::Wait {
                    target: WaitTarget::Screen,
                    pattern: Some(WaitPattern::Contains("done".to_string())),
                    timeout: Some(Duration::from_secs(2)),
                },
            ]
        );
    }

    #[test]
    fn parses_copy_paste_and_modified_keys() {
        let tape = Tape::parse(r#"Copy "hello" Paste Ctrl+Alt+C F5 Shift+Tab"#).unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::Copy("hello".to_string()),
                Command::Paste,
                Command::Key {
                    key: Key::Press {
                        key: KeyCode::Char('C'),
                        modifiers: KeyModifiers {
                            alt: true,
                            ctrl: true,
                            shift: false,
                        },
                    },
                    delay: None,
                    count: 1,
                },
                Command::Key {
                    key: Key::Press {
                        key: KeyCode::Function(5),
                        modifiers: KeyModifiers::default(),
                    },
                    delay: None,
                    count: 1,
                },
                Command::Key {
                    key: Key::Press {
                        key: KeyCode::Tab,
                        modifiers: KeyModifiers {
                            alt: false,
                            ctrl: false,
                            shift: true,
                        },
                    },
                    delay: None,
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn rejects_settings_after_runtime_commands() {
        let err = Tape::parse("Type hi\nSet Width 800").unwrap_err();
        assert!(err.to_string().contains("before runtime tape commands"));
    }

    #[test]
    fn parses_documentation_style_tape() {
        let tape = Tape::parse(
            r##"
            Output docs.gif
            Set Shell "bash"
            Set Theme "Aardvark Blue"
            Set MarginFill "#102040"
            Hide
            Type "cargo build --quiet"
            Enter
            Wait
            Show
            Type "printf 'hello from betamax\n'"
            Enter
            Wait+Screen "hello from betamax"
            "##,
        )
        .unwrap();
        assert!(tape
            .outputs()
            .any(|path| path == PathBuf::from("docs.gif").as_path()));
    }
}
