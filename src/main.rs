mod cli;
mod list_themes;
mod run;

use cli::Cli;

fn main() -> miette::Result<()> {
    Cli::run()?;
    Ok(())
}
