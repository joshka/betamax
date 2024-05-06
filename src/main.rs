mod cli;
mod commands;

use cli::Cli;

fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();
    Cli::run()?;
    Ok(())
}
