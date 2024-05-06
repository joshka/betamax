use std::path::{Path, PathBuf};

use axum::{routing::get, Router};
use clap_stdin::FileOrStdin;
use miette::IntoDiagnostic;
use tokio::net::TcpListener;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::info;

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
    pub async fn run(&self) -> miette::Result<()> {
        let contents = self.input.clone().contents().into_diagnostic()?;
        // println!("input={}", contents);
        start_server().await?;
        Ok(())
    }
}

async fn start_server() -> miette::Result<()> {
    let app = Router::new()
        .nest_service("/", ServeDir::new("assets"))
        .nest_service("/xterm", ServeDir::new("node_modules/@xterm/xterm"))
        .layer(TraceLayer::new_for_http());
    let listener = TcpListener::bind("127.0.0.1:0").await.into_diagnostic()?;
    let addr = listener.local_addr().into_diagnostic()?;
    info!("listening at http://{addr}");
    axum::serve(listener, app).await.into_diagnostic()
}
