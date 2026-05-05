//! Preflight checks for tape-level requirements.

use std::env;
use std::path::PathBuf;

use miette::miette;

use crate::tape::{Command, Tape};
use crate::Result;

/// Validate all `Require` commands before any PTY side effects occur.
pub(super) fn validate_required_programs(tape: &Tape) -> Result<()> {
    for command in &tape.commands {
        if let Command::Require(program) = command {
            if find_on_path(program).is_none() {
                return Err(miette!("required program was not found on PATH: {program}").into());
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
