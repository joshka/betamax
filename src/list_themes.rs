use clap::Parser;
use miette::{Diagnostic, SourceOffset, SourceSpan};
use serde::Deserialize;
use thiserror::Error;

/// List available themes
#[derive(Debug, Parser)]
pub struct ListThemesCommand {
    /// output in markdown format
    #[clap(long)]
    markdown: bool,
}

impl ListThemesCommand {
    pub fn list_themes(&self) -> miette::Result<(), SerdeJsonErrorWrapper> {
        let (prefix, suffix) = if self.markdown {
            ("* `", "`")
        } else {
            ("", "")
        };
        let themes_str = include_str!("themes.json");
        let themes: Vec<Theme> =
            serde_json::from_str(themes_str).map_err(|error| SerdeJsonErrorWrapper {
                src: themes_str.to_string(),
                err_span: SourceOffset::from_location(themes_str, error.line(), error.column())
                    .into(),
                source: error,
            })?;
        for theme in themes {
            println!("{prefix}{name}{suffix}", name = theme.name);
        }

        Ok(())
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("{source}")]
pub struct SerdeJsonErrorWrapper {
    #[source_code]
    src: String,
    #[label("error location")]
    err_span: SourceSpan,
    #[source]
    source: serde_json::Error,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Theme {
    name: String,
    black: String,
    red: String,
    green: String,
    yellow: String,
    blue: String,
    magenta: String,
    cyan: String,
    white: String,
    bright_black: String,
    bright_red: String,
    bright_green: String,
    bright_yellow: String,
    bright_blue: String,
    bright_magenta: String,
    bright_cyan: String,
    bright_white: String,
    background: String,
    foreground: String,
    cursor: Option<String>,
    selection: Option<String>,
    meta: Meta,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    is_dark: bool,
    credits: Option<Vec<Credit>>,
}

#[derive(Debug, Deserialize)]
struct Credit {
    name: String,
    link: String,
}
