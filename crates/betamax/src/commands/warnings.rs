//! Shared CLI warning presentation.

use std::fmt::Display;

use betamax_core::tape::TapeWarning;

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_YELLOW: &str = "\x1b[33m";

/// Print one advisory tape warning to stderr as `warning: <tape>: <location>: <message>`.
pub(super) fn print_tape_warning(tape: impl Display, warning: &TapeWarning) {
    eprintln!("{ANSI_YELLOW}warning{ANSI_RESET}: {tape}: {warning}");
}
