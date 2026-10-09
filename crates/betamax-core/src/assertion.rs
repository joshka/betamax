//! Point-in-time conditions on settled screen cells and state.

use std::ops::Range;

use miette::{miette, Context, IntoDiagnostic};

use crate::ghostty::{StateStyle, TerminalCell, TerminalState};
use crate::runner::TerminalSession;
use crate::state_diff::StateDiff;
use crate::tape::Assertion;
use crate::Result;

pub(crate) fn check(terminal: &mut impl TerminalSession, assertion: &Assertion) -> Result<()> {
    match assertion {
        Assertion::Text(text) => {
            if text.is_empty() {
                return Err(miette!("AssertText requires nonempty text").into());
            }
            let actual = terminal.screen_text()?;
            if !actual.contains(text) {
                return Err(
                    miette!("expected visible text {text:?}; actual screen:\n{actual}").into(),
                );
            }
        }
        Assertion::Absent { text, .. } => check_absent(terminal, text)?,
        Assertion::Cells { column, row, text } => {
            let cells = terminal.viewport_cells()?;
            let actual = cell_range(&cells, *column, *row, text.len())?;
            for (offset, (cell, expected)) in actual.iter().zip(text).enumerate() {
                if cell.text != *expected {
                    return Err(miette!(
                        "cell ({}, {row}): expected text {expected:?}, actual {:?}",
                        usize::from(*column) + offset,
                        cell.text
                    )
                    .into());
                }
            }
        }
        Assertion::Style {
            column,
            row,
            width,
            expected,
        } => {
            let cells = terminal.viewport_cells()?;
            let actual = cell_range(&cells, *column, *row, usize::from(*width))?;
            for (offset, cell) in actual.iter().enumerate() {
                if !style_matches(expected, &cell.style) {
                    return Err(miette!(
                        "cell ({}, {row}): expected style {expected:?}, actual {:?} on text {:?}",
                        usize::from(*column) + offset,
                        cell.style,
                        cell.text
                    )
                    .into());
                }
            }
        }
        Assertion::State(path) => {
            let source = std::fs::read(path)
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to read baseline {}", path.display()))?;
            let expected: TerminalState = serde_json::from_slice(&source)
                .into_diagnostic()
                .wrap_err_with(|| format!("invalid baseline {}", path.display()))?;
            validate_style_indexes(&expected)?;
            let actual = terminal.terminal_state()?;
            let diff = StateDiff::between(&expected, &actual);
            if !diff.is_empty() {
                return Err(miette!("baseline {} failed:\n{diff}", path.display()).into());
            }
        }
    }
    Ok(())
}

pub(crate) fn check_absent(terminal: &mut impl TerminalSession, text: &str) -> Result<()> {
    let actual = terminal.screen_text()?;
    if actual.contains(text) {
        return Err(miette!(
            "expected visible text {text:?} to be absent; actual screen:\n{actual}"
        )
        .into());
    }
    Ok(())
}

fn cell_range(
    cells: &[Vec<TerminalCell>],
    column: u16,
    row: u16,
    width: usize,
) -> Result<&[TerminalCell]> {
    if width == 0 {
        return Err(miette!("cell assertion range must contain at least one cell").into());
    }
    let range = Range {
        start: usize::from(column),
        end: usize::from(column).saturating_add(width),
    };
    cells.get(usize::from(row)).and_then(|row| row.get(range)).ok_or_else(|| miette!("assertion range ({column}, {row}) width {width} is outside the visible terminal grid").into())
}

fn style_matches(expected: &StateStyle, actual: &StateStyle) -> bool {
    let mut expected = expected.clone();
    expected.fg = expected
        .fg
        .or_else(|| actual.fg.clone())
        .map(|color| color.to_ascii_lowercase());
    expected.bg = expected
        .bg
        .or_else(|| actual.bg.clone())
        .map(|color| color.to_ascii_lowercase());
    if expected.underline.is_empty() {
        expected.underline = "none".into();
    }
    expected == *actual
}

fn validate_style_indexes(state: &TerminalState) -> Result<()> {
    for span in state.viewport.iter().chain(&state.scrollback).flatten() {
        if let crate::ghostty::StateSpan::Styled(_, index) = span {
            if *index >= state.styles.len() {
                return Err(miette!(
                    "baseline style index {index} exceeds style table length {}",
                    state.styles.len()
                )
                .into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_colors_allow_any_color_but_attributes_match_exactly() {
        let actual = StateStyle {
            fg: Some("#abcdef".into()),
            bg: Some("#123456".into()),
            bold: true,
            underline: "none".into(),
            ..StateStyle::default()
        };
        assert!(style_matches(
            &StateStyle {
                bold: true,
                ..StateStyle::default()
            },
            &actual
        ));
        assert!(!style_matches(&StateStyle::default(), &actual));
        assert!(!style_matches(
            &StateStyle {
                fg: Some("#ffffff".into()),
                bold: true,
                ..StateStyle::default()
            },
            &actual
        ));
    }
}
