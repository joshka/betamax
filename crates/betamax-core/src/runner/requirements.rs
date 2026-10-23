//! Preflight checks for tape-level requirements.

use std::env;
use std::path::PathBuf;

use miette::miette;

use crate::tape::{Command, Tape};

/// Validate all `Require` commands, wherever they appear, before any PTY side effects occur.
///
/// # Errors
///
/// Returns the index of the first `Require` whose program is missing, with its error.
pub(super) fn validate_required_programs(tape: &Tape) -> Result<(), (usize, crate::Error)> {
    for (index, command) in tape.commands.iter().enumerate() {
        if let Command::Require(program) = command {
            if find_on_path(program).is_none() {
                let error = miette!("required program was not found on PATH: {program}");
                return Err((index, error.into()));
            }
        }
    }
    Ok(())
}

/// Search `PATH` for an executable-looking file.
///
/// This intentionally checks only `is_file`; platform-specific executable-bit semantics are left to
/// the eventual process spawn, matching the loose behavior expected from tape preflight checks.
fn find_on_path(program: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|path| path.join(program))
        .find(|path| path.is_file())
}
