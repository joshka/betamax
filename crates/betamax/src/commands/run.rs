//! `betamax run` command.
//!
//! The CLI command is intentionally thin: it reads tape source, appends an optional CLI-provided
//! output path, builds runner options, and delegates execution to the library API.

use std::path::PathBuf;

use betamax_core::media::{MediaProgress, MediaProgressKind, MediaProgressReporter};
use betamax_core::runner::{RunOptions, Runner};
use betamax_core::tape::Tape;
use clap_stdin::FileOrStdin;
use indicatif::{ProgressBar, ProgressStyle};
use miette::{IntoDiagnostic, Result};

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BLUE: &str = "\x1b[34m";

#[derive(Debug, clap::Parser)]
pub struct Run {
    /// File to read input from, or `-` for stdin
    input: FileOrStdin,

    /// Publish the output
    #[arg(long, short)]
    publish: bool,

    /// File to write output to
    #[arg(long, short, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Do not log messages. If publish flag is provided, it will log shareable URL
    #[arg(long, short)]
    quiet: bool,
}

impl Run {
    /// Execute the command.
    ///
    /// A CLI `--output` path is appended as an `Output` command instead of overriding tape outputs,
    /// so a tape can still write multiple artifacts.
    ///
    /// # Errors
    ///
    /// Returns an error if the input cannot be read, the tape cannot be parsed, runner execution
    /// fails, or a requested output cannot be written.
    pub fn run(&self) -> Result<()> {
        if !self.quiet {
            println!("{ANSI_BLUE}running {}{ANSI_RESET}", self.input.filename());
        }
        let source = self.input.clone().contents().into_diagnostic()?;
        let mut tape = Tape::parse(&source)?;
        if let Some(output) = &self.output {
            tape.add_output(output.clone());
        }

        let options = RunOptions {
            publish: self.publish,
            quiet: self.quiet,
        };
        let mut runner = Runner::new(options);
        if !self.quiet {
            runner = runner.with_media_progress(CliMediaProgress::default());
        }
        runner.run(&tape)?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct CliMediaProgress {
    bar: Option<ProgressBar>,
    kind: Option<MediaProgressKind>,
}

impl MediaProgressReporter for CliMediaProgress {
    fn report(&mut self, progress: MediaProgress) {
        self.ensure_bar(progress);
        if let Some(bar) = &self.bar {
            bar.set_position(progress.position as u64);
            if progress.position == progress.total {
                bar.finish_and_clear();
                self.bar = None;
                self.kind = None;
            }
        }
    }
}

impl CliMediaProgress {
    fn ensure_bar(&mut self, progress: MediaProgress) {
        if self.kind != Some(progress.kind) {
            if let Some(bar) = self.bar.take() {
                bar.finish_and_clear();
            }
            let bar = ProgressBar::new(progress.total as u64);
            bar.set_style(progress_style());
            bar.set_message(progress_label(progress.kind));
            self.bar = Some(bar);
            self.kind = Some(progress.kind);
        }
    }
}

impl Drop for CliMediaProgress {
    fn drop(&mut self) {
        if let Some(bar) = self.bar.take() {
            bar.finish_and_clear();
        }
    }
}

fn progress_style() -> ProgressStyle {
    let style = ProgressStyle::with_template(
        "{spinner:.blue} {msg} {pos}/{len} [{bar:32.cyan/blue}] {eta}",
    );
    match style {
        Ok(style) => style.progress_chars("=> "),
        Err(_) => ProgressStyle::default_bar().progress_chars("=> "),
    }
}

fn progress_label(kind: MediaProgressKind) -> &'static str {
    match kind {
        MediaProgressKind::Gif => "encoding gif",
        MediaProgressKind::PngSequence => "writing frames",
        MediaProgressKind::VideoFrames => "preparing video frames",
    }
}
