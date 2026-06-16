//! Betamax command-line entry point.
//!
//! The binary intentionally delegates core behavior to the library crate. CLI modules should handle
//! argument parsing, source loading, and presentation; parsing, execution, rendering, and media
//! writing belong in library modules where they can be tested directly.

use clap::builder::styling::AnsiColor;
use clap::builder::Styles;
use clap::Parser;
use miette::Result;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

mod commands;

use commands::Command;

fn main() -> Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_writer(std::io::stderr)
        .init();
    if let Err(error) = libghostty_vt::set_logger(Some(Box::new(libghostty_vt::log::TracingLogger)))
    {
        tracing::debug!(%error, "failed to install libghostty-vt logger");
    }
    Cli::run()
}

const HELP_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Blue.on_default().bold())
    .usage(AnsiColor::Blue.on_default().bold())
    .literal(AnsiColor::White.on_default())
    .placeholder(AnsiColor::Green.on_default());

#[derive(Debug, Parser)]
#[command(version, author, about, styles = HELP_STYLES)]
/// A terminal application for recording and running shell commands.
pub struct Cli {
    /// Selected subcommand.
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    /// Parse process arguments and run the selected command.
    ///
    /// # Errors
    ///
    /// Returns an error if argument dispatch reaches a subcommand that fails during parsing,
    /// validation, rendering, or output writing.
    pub fn run() -> Result<()> {
        let cli = Cli::parse();
        cli.command.run()
    }
}
