use std::path::PathBuf;
use std::time::Duration;

use crate::Result;

/// Parsed tape source.
///
/// A tape is an ordered command stream. It preserves configuration, environment, output, and
/// runtime commands in one vector because ordering matters for validation and execution. Use
/// [`Tape::parse`](super::Tape::parse) for source text and [`Tape::add_output`] when a caller
/// wants to append an additional primary output path outside the tape file.
#[derive(Debug, Clone, PartialEq)]
pub struct Tape {
    /// Parsed commands in execution order.
    ///
    /// Configuration commands such as [`Command::Set`], [`Command::Env`], [`Command::Require`],
    /// and [`Command::Output`] are kept in this same stream because they are ordered in the input
    /// language. [`Tape::parse`](super::Tape::parse) validates that settings which affect process
    /// startup appear before runtime commands.
    pub commands: Vec<Command>,
}

impl Tape {
    /// Parse a VHS-style tape source string into commands.
    ///
    /// Tokenization uses `shell_words`, so quoted strings and backslash escaping follow shell-like
    /// rules rather than a bespoke parser. Blank lines and lines whose trimmed form starts with
    /// `#` are ignored. A single physical line may contain multiple commands, for example
    /// `Type "echo hi" Enter Sleep 1s`.
    ///
    /// This parser validates syntax, known command names, duration literals, regex literals, and
    /// the ordering rule that startup-affecting commands cannot appear after runtime commands. It
    /// deliberately does not validate executable availability, output extension support, theme
    /// names, or whether a parsed command has an implementation in the runner.
    ///
    /// # Errors
    ///
    /// Returns an error when tokenization fails, a command is unknown, a required argument is
    /// missing, a duration or regex is invalid, or startup-affecting commands appear after runtime
    /// commands.
    pub fn parse(source: &str) -> Result<Self> {
        super::parser::parse_tape(source)
    }

    /// Iterate over top-level `Output` paths in source order.
    ///
    /// Inline `Screenshot` and `State` paths are not returned here because they are command outputs
    /// rather than the primary tape outputs used by CLI defaults.
    pub fn outputs(&self) -> impl Iterator<Item = &PathBuf> {
        self.commands.iter().filter_map(|command| match command {
            Command::Output(path) => Some(path),
            _ => None,
        })
    }

    /// Append a primary output path.
    ///
    /// The CLI uses this when a user supplies an output path outside the tape file. Appending keeps
    /// the operation simple and preserves the same behavior as writing an `Output` command at the
    /// end of the configuration block.
    pub fn add_output(&mut self, path: PathBuf) {
        self.commands.push(Command::Output(path));
    }
}

/// Parsed tape command.
///
/// The enum is intentionally close to the textual tape language. Some variants, such as
/// [`Command::Source`], are parsed for language compatibility even though the current runner
/// returns a targeted "not implemented" error for them. Use [`Tape::parse`] to construct commands
/// from source text and [`crate::Runner`] to execute them.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Primary output requested by the tape or CLI.
    ///
    /// The runner classifies this by extension: `.gif`, `.png`, `.json`, `.webm`, `.mp4`, or an
    /// extensionless directory for a PNG frame sequence.
    Output(PathBuf),
    /// Require an executable to be present on `PATH` before running the tape.
    Require(String),
    /// Set a runner option such as dimensions, typography, theme, or timing.
    ///
    /// The runner rejects unknown settings and values with mismatched types before starting the
    /// shell.
    Set {
        /// Setting name exactly as written after `Set`.
        key: String,
        /// Parsed setting value. The parser infers primitive types before falling back to string.
        value: Value,
    },
    /// Pause execution for a fixed duration.
    Sleep(Duration),
    /// Type Unicode text into the PTY one scalar value at a time.
    Type {
        /// Text to write to the PTY.
        text: String,
        /// Optional per-character delay from `Type@duration`; runner defaults apply when absent.
        delay: Option<Duration>,
    },
    /// Wait until terminal text matches a pattern.
    ///
    /// Waits require capture because matching is performed against libghostty-vt terminal state,
    /// not raw PTY bytes.
    Wait {
        /// Text scope to inspect while waiting.
        target: WaitTarget,
        /// Optional command-specific pattern. When absent, the runner's default wait pattern is
        /// used, currently a VHS-style prompt regex.
        pattern: Option<WaitPattern>,
        /// Optional timeout from `Wait@duration`; runner defaults apply when absent.
        timeout: Option<Duration>,
    },
    /// Send a named key press, optionally with modifiers and repeat count.
    Key {
        /// Key identity and modifiers.
        key: Key,
        /// Optional delay after each press.
        delay: Option<Duration>,
        /// Repeat count parsed from the token after the key name; defaults to one.
        count: u16,
    },
    /// Stop recording frames while continuing to execute and update terminal state.
    Hide,
    /// Resume recording and immediately capture the current terminal frame.
    Show,
    /// Set an environment variable for the spawned PTY process.
    Env {
        /// Environment variable name.
        key: String,
        /// Environment variable value.
        value: String,
    },
    /// Store text in the tape-local clipboard.
    Copy(String),
    /// Write the tape-local clipboard into the PTY.
    Paste,
    /// Set presentation text rendered on later media frames.
    ///
    /// Captions are not PTY input and do not participate in waits. An empty string clears the
    /// active caption.
    Caption(String),
    /// Parsed placeholder for VHS `Source`; execution is intentionally not implemented yet.
    Source(PathBuf),
    /// Capture an immediate PNG screenshot without changing the primary outputs.
    Screenshot(PathBuf),
    /// Write an immediate JSON state snapshot without changing the primary outputs.
    State(PathBuf),
}

