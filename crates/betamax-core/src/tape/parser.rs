use std::path::{Path, PathBuf};
use std::time::Duration;

use miette::miette;
use regex::Regex;

use super::diagnostics::{LateSettingEffect, TapeWarning, TapeWarningKind};
use super::{
    Assertion, Command, Key, KeyCode, KeyModifiers, MouseButton, MouseEvent, ScrollDirection, Tape,
    Value, ViewportDirection, WaitPattern, WaitTarget,
};
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

/// Maximum `Source` nesting depth, a backstop behind cycle detection.
const MAX_SOURCE_DEPTH: usize = 16;

/// Parse a VHS-style tape source string into commands.
///
/// The parser is kept separate from the public command model so the syntax cursor, token
/// lookahead, and validation rules do not obscure what a parsed tape represents. Without a
/// [`SourceContext`], `Source` is an error because there is no file to resolve paths against.
pub(super) fn parse_tape(source: &str, mut context: Option<SourceContext<'_>>) -> Result<Tape> {
    let mut parsed = Parsed::default();
    match context.as_mut() {
        Some(context) => {
            let (canonical, display) = context.tape_path.map_or_else(
                || (PathBuf::from("<stdin>"), "<stdin>".to_string()),
                |path| {
                    let canonical =
                        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
                    (canonical, path.display().to_string())
                },
            );
            context.stack.push(Frame {
                canonical,
                display,
                source_line: 0,
            });
            parse_lines(source, &mut parsed, Some(context))?;
        }
        None => parse_lines(source, &mut parsed, None)?,
    }
    let tape = Tape {
        commands: parsed.commands,
        locations: parsed.locations,
    };
    let mut warnings = parsed.warnings;
    warnings.extend(late_startup_warnings(&tape));
    // Report in command order; the sort is stable, so same-position warnings keep parse order.
    warnings.sort_by_key(|(index, _)| *index);
    if let Some(context) = context {
        for (_, warning) in warnings {
            (context.warn)(warning);
        }
    }
    Ok(tape)
}

/// File context used to expand `Source` commands.
pub(super) struct SourceContext<'a> {
    /// Path of the top-level tape, or `None` for stdin.
    tape_path: Option<&'a Path>,
    /// Directory that relative `Source` paths resolve against first.
    base_dir: PathBuf,
    /// Open tapes, outermost first, for cycle detection and error locations.
    stack: Vec<Frame>,
    /// Receives warnings, such as a dropped `Output` in a sourced tape.
    warn: &'a mut dyn FnMut(TapeWarning),
}

impl<'a> SourceContext<'a> {
    /// Build a context for a tape at `tape_path`, or for stdin when `None`.
    pub(super) fn new(tape_path: Option<&'a Path>, warn: &'a mut dyn FnMut(TapeWarning)) -> Self {
        let base_dir = tape_path
            .and_then(Path::parent)
            .map_or_else(PathBuf::new, Path::to_path_buf);
        Self {
            tape_path,
            base_dir,
            stack: Vec::new(),
            warn,
        }
    }

    /// Describe where a command on `line` of the current sourced tape came from.
    fn origin(&self, line: usize) -> Option<String> {
        let (current, parents) = self.stack.split_last()?;
        if parents.is_empty() {
            return None;
        }
        let mut origin = format!("{} line {line}", current.display);
        for parent in parents.iter().rev() {
            origin.push_str(&format!(
                ", sourced at {} line {}",
                parent.display, parent.source_line
            ));
        }
        Some(origin)
    }
}

/// One open tape in the `Source` chain.
struct Frame {
    /// Canonical path used for cycle detection.
    canonical: PathBuf,
    /// Path as shown in diagnostics.
    display: String,
    /// Line of the `Source` command currently being expanded in this tape.
    source_line: usize,
}

/// Commands, where each came from, and warnings keyed by the command index they precede.
#[derive(Default)]
struct Parsed {
    commands: Vec<Command>,
    locations: Vec<String>,
    warnings: Vec<(usize, TapeWarning)>,
}

/// Describe `line` of the current tape: `line 4` at top level, or the full `Source` chain.
fn location(context: Option<&SourceContext<'_>>, line: usize) -> String {
    context
        .and_then(|context| context.origin(line))
        .unwrap_or_else(|| format!("line {line}"))
}

