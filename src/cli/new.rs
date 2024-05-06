use std::{
    fs::{write, File},
    path::PathBuf,
};

use clap::Parser;
use interpolator::{format, iformat, iwrite};
use miette::{miette, IntoDiagnostic};

#[derive(Debug, Parser)]
pub struct NewCommand {
    /// The name of the tape file to create
    name: PathBuf,
}

impl NewCommand {
    pub fn new(&self) -> miette::Result<()> {
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
