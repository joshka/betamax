//! `betamax run` command.
//!
//! The CLI command is intentionally thin: it reads tape source, appends an optional CLI-provided
//! output path, builds runner options, and delegates execution to the library API.

use std::path::PathBuf;

use betamax_core::runner::{RunOptions, Runner};
use betamax_core::tape::Tape;
use clap_stdin::FileOrStdin;
use miette::{IntoDiagnostic, Result};

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BLUE: &str = "\x1b[34m";

#[derive(Debug, clap::Parser)]
pub struct Run {
    /// File to read input from, or `-` for stdin
    input: FileOrStdin,

    /// Publish the output
    #[arg(long, short)]
    publish: bool,

    /// File to write output to
    #[arg(long, short, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Do not log messages. If publish flag is provided, it will log shareable URL
    #[arg(long, short)]
    quiet: bool,
}

impl Run {
    /// Execute the command.
    ///
    /// A CLI `--output` path is appended as an `Output` command instead of overriding tape outputs,
    /// so a tape can still write multiple artifacts.
    ///
    /// # Errors
    ///
    /// Returns an error if the input cannot be read, the tape cannot be parsed, runner execution
    /// fails, or a requested output cannot be written.
    pub fn run(&self) -> Result<()> {
        if !self.quiet {
            println!("{ANSI_BLUE}running {}{ANSI_RESET}", self.input.filename());
        }
        let source = self.input.clone().contents().into_diagnostic()?;
        let mut tape = Tape::parse(&source)?;
        if let Some(output) = &self.output {
            tape.add_output(output.clone());
        }

        let options = RunOptions {
            publish: self.publish,
            quiet: self.quiet,
        };
        Runner::new(options).run(&tape)?;
        Ok(())
    }
}