/// Parsed primitive value for a `Set` command.
///
/// Values are inferred by the parser before the runner knows which setting is being applied. That
/// means a syntactically valid [`Value`] can still be rejected later if it is not the kind expected
/// by the target setting. See the repository tape reference for setting names, accepted value
/// kinds, and defaults.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A string value, usually from a quoted tape token or a value that did not parse as a
    /// primitive.
    String(String),
    /// A numeric value, including percentage values normalized to `0.0..=1.0`.
    Number(f64),
    /// A duration literal such as `500ms`, `1s`, or `2 minutes`.
    Duration(Duration),
    /// A boolean literal accepted by Rust's `bool` parser.
    Bool(bool),
}

/// Parsed key command.
///
/// Betamax models only key presses because terminal input ultimately becomes byte sequences sent
/// to the PTY. Key release events, scan codes, and platform keyboard layouts are outside the tape
/// language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// A key press event. Release/repeat events are not represented because the tape language only
    /// needs terminal input bytes.
    Press {
        /// Logical key code.
        key: KeyCode,
        /// Keyboard modifiers to encode with the key.
        modifiers: KeyModifiers,
    },
}

/// Logical key code accepted by the tape parser.
///
/// These are mapped to terminal escape sequences by Betamax's key encoder. Printable text should
/// usually use [`Command::Type`] instead; [`KeyCode::Char`] is for one-character key commands such
/// as `Ctrl+C`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// Backspace key.
    Backspace,
    /// Delete key.
    Delete,
    /// Down arrow key.
    Down,
    /// End key.
    End,
    /// Enter/return key.
    Enter,
    /// Escape key.
    Escape,
    /// Function key number. The parser accepts F1 through F25.
    Function(u8),
    /// Home key.
    Home,
    /// Insert key.
    Insert,
    /// Left arrow key.
    Left,
    /// Page Down key.
    PageDown,
    /// Page Up key.
    PageUp,
    /// Right arrow key.
    Right,
    /// Space key.
    Space,
    /// Tab key.
    Tab,
    /// Up arrow key.
    Up,
    /// A single Unicode scalar from a one-character key token.
    Char(char),
}

/// Modifier keys attached to a key press.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    /// Whether the `Alt` modifier was present.
    pub alt: bool,
    /// Whether the `Ctrl` modifier was present.
    pub ctrl: bool,
    /// Whether the `Shift` modifier was present.
    pub shift: bool,
}

/// Terminal text region inspected by a wait command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitTarget {
    /// Match only the cursor's current viewport line.
    Line,
    /// Match all visible screen text.
    Screen,
}

/// Pattern used by a wait command.
///
/// Patterns are matched by the runner against either the current cursor line or the visible screen,
/// depending on [`WaitTarget`]. Regex strings are stored without surrounding slash delimiters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitPattern {
    /// Plain substring match.
    Contains(String),
    /// Regular expression match without surrounding slash delimiters.
    Regex(String),
}
