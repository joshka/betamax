//! VHS-compatible plain-text outputs (`Output demo.txt` / `Output demo.ascii`).
//!
//! VHS appends the visible screen to the file after every command it executes while recording,
//! then a separator line. Betamax matches that byte format so VHS golden files compare directly,
//! with two deliberate differences: it dumps the visible viewport (VHS reads the oldest scrollback
//! rows once output scrolls) and it skips the tape header, whose VHS snapshots depend on browser
//! resize timing.

use std::fs;
use std::path::PathBuf;

use miette::IntoDiagnostic;

use super::TerminalSession;
use crate::media::ensure_parent_dir;
use crate::tape::Command;
use crate::Result;

/// The line VHS writes after each snapshot: 80 box-drawing horizontals.
const SEPARATOR: &str =
    "────────────────────────────────────────────────────────────────────────────────";

/// Accumulated text snapshots for one run, written to every text output at the end.
#[derive(Debug, Default)]
pub(super) struct TextOutput {
    paths: Vec<PathBuf>,
    text: String,
}

impl TextOutput {
    pub(super) fn new(paths: Vec<PathBuf>) -> Self {
        Self {
            paths,
            text: String::new(),
        }
    }

    /// Append a snapshot after `command` ran, if the command is one VHS snapshots.
    ///
    /// `Set`, `Output`, and `Require` are configuration, not recorded commands, so they never
    /// produce snapshots. `Hide` and hidden commands are skipped through `visible`.
    pub(super) fn after_command(
        &mut self,
        command: &Command,
        visible: bool,
        terminal: &mut impl TerminalSession,
    ) -> Result<()> {
        if self.paths.is_empty()
            || !visible
            || matches!(
                command,
                Command::Set { .. } | Command::Output(_) | Command::Require(_)
            )
        {
            return Ok(());
        }
        push_snapshot(&mut self.text, &terminal.viewport_lines()?);
        Ok(())
    }

    /// Write the accumulated snapshots to every requested path, returning the written paths.
    pub(super) fn write(self) -> Result<Vec<PathBuf>> {
        for path in &self.paths {
            ensure_parent_dir(path)?;
            fs::write(path, &self.text).into_diagnostic()?;
        }
        Ok(self.paths)
    }
}

fn push_snapshot(text: &mut String, lines: &[String]) {
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text.push_str(SEPARATOR);
    text.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_keeps_blank_rows_and_ends_with_separator() {
        let mut text = String::new();
        push_snapshot(&mut text, &["> ls".into(), String::new(), "你好".into()]);

        assert_eq!(text, format!("> ls\n\n你好\n{SEPARATOR}\n"));
        assert_eq!(SEPARATOR.chars().count(), 80);
    }
}
