//! Output classification.
//!
//! The tape parser records output paths without interpreting them. This module is the narrow place
//! where paths become output kinds and where commands that need terminal capture are detected.

use std::path::{Path, PathBuf};

use miette::miette;

use crate::tape::{Command, Tape};
use crate::Result;

#[derive(Debug, Default)]
pub(crate) struct Outputs {
    /// Extensionless primary outputs, written as numbered PNG frame directories.
    pub(crate) frame_dirs: Vec<PathBuf>,
    /// Animated GIF primary outputs.
    pub(crate) gifs: Vec<PathBuf>,
    /// MP4 primary outputs written through ffmpeg.
    pub(crate) mp4s: Vec<PathBuf>,
    /// Static PNG primary outputs of the final frame.
    pub(crate) pngs: Vec<PathBuf>,
    /// JSON terminal-state primary outputs.
    pub(crate) states: Vec<PathBuf>,
    /// WebM primary outputs written through ffmpeg.
    pub(crate) webms: Vec<PathBuf>,
    /// Whether a command, rather than a primary output, requires capture.
    ///
    /// Inline `Screenshot`, inline `State`, and `Wait` need terminal state even if the tape does
    /// not request a captured primary output.
    pub(crate) needs_capture: bool,
}

impl Outputs {
    /// Return whether the runner must instantiate libghostty-vt capture.
    pub(crate) fn requires_capture(&self) -> bool {
        self.needs_capture
            || !self.gifs.is_empty()
            || !self.frame_dirs.is_empty()
            || !self.mp4s.is_empty()
            || !self.pngs.is_empty()
            || !self.states.is_empty()
            || !self.webms.is_empty()
    }

    /// Return primary output paths in the order the runner writes output groups.
    ///
    /// This is deterministic but not necessarily source order; callers should treat it as metadata
    /// about written primary outputs, not as a reconstruction of the tape.
    pub(crate) fn paths(&self) -> Vec<PathBuf> {
        self.gifs
            .iter()
            .chain(&self.mp4s)
            .chain(&self.pngs)
            .chain(&self.states)
            .chain(&self.webms)
            .chain(&self.frame_dirs)
            .cloned()
            .collect()
    }
}

/// Classify all outputs and capture-dependent commands in a tape.
///
/// Unsupported extensions fail before a PTY is spawned. Extensionless `Output` paths mean "write a
/// PNG sequence directory", mirroring VHS behavior.
pub(crate) fn classify_outputs(tape: &Tape) -> Result<Outputs> {
    let mut outputs = Outputs::default();

    for command in &tape.commands {
        match command {
            Command::Output(path) => match extension(path).as_deref() {
                Some("gif") => outputs.gifs.push(path.clone()),
                Some("png") => outputs.pngs.push(path.clone()),
                Some("json") => outputs.states.push(path.clone()),
                Some("webm") => outputs.webms.push(path.clone()),
                Some("mp4") => outputs.mp4s.push(path.clone()),
                Some(ext) => {
                    return Err(miette!(
                        "unsupported output extension `.{ext}`: {}",
                        path.display()
                    )
                    .into());
                }
                None => outputs.frame_dirs.push(path.clone()),
            },
            Command::Screenshot(path) => match extension(path).as_deref() {
                Some("png") => outputs.needs_capture = true,
                Some(ext) => {
                    return Err(miette!(
                        "Screenshot only supports .png in the first cut, got `.{ext}`: {}",
                        path.display()
                    )
                    .into());
                }
                None => {
                    return Err(
                        miette!("screenshot path has no extension: {}", path.display()).into(),
                    );
                }
            },
            Command::State(path) => match extension(path).as_deref() {
                Some("json") => outputs.needs_capture = true,
                Some(ext) => {
                    return Err(miette!(
                        "State only supports .json in the first cut, got `.{ext}`: {}",
                        path.display()
                    )
                    .into());
                }
                None => {
                    return Err(miette!("state path has no extension: {}", path.display()).into());
                }
            },
            Command::Wait { .. } => outputs.needs_capture = true,
            _ => {}
        }
    }

    Ok(outputs)
}

/// Return a lowercase extension without the leading dot.
fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_webm_output() {
        let tape = Tape::parse("Output demo.webm").unwrap();
        let outputs = classify_outputs(&tape).unwrap();

        assert_eq!(outputs.webms, vec![PathBuf::from("demo.webm")]);
        assert!(outputs.requires_capture());
    }

    #[test]
    fn classifies_json_state_output() {
        let tape = Tape::parse("Output terminal.json State checkpoint.json").unwrap();
        let outputs = classify_outputs(&tape).unwrap();

        assert_eq!(outputs.states, vec![PathBuf::from("terminal.json")]);
        assert!(outputs.requires_capture());
    }

    #[test]
    fn classifies_extensionless_output_as_png_sequence() {
        let tape = Tape::parse("Output frames").unwrap();
        let outputs = classify_outputs(&tape).unwrap();

        assert_eq!(outputs.frame_dirs, vec![PathBuf::from("frames")]);
        assert!(outputs.requires_capture());
    }
}
