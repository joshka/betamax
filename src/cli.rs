use clap::Parser;
use clap_stdin::FileOrStdin;
use miette::IntoDiagnostic;

use crate::{list_themes::ListThemesCommand, run::RunCommand};

#[derive(Debug, Parser)]
#[clap(version, author, about)]
/// A terminal application for recording and running shell commands.
pub struct Cli {
    #[clap(subcommand)]
    command: Command,
}

impl Cli {
    pub fn run() -> miette::Result<()> {
        let cli = Cli::parse();
        cli.command.run()
    }
}

#[derive(Debug, Parser)]
enum Command {
    /// Create a new tape file with example tape file contents and documentation
    New,
    /// Publish your GIF to vhs.charm.sh and get a shareable URL
    Publish,
    /// Create a new tape file by recording your actions
    Record,
    /// Run a tape file
    Run(RunCommand),
    /// Start the VHS SSH server
    Serve,
    /// List available themes
    #[clap(name = "themes")]
    ListThemes(ListThemesCommand),
    /// Validate a glob file path and parses all the files to ensure they are valid without running them
    Validate,
}

impl Command {
    fn run(&self) -> miette::Result<()> {
        match self {
            Command::New => todo!("new"),
            Command::Publish => todo!("publish"),
            Command::Record => todo!("record"),
            Command::Run(command) => command.run()?,
            Command::Serve => todo!("serve"),
            Command::ListThemes(command) => command.list_themes()?,
            Command::Validate => todo!("validate"),
        }
        Ok(())
    }
}
