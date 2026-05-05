//! Options supplied by runner callers.

/// Configures behavior outside the tape file.
///
/// These options come from the embedding application rather than the tape source. Tape settings
/// such as dimensions, theme, timing, and shell are parsed from the tape itself.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions {
    /// Whether to publish the generated output.
    ///
    /// This option exists to keep the public API compatible with the VHS-shaped CLI surface, but
    /// publishing is intentionally not implemented in the current Ghostty-first runner.
    pub publish: bool,
    /// Suppress informational logs from successful no-capture execution.
    ///
    /// Errors are still returned normally. Capture runs currently do not emit routine progress
    /// logs.
    pub quiet: bool,
}
