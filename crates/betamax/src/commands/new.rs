//! `betamax new` command.
//!
//! This command writes a starter tape from a bundled template. It exists for VHS CLI familiarity;
//! it does not inspect the target directory or infer project-specific settings.

use std::fs::write;
use std::path::PathBuf;

use clap::Parser;
use interpolator::iformat;
use miette::{miette, IntoDiagnostic, Result};

#[derive(Debug, Parser)]
pub struct New {
    /// The name of the tape file to create
    name: PathBuf,
}

impl New {
    /// Render the bundled template and write it to the requested path.
    ///
    /// # Errors
    ///
    /// Returns an error if the target path has no file stem, template interpolation fails, or the
    /// output file cannot be written.
    pub fn run(&self) -> Result<()> {
        let template = include_str!("new.tape");
        let name = self
            .name
            .file_stem()
            .ok_or(miette!("No file name"))?
            .to_string_lossy();
        let contents = iformat!(template, name).into_diagnostic()?;
        write(&self.name, contents).into_diagnostic()?;
        Ok(())
    }
}
