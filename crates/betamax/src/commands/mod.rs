//! CLI subcommands and dispatch.
//!
//! Each subcommand module owns argument fields and command-specific presentation concerns. This
//! module owns the top-level command enum so `main.rs` can stay focused on process setup and
//! top-level argument parsing.

use clap::Parser;
use miette::{miette, Result};

mod list_themes;
mod new;
mod run;
mod validate;

use list_themes::ListThemes;
use new::New;
use run::Run;
use validate::Validate;

#[derive(Debug, Parser)]
pub enum Command {
    /// Create a new tape file with example tape file contents and documentation
    New(New),

    /// Publish your GIF and get a shareable URL
    Publish,

    /// Create a new tape file by recording your actions
    Record,

    /// Run a tape file
    Run(Run),

    /// Start the SSH server
    Serve,

    /// List available themes
    Themes(ListThemes),

    /// Validate a glob file path and parses all the files to ensure they are valid without running
    /// them
    Validate(Validate),
}

impl Command {
    /// Dispatch a parsed subcommand.
    ///
    /// VHS commands that require network services or interactive recording are retained as explicit
    /// "not implemented" errors so users get a clear answer instead of an unknown-command failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected command fails or if the selected VHS-compatible command is
    /// intentionally not implemented by Betamax.
    pub fn run(&self) -> Result<()> {
        match self {
            Command::New(command) => command.run()?,
            Command::Publish => return Err(miette!("publish is intentionally not implemented")),
            Command::Record => return Err(miette!("record is intentionally not implemented")),
            Command::Run(command) => command.run()?,
            Command::Serve => return Err(miette!("serve is intentionally not implemented")),
            Command::Themes(command) => command.run()?,
            Command::Validate(command) => command.run()?,
        }
        Ok(())
    }
}