/// Parse every line of `source`, expanding `Source` commands when a context is available.
fn parse_lines(
    source: &str,
    parsed: &mut Parsed,
    mut context: Option<&mut SourceContext<'_>>,
) -> Result<()> {
    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let tokens = shell_words::split(trimmed)
            .map_err(|err| miette!("line {line_number}: failed to tokenize tape line: {err}"))?;
        let mut line_commands = Vec::new();
        let mut line_warnings = Vec::new();
        parse_tokens(line_number, &tokens, &mut line_commands, &mut line_warnings)?;
        let at = location(context.as_deref(), line_number);
        for kind in line_warnings {
            let warning = TapeWarning {
                location: at.clone(),
                kind,
            };
            parsed.warnings.push((parsed.commands.len(), warning));
        }

        for command in line_commands {
            let Some(context) = context.as_deref_mut() else {
                if let Command::Source(_) = command {
                    return Err(miette!(
                        "line {line_number}: `Source` needs a tape file to resolve paths; \
                         use `Tape::parse_file`"
                    )
                    .into());
                }
                parsed.commands.push(command);
                parsed.locations.push(at.clone());
                continue;
            };
            let nested = context.stack.len() > 1;
            match command {
                Command::Source(path) => {
                    expand_source(line_number, &path, parsed, context)?;
                }
                Command::Output(path) if nested => {
                    let warning = TapeWarning {
                        location: at.clone(),
                        kind: TapeWarningKind::SourcedOutputIgnored { path },
                    };
                    parsed.warnings.push((parsed.commands.len(), warning));
                }
                command => {
                    parsed.commands.push(command);
                    parsed.locations.push(at.clone());
                }
            }
        }
    }
    Ok(())
}

/// Inline the tape named by a `Source` command on `line_number`.
fn expand_source(
    line_number: usize,
    path: &Path,
    parsed: &mut Parsed,
    context: &mut SourceContext<'_>,
) -> Result<()> {
    let at = |context: &SourceContext<'_>| {
        context.stack.last().map_or_else(
            || format!("line {line_number}"),
            |frame| format!("{} line {line_number}", frame.display),
        )
    };
    if path.extension().and_then(|ext| ext.to_str()) != Some("tape") {
        return Err(miette!(
            "{}: `Source {}` must name a file with a .tape extension",
            at(context),
            path.display()
        )
        .into());
    }
    let resolved = resolve_source_path(path, &context.base_dir).ok_or_else(|| {
        miette!(
            "{}: `Source {}` not found relative to the tape or the current directory",
            at(context),
            path.display()
        )
    })?;
    let canonical = std::fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());
    if let Some(frame) = context.stack.last_mut() {
        frame.source_line = line_number;
    }
    if let Some(start) = context
        .stack
        .iter()
        .position(|frame| frame.canonical == canonical)
    {
        let chain: Vec<String> = context.stack[start..]
            .iter()
            .map(|frame| format!("{} line {}", frame.display, frame.source_line))
            .chain(std::iter::once(resolved.display().to_string()))
            .collect();
        return Err(miette!("Source cycle: {}", chain.join(" -> ")).into());
    }
    if context.stack.len() >= MAX_SOURCE_DEPTH {
        return Err(miette!(
            "{}: `Source` nesting is deeper than {MAX_SOURCE_DEPTH} tapes",
            at(context)
        )
        .into());
    }
    let source = std::fs::read_to_string(&resolved).map_err(|err| {
        miette!(
            "{}: failed to read {}: {err}",
            at(context),
            resolved.display()
        )
    })?;
    if source.is_empty() {
        return Err(miette!(
            "{}: sourced tape {} is empty",
            at(context),
            resolved.display()
        )
        .into());
    }

    let sourced_at = at(context);
    let display = resolved.display().to_string();
    let base_dir = std::mem::replace(
        &mut context.base_dir,
        resolved
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf),
    );
    context.stack.push(Frame {
        canonical,
        display: display.clone(),
        source_line: 0,
    });
    let result = parse_lines(&source, parsed, Some(context));
    context.stack.pop();
    context.base_dir = base_dir;
    result.map_err(|err| {
        let message = err.to_string();
        // Name the sourced tape on bare line errors; extend the chain on errors from deeper tapes.
        if message.starts_with("line ") {
            miette!("{display} {message} (sourced at {sourced_at})").into()
        } else if let Some(chain) = message
            .strip_suffix(')')
            .filter(|m| m.contains("(sourced at "))
        {
            miette!("{chain}, sourced at {sourced_at})").into()
        } else {
            err
        }
    })
}

/// Resolve a `Source` path against the sourcing tape's directory, then the current directory.
fn resolve_source_path(path: &Path, base_dir: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        return path.is_file().then(|| path.to_path_buf());
    }
    let relative_to_tape = base_dir.join(path);
    if relative_to_tape.is_file() {
        return Some(relative_to_tape);
    }
    path.is_file().then(|| path.to_path_buf())
}

