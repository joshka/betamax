//! Advisory diagnostics for parsed tapes.
//!
//! Warnings flag VHS tape patterns that Betamax accepts for compatibility but that do not do what
//! their position suggests. They never change parse success.

use std::fmt;
use std::path::PathBuf;

/// A non-fatal warning about tape authoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapeWarning {
    /// Where the command came from: `line 4`, or for a sourced tape,
    /// `setup.tape line 2, sourced at demo.tape line 1`.
    pub location: String,
    /// Warning category and structured warning data.
    pub kind: TapeWarningKind,
}

/// Category and structured data for a tape warning.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TapeWarningKind {
    /// A `Set` other than `TypingSpeed` follows a runtime command.
    LateSetting {
        /// Setting name.
        key: String,
        /// What Betamax does with the setting.
        effect: LateSettingEffect,
    },
    /// An `Env` follows a runtime command; it still applies from shell start.
    LateEnv {
        /// Environment variable name.
        key: String,
    },
    /// A `Require` follows a runtime command; it is still checked before the shell starts.
    LateRequire {
        /// Required program.
        program: String,
    },
    /// An `Output` in a sourced tape is dropped so it cannot replace the parent's outputs.
    SourcedOutputIgnored {
        /// Output path from the sourced tape.
        path: PathBuf,
    },
    /// A `Type` command has several strings, which are joined with one space.
    TypeJoinedStrings {
        /// Number of strings on the command.
        strings: usize,
        /// The joined text that is typed.
        text: String,
    },
}

/// What Betamax does with a `Set` that follows runtime commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateSettingEffect {
    /// The setting is ignored, as in VHS.
    Ignored,
    /// The setting applies to the whole tape, as `Set Shell` does in VHS.
    WholeTape,
}

impl fmt::Display for TapeWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.location)?;
        match &self.kind {
            TapeWarningKind::LateSetting {
                key,
                effect: LateSettingEffect::Ignored,
            } => write!(
                f,
                "`Set {key}` after runtime commands is ignored; move it above the first runtime command"
            ),
            TapeWarningKind::LateSetting {
                key,
                effect: LateSettingEffect::WholeTape,
            } => write!(
                f,
                "`Set {key}` after runtime commands applies to the whole tape; move it above the first runtime command"
            ),
            TapeWarningKind::LateEnv { key } => write!(
                f,
                "`Env {key}` after runtime commands applies from shell start; move it above the first runtime command"
            ),
            TapeWarningKind::LateRequire { program } => write!(
                f,
                "`Require {program}` after runtime commands is checked before the shell starts; move it above the first runtime command"
            ),
            TapeWarningKind::SourcedOutputIgnored { path } => write!(
                f,
                "ignoring `Output {}` from sourced tape",
                path.display()
            ),
            TapeWarningKind::TypeJoinedStrings { strings, text } => write!(
                f,
                "`Type` with {strings} strings types them joined by spaces; write one string: `Type {text:?}`"
            ),
        }
    }
}
