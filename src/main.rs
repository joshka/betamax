mod cli;
mod commands;

use cli::Cli;
use miette::IntoDiagnostic;
use tracing::{instrument::WithSubscriber, level_filters::LevelFilter};
use tracing_subscriber::{filter::Directive, fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive(LevelFilter::DEBUG.into()))
        .with(fmt::layer())
        .init();
    Cli::run().await?;
    Ok(())
}
