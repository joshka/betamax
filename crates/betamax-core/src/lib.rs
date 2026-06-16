#![warn(missing_docs)]
#![cfg_attr(
    windows,
    doc = "This crate is not supported on Windows because libghostty-vt-sys does not support Windows builds."
)]

//! Core library for Betamax terminal capture.
//!
//! `betamax-core` is the reusable engine behind the `betamax` command-line
//! application. It parses VHS-style tape source, executes commands in a PTY,
//! feeds terminal output into `libghostty-vt`, renders terminal frames, writes
//! media artifacts, and can return structured terminal state for snapshot tests.
//!
//! Most applications should start with [`Tape`] and [`Runner`]:
//!
//! The full tape language is documented on the
//! [Betamax documentation site](https://www.joshka.net/betamax/reference/tape-reference/),
//! including each command's behavior and each setting's default value.
//!
//! ```no_run
//! use betamax_core::{RunOptions, Runner, Tape};
//!
//! # fn main() -> betamax_core::Result<()> {
//! let tape = Tape::parse(
//!     r#"
//! Output /tmp/betamax-state.json
//! Set Shell "bash"
//! Type "printf 'hello\n'"
//! Enter
//! Wait+Screen "hello"
//! Hide
//! Type "exit"
//! Enter
//! "#,
//! )?;
//! let artifacts = Runner::new(RunOptions::default()).run_artifacts(&tape)?;
//! assert!(artifacts.final_state.is_some());
//! # Ok(())
//! # }
//! ```
//!
//! # Module Guide
//!
//! - [`tape`] contains the parser and in-memory command model. Use it when you need to inspect or
//!   construct tape commands before execution.
//! - [`runner`] executes a parsed tape and writes requested outputs.
//! - [`ghostty`] exposes the lower-level libghostty-vt session, theme, frame capture, and
//!   terminal-state types used by the runner.
//! - [`media`] contains raw frame types and format writers. These are public for embedders that
//!   want to render or encode frames directly, but the higher level [`Runner`] API is the stable
//!   path for most users.
//!
//! # Stability Notes
//!
//! The crate is pre-`1.0`. The tape parser and runner are intended to become the
//! primary stable API. Lower-level rendering and media modules are public because
//! they are useful for tests and tools, but their exact shape may still change as
//! libghostty exposes more rendering functionality.

#[cfg(windows)]
compile_error!("betamax-core is not supported on Windows because libghostty-vt-sys does not support Windows builds.");

mod error;
pub mod ghostty;
mod key;
pub mod media;
mod output;
pub mod runner;
mod shell;
pub mod tape;
mod trace;
mod wait;

#[doc(inline)]
pub use error::{Error, Result};
#[doc(inline)]
pub use runner::{FrameCapture, RunArtifacts, RunOptions, Runner, TerminalSession};
#[doc(inline)]
pub use tape::Tape;
