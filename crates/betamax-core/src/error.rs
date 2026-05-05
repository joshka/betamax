//! Crate-owned error and result types.
//!
//! Betamax uses `miette` internally for diagnostic construction because tape parsing, process
//! execution, and media writing benefit from contextual messages. Public APIs expose this small
//! wrapper instead of `miette::Report` directly so the crate can evolve its diagnostic backend
//! without making every caller depend on that concrete error type.

use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

/// Error returned by `betamax-core`.
///
/// The current implementation stores a `miette::Report` so callers still receive the detailed
/// diagnostics produced throughout the parser, runner, renderer, and media writers. Treat the
/// textual representation as user-facing diagnostic text, not as a stable machine-readable format.
#[derive(Debug)]
pub struct Error {
    /// Rich diagnostic report produced by the subsystem that failed.
    report: miette::Report,
}

impl From<miette::Report> for Error {
    fn from(report: miette::Report) -> Self {
        Self { report }
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.report, formatter)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.report.source()
    }
}

impl miette::Diagnostic for Error {}

/// Result type returned by `betamax-core`.
pub type Result<T, E = Error> = std::result::Result<T, E>;
