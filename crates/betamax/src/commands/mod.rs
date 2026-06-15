//! CLI subcommands and dispatch.
//!
//! Each subcommand module owns argument fields and command-specific presentation concerns. This
//! module owns the top-level command enum so `main.rs` can stay focused on process setup and
//! top-level argument parsing.

use clap::Parser;
use miette::Result;

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

    /// Run a tape file
    Run(Run),

    /// List available themes
    Themes(ListThemes),

    /// Validate a glob file path and parses all the files to ensure they are valid without running
    /// them
    Validate(Validate),
}

impl Command {
    /// Dispatch a parsed subcommand.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected command fails.
    pub fn run(&self) -> Result<()> {
        match self {
            Command::New(command) => command.run()?,
            Command::Run(command) => command.run()?,
            Command::Themes(command) => command.run()?,
            Command::Validate(command) => command.run()?,
        }
        Ok(())
    }
}
