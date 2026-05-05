//! Tape parsing and the in-memory command model.
//!
//! This module intentionally keeps parsing separate from execution. A [`Tape`] is a lossless enough
//! representation of what the user asked for, but it does not decide whether a command requires a
//! renderer, how shell startup should work, or what media files should be written. Those decisions
//! live in the runner and output-classification modules so tests can validate the CLI language
//! without spawning a PTY.
//!
//! For user-facing command behavior and setting defaults, see the repository's
//! [`docs/tape-reference.md`](https://github.com/joshka/betamax/blob/main/docs/tape-reference.md).
//!
//! # Examples
//!
//! ```
//! use betamax_core::tape::{Command, Tape};
//!
//! # fn main() -> betamax_core::Result<()> {
//! let tape = Tape::parse(
//!     r#"
//! Output demo.gif
//! Set Theme "Aardvark Blue"
//! Type "echo hello"
//! Enter
//! Wait+Screen "hello"
//! "#,
//! )?;
//!
//! assert!(matches!(tape.commands[0], Command::Output(_)));
//! assert_eq!(tape.outputs().count(), 1);
//! # Ok(())
//! # }
//! ```

mod model;
mod parser;

#[doc(inline)]
pub use model::{Command, Key, KeyCode, KeyModifiers, Tape, Value, WaitPattern, WaitTarget};
#[doc(inline)]
pub use parser::parse_duration;
