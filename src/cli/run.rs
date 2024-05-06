use std::path::{Path, PathBuf};

use clap_stdin::FileOrStdin;
use miette::IntoDiagnostic;

#[derive(Debug, clap::Parser)]
pub struct RunCommand {
    /// File to read input from, or `-` for stdin
    input: FileOrStdin,

    /// Publish the output
    #[clap(long, short)]
    publish: bool,

    /// File to write output to
    #[clap(long, short, value_name = "FILE")]
    output: Option<PathBuf>,

    /// do not log messages. If publish flag is provided, it will log shareable URL
    #[clap(long, short)]
    quiet: bool,
}

impl RunCommand {
    pub fn run(&self) -> miette::Result<()> {
        let contents = self.input.clone().contents().into_diagnostic()?;
        println!("input={}", contents);
        Ok(())
    }
}