/// Parse one tokenized tape line into one or more commands.
///
/// The cursor-based loop is what allows command chaining on a single line. Each branch is
/// responsible for consuming only the tokens that belong to that command; if it consumes too much,
/// the next command silently disappears, so the tests focus on chained-command examples.
fn parse_tokens(
    line_number: usize,
    tokens: &[String],
    commands: &mut Vec<Command>,
    warnings: &mut Vec<TapeWarningKind>,
) -> Result<()> {
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
                    value: if key == "LoopOffset" {
                        parse_loop_offset_value(value)
                    } else {
                        parse_value(value)
                    },
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
            "ScrollUp" | "ScrollDown" => {
                let count = optional_count(tokens.get(cursor))?;
                if count.is_some() {
                    cursor += 1;
                }
                let direction = if name == "ScrollUp" {
                    ViewportDirection::Up
                } else {
                    ViewportDirection::Down
                };
                commands.push(Command::ScrollViewport {
                    direction,
                    delay,
                    count: count.unwrap_or(1),
                });
            }
            "WindowTitle" => {
                if delay.is_some() {
                    return Err(miette!("WindowTitle does not accept a duration").into());
                }
                let text = required_token(line_number, tokens, cursor, "window title")?;
                commands.push(Command::WindowTitle(text.to_string()));
                cursor += 1;
            }
            "Caption" => {
                let text = required_token(line_number, tokens, cursor, "caption text")?;
                commands.push(Command::Caption(text.to_string()));
                cursor += 1;
            }
            "Type" => {
                let mut text =
                    required_token(line_number, tokens, cursor, "text to type")?.to_string();
                cursor += 1;
                // VHS joins several strings with one space: `Type "a" "b"` types `a b`.
                let mut strings = 1;
                while let Some(token) = tokens.get(cursor).filter(|token| !is_command_token(token))
                {
                    text.push(' ');
                    text.push_str(token);
                    strings += 1;
                    cursor += 1;
                }
                if strings > 1 {
                    warnings.push(TapeWarningKind::TypeJoinedStrings {
                        strings,
                        text: text.clone(),
                    });
                }
                commands.push(Command::Type { text, delay });
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
            _ if name.split('+').any(|part| part.starts_with("Mouse")) => {
                if delay.is_some() {
                    return Err(miette!("line {line_number}: mouse commands do not accept @duration; use Wait for synchronization").into());
                }
                let (event, modifiers, consumed) = parse_mouse(line_number, name, tokens, cursor)?;
                commands.push(Command::Mouse { event, modifiers });
                cursor += consumed;
            }
            "AssertText" | "AssertAbsent" | "AssertCells" | "AssertStyle" | "AssertState" => {
                let (assertion, consumed) =
                    parse_assertion(line_number, name, delay, tokens, cursor)?;
                commands.push(Command::Assert(assertion));
                cursor += consumed;
            }
            "Resize" => {
                if delay.is_some() {
                    return Err(miette!(
                        "line {line_number}: Resize does not accept @duration; use Wait for redraw"
                    )
                    .into());
                }
                let columns = positive_grid_number(
                    line_number,
                    required_token(line_number, tokens, cursor, "resize columns")?,
                )?;
                let rows = positive_grid_number(
                    line_number,
                    required_token(line_number, tokens, cursor + 1, "resize rows")?,
                )?;
                commands.push(Command::Resize { columns, rows });
                cursor += 2;
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

fn parse_assertion(
    line: usize,
    name: &str,
    delay: Option<Duration>,
    tokens: &[String],
    cursor: usize,
) -> Result<(Assertion, usize)> {
    if delay.is_some() && name != "AssertAbsent" {
        return Err(miette!("line {line}: only AssertAbsent accepts @duration").into());
    }
    let value = required_token(line, tokens, cursor, "assertion argument")?;
    match name {
        "AssertText" | "AssertAbsent" => {
            if value.is_empty() || delay.is_some_and(|duration| duration.is_zero()) {
                return Err(miette!("line {line}: text assertions require nonempty text and positive observation durations").into());
            }
            let assertion = if name == "AssertText" {
                Assertion::Text(value.to_owned())
            } else {
                Assertion::Absent {
                    text: value.to_owned(),
                    duration: delay,
                }
            };
            Ok((assertion, 1))
        }
        "AssertState" => Ok((Assertion::State(PathBuf::from(value)), 1)),
        _ => {
            let column = mouse_number(line, value, "column", false)?;
            let row = mouse_number(
                line,
                required_token(line, tokens, cursor + 1, "assertion row")?,
                "row",
                false,
            )?;
            let value = required_token(line, tokens, cursor + 2, "cell list or style width")?;
            if name == "AssertCells" {
                let text: Vec<String> = serde_json::from_str(value).map_err(|error| {
                    miette!("line {line}: expected JSON array of cell strings: {error}")
                })?;
                if text.is_empty() {
                    return Err(miette!("line {line}: expected at least one cell").into());
                }
                Ok((Assertion::Cells { column, row, text }, 3))
            } else {
                let width = positive_grid_number(line, value)?;
                let style = required_token(line, tokens, cursor + 3, "expected style JSON")?;
                let expected: crate::ghostty::StateStyle =
                    serde_json::from_str(style).map_err(|error| {
                        miette!("line {line}: invalid expected style JSON: {error}")
                    })?;
                for color in [&expected.fg, &expected.bg].into_iter().flatten() {
                    if color.len() != 7
                        || !color.starts_with('#')
                        || !color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
                    {
                        return Err(miette!(
                            "line {line}: expected #RRGGBB assertion color, got `{color}`"
                        )
                        .into());
                    }
                }
                if !matches!(
                    expected.underline.as_str(),
                    "" | "none" | "single" | "double" | "curly" | "dotted" | "dashed"
                ) {
                    return Err(miette!(
                        "line {line}: invalid underline style `{}`",
                        expected.underline
                    )
                    .into());
                }
                Ok((
                    Assertion::Style {
                        column,
                        row,
                        width,
                        expected,
                    },
                    4,
                ))
            }
        }
    }
}

fn positive_grid_number(line: usize, value: &str) -> Result<u16> {
    match value.parse::<u16>() {
        Ok(value) if value > 0 => Ok(value),
        _ => Err(miette!(
            "line {line}: invalid resize dimension `{value}`; expected integer in range 1..=65535"
        )
        .into()),
    }
}

fn parse_mouse(
    line: usize,
    name: &str,
    tokens: &[String],
    cursor: usize,
) -> Result<(MouseEvent, KeyModifiers, usize)> {
    let mut modifiers = KeyModifiers::default();
    let mut action = None;
    for part in name.split('+') {
        match part {
            "Alt" => modifiers.alt = true,
            "Ctrl" => modifiers.ctrl = true,
            "Shift" => modifiers.shift = true,
            "MouseMove" | "MouseDown" | "MouseUp" | "MouseScroll" if action.is_none() => {
                action = Some(part)
            }
            _ => return Err(miette!("line {line}: invalid mouse command `{name}`").into()),
        }
    }
    let value = required_token(line, tokens, cursor, "mouse argument")?;
    let (event, consumed) = match action {
        Some("MouseMove") => {
            let column = mouse_number(line, value, "column", false)?;
            let row = mouse_number(
                line,
                required_token(line, tokens, cursor + 1, "mouse row")?,
                "row",
                false,
            )?;
            (MouseEvent::Move { column, row }, 2)
        }
        Some("MouseDown" | "MouseUp") => {
            let button = match value {
                "Left" => MouseButton::Left,
                "Middle" => MouseButton::Middle,
                "Right" => MouseButton::Right,
                _ => {
                    return Err(miette!(
                        "line {line}: expected mouse button Left, Middle, or Right, got `{value}`"
                    )
                    .into())
                }
            };
            let event = if action == Some("MouseDown") {
                MouseEvent::Down(button)
            } else {
                MouseEvent::Up(button)
            };
            (event, 1)
        }
        Some("MouseScroll") => {
            let direction = match value {
                "Up" => ScrollDirection::Up,
                "Down" => ScrollDirection::Down,
                "Left" => ScrollDirection::Left,
                "Right" => ScrollDirection::Right,
                _ => {
                    return Err(miette!(
                    "line {line}: expected wheel direction Up, Down, Left, or Right, got `{value}`"
                )
                    .into())
                }
            };
            let count = mouse_number(
                line,
                required_token(line, tokens, cursor + 1, "wheel tick count")?,
                "wheel tick count",
                true,
            )?;
            (MouseEvent::Scroll { direction, count }, 2)
        }
        _ => return Err(miette!("line {line}: invalid mouse command `{name}`").into()),
    };
    Ok((event, modifiers, consumed))
}

fn mouse_number(line: usize, value: &str, label: &str, positive: bool) -> Result<u16> {
    match value.parse::<u16>() {
        Ok(value) if !positive || value > 0 => Ok(value),
        _ => Err(miette!(
            "line {line}: invalid mouse {label} `{value}`; expected {}integer in range {}..=65535",
            if positive { "positive " } else { "" },
            if positive { 1 } else { 0 }
        )
        .into()),
    }
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

/// Parse a `LoopOffset` value, where VHS reads bare numbers as percentages.
///
/// VHS appends `%` to every `LoopOffset` value, so `20` and `20%` both mean 20 percent, and `0.5`
/// means half a percent. The number is kept unnormalized so the runner can tell it apart from a
/// duration such as `2s`, which Betamax accepts as an extension.
fn parse_loop_offset_value(value: &str) -> Value {
    let number = value.strip_suffix('%').unwrap_or(value);
    match number.parse::<f64>() {
        Ok(percent) => Value::Number(percent),
        Err(_) => parse_value(value),
    }
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
/// (`m`). Zero is allowed. Negative durations are rejected because they would make execution timing
/// ambiguous.
///
/// # Errors
///
/// Returns an error when the value is not numeric, uses an unsupported suffix, or is negative.
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
    Ok(Duration::from_secs_f64(parse_duration_number(
        value, value,
    )?))
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
    if number.is_sign_negative() || !number.is_finite() {
        return Err(miette!("duration must be zero or positive: `{original}`").into());
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
            | "ScrollUp"
            | "ScrollDown"
            | "Caption"
            | "WindowTitle"
            | "Source"
            | "Screenshot"
            | "State"
            | "Resize"
            | "AssertText"
            | "AssertAbsent"
            | "AssertCells"
            | "AssertStyle"
            | "AssertState"
    ) || name.split('+').any(|part| part.starts_with("Mouse"))
        || name == "Wait"
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

/// Warn about startup commands that follow runtime commands.
///
/// VHS accepts these, so Betamax does too. [`Tape::header_len`] and [`Tape::startup_commands`]
/// define what each one does; this only reports them.
fn late_startup_warnings(tape: &Tape) -> Vec<(usize, TapeWarning)> {
    let header_len = tape.header_len();
    tape.commands
        .iter()
        .zip(&tape.locations)
        .enumerate()
        .skip(header_len)
        .filter_map(|(index, (command, location))| {
            let kind = match command {
                Command::Set { key, .. } if key == "TypingSpeed" => return None,
                Command::Set { key, .. } => TapeWarningKind::LateSetting {
                    key: key.clone(),
                    effect: if key == "Shell" {
                        LateSettingEffect::WholeTape
                    } else {
                        LateSettingEffect::Ignored
                    },
                },
                Command::Env { key, .. } => TapeWarningKind::LateEnv { key: key.clone() },
                Command::Require(program) => TapeWarningKind::LateRequire {
                    program: program.clone(),
                },
                _ => return None,
            };
            let warning = TapeWarning {
                location: location.clone(),
                kind,
            };
            Some((index, warning))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_runtime_window_titles_without_setting_type_inference() {
        let tape = Tape::parse(
            r#"Set WindowTitle initial
Sleep 1ms WindowTitle "123" WindowTitle "" WindowTitle "title reset by tape"
Wait+Screen ready WindowTitle final"#,
        )
        .unwrap();
        assert!(tape.commands.contains(&Command::WindowTitle("123".into())));
        assert!(tape.commands.contains(&Command::WindowTitle(String::new())));
        assert_eq!(
            tape.commands.last(),
            Some(&Command::WindowTitle("final".into()))
        );
        for source in ["WindowTitle", "WindowTitle@1s title"] {
            assert!(Tape::parse(source).is_err(), "{source}");
        }
        let (_, warnings) = parse_warnings("WindowTitle title Set WindowTitle late");
        assert!(matches!(
            warnings[..],
            [TapeWarning {
                kind: TapeWarningKind::LateSetting {
                    effect: LateSettingEffect::Ignored,
                    ..
                },
                ..
            }]
        ));
    }

    #[test]
    fn parses_chained_assertions_and_validates_json_with_line_context() {
        let tape = Tape::parse(r#"AssertText "ready" AssertCells 0 1 '["界", " "]' AssertStyle 0 1 2 '{"bold":true}' AssertAbsent@100ms "error" AssertState baseline.json"#).unwrap();
        assert_eq!(tape.commands.len(), 5);
        for source in [
            r#"AssertText """#,
            "AssertText@1s ready",
            "AssertCells 0 0 []",
            "AssertCells 0 0 [1]",
            r#"AssertStyle 0 0 1 '{"unknown":true}'"#,
            r#"AssertStyle 0 0 1 '{"fg":"red"}'"#,
            "AssertAbsent@0ms error",
            "AssertStyle 0 0 0 {}",
        ] {
            assert!(
                Tape::parse(&format!("# heading\n{source}"))
                    .unwrap_err()
                    .to_string()
                    .contains("line 2"),
                "{source}"
            );
        }
    }

    #[test]
    fn parses_runtime_resize_and_rejects_invalid_dimensions() {
        let tape = Tape::parse("Wait Resize 50 16 MouseMove 49 15").unwrap();
        assert!(matches!(
            tape.commands[1],
            Command::Resize {
                columns: 50,
                rows: 16
            }
        ));
        for source in [
            "Resize 0 1",
            "Resize 1 0",
            "Resize -1 1",
            "Resize 65536 1",
            "Resize 1",
            "Resize@1s 50 16",
        ] {
            assert!(Tape::parse(&format!("# heading\n{source}"))
                .unwrap_err()
                .to_string()
                .contains("line 2"));
        }
    }

    #[test]
    fn parses_vhs_viewport_scroll_forms() {
        let tape = Tape::parse("ScrollUp\nScrollUp 5\nScrollDown@100ms 3 ScrollDown").unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::ScrollViewport {
                    direction: ViewportDirection::Up,
                    delay: None,
                    count: 1,
                },
                Command::ScrollViewport {
                    direction: ViewportDirection::Up,
                    delay: None,
                    count: 5,
                },
                Command::ScrollViewport {
                    direction: ViewportDirection::Down,
                    delay: Some(Duration::from_millis(100)),
                    count: 3,
                },
                Command::ScrollViewport {
                    direction: ViewportDirection::Down,
                    delay: None,
                    count: 1,
                },
            ]
        );
        assert!(Tape::parse("ScrollUp 70000").is_err());
    }

    #[test]
    fn parses_chained_mouse_actions_and_modifiers() {
        let tape = Tape::parse("Wait MouseMove 0 1 Shift+Ctrl+MouseDown Left MouseMove 4 1 MouseUp Left MouseScroll Down 3").unwrap();
        assert_eq!(tape.commands.len(), 6);
        assert!(matches!(
            tape.commands[1],
            Command::Mouse {
                event: MouseEvent::Move { column: 0, row: 1 },
                ..
            }
        ));
        assert!(matches!(
            tape.commands[2],
            Command::Mouse {
                modifiers: KeyModifiers {
                    shift: true,
                    ctrl: true,
                    alt: false
                },
                ..
            }
        ));
        assert!(matches!(
            tape.commands[5],
            Command::Mouse {
                event: MouseEvent::Scroll {
                    direction: ScrollDirection::Down,
                    count: 3
                },
                ..
            }
        ));
    }

    #[test]
    fn rejects_invalid_mouse_arguments_with_line_context() {
        for source in [
            "MouseMove -1 0",
            "MouseMove 0",
            "MouseMove 65536 0",
            "MouseDown Nope",
            "MouseScroll Down 0",
            "MouseScroll Down",
            "MouseScroll Nope 1",
            "Meta+MouseDown Left",
            "MouseMove@1s 0 0",
        ] {
            let error = Tape::parse(&format!("# heading\n{source}")).unwrap_err();
            assert!(error.to_string().contains("line 2"), "{source}: {error}");
        }
    }

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
    fn parses_bare_speed_override_as_seconds() {
        let tape = Tape::parse(r#"Type@0.1 "x" Sleep 0"#).unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::Type {
                    text: "x".to_string(),
                    delay: Some(Duration::from_millis(100)),
                },
                Command::Sleep(Duration::ZERO),
            ]
        );
    }

    #[test]
    fn rejects_negative_and_non_finite_durations() {
        for source in [r#"Type@-1 "x""#, "Sleep -0.5", "Sleep inf"] {
            let error = Tape::parse(source).unwrap_err().to_string();
            assert!(
                error.contains("must be zero or positive"),
                "{source:?}: {error}"
            );
        }
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
        let tape =
            Tape::parse(r#"Copy "hello" Paste Caption "Ready" Ctrl+Alt+C F5 Shift+Tab"#).unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::Copy("hello".to_string()),
                Command::Paste,
                Command::Caption("Ready".to_string()),
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

    /// Parse without a tape file and collect warnings.
    fn parse_warnings(source: &str) -> (Tape, Vec<TapeWarning>) {
        let mut warnings = Vec::new();
        let tape =
            Tape::parse_with_sources(source, None, &mut |warning| warnings.push(warning)).unwrap();
        (tape, warnings)
    }

    fn late(location: &str, kind: TapeWarningKind) -> TapeWarning {
        TapeWarning {
            location: location.to_string(),
            kind,
        }
    }

    #[test]
    fn warns_about_startup_commands_after_runtime_commands() {
        let (tape, warnings) = parse_warnings(
            "Set Width 800\nType hi\nSet TypingSpeed 10ms\nSet Theme Dracula\nSet Shell zsh\nEnv FOO bar\nRequire git",
        );
        assert_eq!(tape.header_len(), 1);
        assert_eq!(
            warnings,
            [
                late(
                    "line 4",
                    TapeWarningKind::LateSetting {
                        key: "Theme".into(),
                        effect: LateSettingEffect::Ignored,
                    }
                ),
                late(
                    "line 5",
                    TapeWarningKind::LateSetting {
                        key: "Shell".into(),
                        effect: LateSettingEffect::WholeTape,
                    }
                ),
                late("line 6", TapeWarningKind::LateEnv { key: "FOO".into() }),
                late(
                    "line 7",
                    TapeWarningKind::LateRequire {
                        program: "git".into(),
                    }
                ),
            ]
        );
    }

    #[test]
    fn header_env_and_require_do_not_warn() {
        let (_, warnings) = parse_warnings("Env FOO bar\nSet Width 800\nRequire git\nType hi");
        assert!(warnings.is_empty());
    }

    #[test]
    fn joins_several_type_strings_with_a_space() {
        let (tape, warnings) = parse_warnings("Type@10ms \"echo\" \"hi there\" Enter");
        assert_eq!(
            tape.commands[0],
            Command::Type {
                text: "echo hi there".to_string(),
                delay: Some(Duration::from_millis(10)),
            }
        );
        assert!(matches!(tape.commands[1], Command::Key { .. }));
        assert_eq!(
            warnings,
            [late(
                "line 1",
                TapeWarningKind::TypeJoinedStrings {
                    strings: 2,
                    text: "echo hi there".to_string(),
                }
            )]
        );
    }

    #[test]
    fn keeps_caption_as_command_after_wait_without_pattern() {
        let tape = Tape::parse(r#"Wait+Screen Caption "Loaded""#).unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::Wait {
                    target: WaitTarget::Screen,
                    pattern: None,
                    timeout: None,
                },
                Command::Caption("Loaded".to_string()),
            ]
        );
    }

    #[test]
    fn parses_empty_caption_for_clear() {
        let tape = Tape::parse(r#"Caption "" Caption "Next""#).unwrap();
        assert_eq!(
            tape.commands,
            vec![
                Command::Caption(String::new()),
                Command::Caption("Next".to_string()),
            ]
        );
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

    /// Write `files` into a fresh temporary directory and return its path.
    fn tape_dir(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("betamax-source-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, contents) in files {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        dir
    }

    fn parse_dir(dir: &Path, warnings: &mut Vec<TapeWarning>) -> Result<Tape> {
        Tape::parse_file(&dir.join("main.tape"), &mut |warning| {
            warnings.push(warning)
        })
    }

    #[test]
    fn source_inlines_header_settings_and_drops_output() {
        let dir = tape_dir(
            "inline",
            &[
                (
                    "main.tape",
                    "Output demo.gif\nSource lib/setup.tape\nType \"hi\"",
                ),
                (
                    "lib/setup.tape",
                    "Output setup.gif\nSet Width 640\nRequire git",
                ),
            ],
        );
        let mut warnings = Vec::new();
        let tape = parse_dir(&dir, &mut warnings).unwrap();
        assert_eq!(
            tape.commands,
            [
                Command::Output("demo.gif".into()),
                Command::Set {
                    key: "Width".into(),
                    value: Value::Number(640.0)
                },
                Command::Require("git".into()),
                Command::Type {
                    text: "hi".into(),
                    delay: None
                },
            ]
        );
        let setup = dir.join("lib/setup.tape");
        let main = dir.join("main.tape");
        assert_eq!(
            tape.locations,
            [
                "line 1".to_string(),
                format!(
                    "{} line 2, sourced at {} line 2",
                    setup.display(),
                    main.display()
                ),
                format!(
                    "{} line 3, sourced at {} line 2",
                    setup.display(),
                    main.display()
                ),
                "line 3".to_string(),
            ]
        );
        assert_eq!(warnings.len(), 1);
        let warning = warnings[0].to_string();
        assert!(warning.contains("Output setup.gif"), "{warning}");
        assert!(
            warning.contains("setup.tape line 1, sourced at"),
            "{warning}"
        );
    }

    #[test]
    fn source_resolves_nested_paths_relative_to_each_tape() {
        let dir = tape_dir(
            "nested",
            &[
                ("main.tape", "Source a/one.tape"),
                ("a/one.tape", "Source b/two.tape\nType \"one\""),
                ("a/b/two.tape", "Type \"two\""),
            ],
        );
        let tape = parse_dir(&dir, &mut Vec::new()).unwrap();
        assert_eq!(tape.commands.len(), 2);
        assert!(matches!(&tape.commands[0], Command::Type { text, .. } if text == "two"));
    }

    #[test]
    fn source_falls_back_to_the_current_directory() {
        // Cargo runs unit tests from the package root, and the empty directory has no `src/`.
        let dir = tape_dir("cwd", &[("main.tape", "")]);
        let path = Path::new("src/tape/parser.rs");
        assert_eq!(resolve_source_path(path, &dir), Some(path.to_path_buf()));
        assert_eq!(
            resolve_source_path(Path::new("main.tape"), &dir),
            Some(dir.join("main.tape"))
        );
    }

    #[test]
    fn source_after_runtime_commands_warns_with_the_setting_origin() {
        let dir = tape_dir(
            "order",
            &[
                ("main.tape", "Type \"x\"\nSource setup.tape"),
                ("setup.tape", "\nSet Width 640"),
            ],
        );
        let mut warnings = Vec::new();
        parse_dir(&dir, &mut warnings).unwrap();
        assert_eq!(warnings.len(), 1);
        let warning = warnings[0].to_string();
        assert!(
            warning.contains("`Set Width` after runtime commands is ignored"),
            "{warning}"
        );
        assert!(
            warning.contains("setup.tape line 2, sourced at "),
            "{warning}"
        );
        assert!(warning.contains("main.tape line 2: "), "{warning}");
    }

    #[test]
    fn source_errors_point_into_the_sourced_tape() {
        let dir = tape_dir(
            "inner-error",
            &[
                ("main.tape", "\n\nSource setup.tape"),
                ("setup.tape", "Type \"ok\"\nBogus"),
            ],
        );
        let error = parse_dir(&dir, &mut Vec::new()).unwrap_err().to_string();
        assert!(
            error.contains("setup.tape line 2: unsupported tape command `Bogus`"),
            "{error}"
        );
        assert!(
            error.contains("(sourced at ") && error.contains("main.tape line 3)"),
            "{error}"
        );
    }

    #[test]
    fn source_errors_list_every_sourcing_tape() {
        let dir = tape_dir(
            "inner-chain",
            &[
                ("main.tape", "Source a.tape"),
                ("a.tape", "\nSource b.tape"),
                ("b.tape", "Bogus"),
            ],
        );
        let error = parse_dir(&dir, &mut Vec::new()).unwrap_err().to_string();
        assert!(error.contains("b.tape line 1: unsupported"), "{error}");
        assert!(error.contains("a.tape line 2, sourced at "), "{error}");
        assert!(error.ends_with("main.tape line 1)"), "{error}");
    }

    #[test]
    fn source_rejects_cycles_missing_empty_and_non_tape_files() {
        let cases = [
            (
                "cycle",
                vec![
                    ("main.tape", "Source a.tape"),
                    ("a.tape", "Source main.tape"),
                ],
                "Source cycle: ",
            ),
            (
                "self",
                vec![("main.tape", "Source main.tape")],
                "Source cycle: ",
            ),
            (
                "missing",
                vec![("main.tape", "Source nope.tape")],
                "not found",
            ),
            (
                "empty",
                vec![("main.tape", "Source empty.tape"), ("empty.tape", "")],
                "is empty",
            ),
            (
                "ext",
                vec![
                    ("main.tape", "Source notes.txt"),
                    ("notes.txt", "Type \"x\""),
                ],
                ".tape extension",
            ),
        ];
        for (name, files, expected) in cases {
            let dir = tape_dir(name, &files);
            let error = parse_dir(&dir, &mut Vec::new()).unwrap_err().to_string();
            assert!(error.contains(expected), "{name}: {error}");
        }
        let dir = tape_dir(
            "cycle-chain",
            &[
                ("main.tape", "Source a.tape"),
                ("a.tape", "\nSource main.tape"),
            ],
        );
        let error = parse_dir(&dir, &mut Vec::new()).unwrap_err().to_string();
        assert!(error.contains("main.tape line 1 -> "), "{error}");
        assert!(error.contains("a.tape line 2 -> "), "{error}");
    }

    #[test]
    fn source_without_a_file_context_is_an_error() {
        let error = Tape::parse("Source other.tape").unwrap_err().to_string();
        assert!(error.contains("Tape::parse_file"), "{error}");
    }
}
