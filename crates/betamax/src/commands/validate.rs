//! `betamax validate` command.
//!
//! Validation parses tape files without running commands, spawning a shell, loading themes, or
//! writing outputs. It is therefore a syntax check, not a full execution preflight. Tape warnings
//! are printed to stderr; `--strict` makes them fail validation.

use std::path::{Path, PathBuf};

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
    /// Empty globs are treated as errors so CI jobs do not silently validate nothing. A failed
    /// input does not hide later ones: each failure is printed before the command fails.
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
        let mut failures = Vec::new();
        for input in &self.inputs {
            let paths = match expand_input(input) {
                Ok(paths) => paths,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            };
            if paths.is_empty() {
                failures.push(miette!("no tape files matched `{input}`"));
            }
            for path in paths {
                match validate_tape(&path, &mut warned) {
                    Ok(()) => validated += 1,
                    Err(error) => failures.push(error),
                }
            }
        }

        if failures.len() > 1 {
            for failure in &failures {
                eprintln!("{failure:?}");
            }
            return Err(miette!(
                "{} input(s) failed; validated {validated} tape file(s)",
                failures.len()
            ));
        }
        if let Some(failure) = failures.pop() {
            return Err(failure);
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

/// Parse one tape, printing its warnings and adding them to `warned`.
fn validate_tape(path: &Path, warned: &mut usize) -> Result<()> {
    let source = std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read {}", path.display()))?;
    Tape::parse_with_sources(&source, Some(path), &mut |warning| {
        print_tape_warning(path.display(), &warning);
        *warned += 1;
    })
    .wrap_err_with(|| format!("failed to parse {}", path.display()))?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_every_input_before_failing() {
        let dir = std::env::temp_dir().join(format!("betamax-validate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tape = |name: &str, source: &str| {
            let path = dir.join(name);
            std::fs::write(&path, source).unwrap();
            path.display().to_string()
        };
        let inputs = vec![
            tape("bad.tape", "Bogus"),
            dir.join("missing-*.tape").display().to_string(),
            tape("good.tape", "Enter"),
        ];
        let error = Validate {
            inputs,
            strict: false,
        }
        .run()
        .unwrap_err();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            error.to_string(),
            "2 input(s) failed; validated 1 tape file(s)"
        );
    }
}
