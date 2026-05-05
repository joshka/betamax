//! `betamax themes` command.
//!
//! This command exposes the same theme search path used by rendering, which makes it useful for
//! checking exact names before writing `Set Theme "..."`

use betamax_core::ghostty::theme_names;
use clap::Parser;
use miette::{IntoDiagnostic, Result};

/// List available themes
#[derive(Debug, Parser)]
pub struct ListThemes {
    /// output in markdown format
    #[arg(long)]
    markdown: bool,

    /// output in JSON format
    #[arg(long)]
    json: bool,
}

impl ListThemes {
    /// Print theme names in plain text, Markdown, or JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if theme discovery fails while inspecting a readable theme directory, or if
    /// JSON output cannot be serialized.
    pub fn run(&self) -> Result<()> {
        let themes = theme_names()?;
        if self.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&themes).into_diagnostic()?
            );
            return Ok(());
        }
        let (prefix, suffix) = if self.markdown {
            ("* `", "`")
        } else {
            ("", "")
        };
        for theme in themes {
            println!("{prefix}{theme}{suffix}");
        }
        Ok(())
    }
}
