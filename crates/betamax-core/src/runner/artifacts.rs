//! Return values from completed runner executions.

use std::path::PathBuf;

use crate::ghostty::TerminalState;

/// Artifacts and terminal state returned from a completed run.
///
/// File contents are written before this value is returned. The value intentionally stores paths
/// and final terminal state rather than large media buffers so callers can decide how much data to
/// keep in memory.
#[derive(Debug, Clone, Default)]
pub struct RunArtifacts {
    /// Final terminal state when the run used capture.
    ///
    /// This is `None` for no-capture runs because Betamax did not instantiate libghostty-vt and
    /// therefore cannot accurately report viewport text, scrollback, styles, or cursor state.
    pub final_state: Option<TerminalState>,
    /// Primary output paths written by the run, in deterministic output-classification order.
    ///
    /// Inline `Screenshot` and `State` commands write files as side effects but are not included
    /// here because they are command checkpoints rather than top-level tape outputs.
    pub output_paths: Vec<PathBuf>,
}
