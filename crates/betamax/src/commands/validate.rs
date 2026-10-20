//! `betamax validate` command.
//!
//! Validation parses tape files without running commands, spawning a shell, loading themes, or
//! writing outputs. It is therefore a syntax check, not a full execution preflight. Tape warnings
//! are printed to stderr; `--strict` makes them fail validation.

use std::path::PathBuf;

use betamax_core::tape::Tape;
use glob::glob;
use miette::{miette, Context, IntoDiagnostic, Result};

use super::warnings::print_tape_warning;

#[derive(Debug, clap::Parser)]
pub struct Validate {
    /// Tape file paths or glob patterns to validate
    inputs: Vec<String>,

    /// Fail when any tape has warnings
    #[arg(long)]
    strict: bool,
}

impl Validate {
    /// Validate all provided tape paths or globs.
    ///
    /// Empty globs are treated as errors so CI jobs do not silently validate nothing.
    ///
    /// # Errors
    ///
    /// Returns an error if no inputs are provided, a glob cannot be read, an input matches no
    /// files, a file cannot be read, a tape cannot be parsed, or `--strict` is set and a tape has
    /// warnings.
    pub fn run(&self) -> Result<()> {
        if self.inputs.is_empty() {
            return Err(miette!("validate requires at least one tape path or glob"));
        }

        let mut validated = 0usize;
        let mut warned = 0usize;
        for input in &self.inputs {
            let paths = expand_input(input)?;
            if paths.is_empty() {
                return Err(miette!("no tape files matched `{input}`"));
            }

            for path in paths {
                let source = std::fs::read_to_string(&path)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("failed to read {}", path.display()))?;
                Tape::parse_with_sources(&source, Some(&path), &mut |warning| {
                    print_tape_warning(path.display(), &warning);
                    warned += 1;
                })
                .wrap_err_with(|| format!("failed to parse {}", path.display()))?;
                validated += 1;
            }
        }

        if self.strict && warned > 0 {
            return Err(miette!(
                "{warned} warning(s) in {validated} tape file(s) (--strict)"
            ));
        }
        println!("validated {validated} tape file(s)");
        Ok(())
    }
}

/// Expand a literal path or glob into file paths.
fn expand_input(input: &str) -> Result<Vec<PathBuf>> {
    if has_glob_metachar(input) {
        return glob(input)
            .into_diagnostic()?
            .map(|entry| entry.into_diagnostic())
            .collect();
    }

    Ok(vec![PathBuf::from(input)])
}

/// Return whether an input string contains glob syntax.
fn has_glob_metachar(input: &str) -> bool {
    input.contains('*') || input.contains('?') || input.contains('[')
}
