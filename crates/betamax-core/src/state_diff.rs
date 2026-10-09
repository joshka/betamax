//! Text-first comparison helpers for Betamax terminal state snapshots.

use std::fmt;

use crate::ghostty::{StateCursor, StateRow, TerminalState};

const CONTEXT_LINES: usize = 2;

/// A reusable comparison result for two terminal state snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateDiff {
    differences: Vec<StateDifference>,
}

impl StateDiff {
    /// Compare two terminal states using Betamax's text-first review surface.
    ///
    /// The comparison checks terminal size, cursor metadata, viewport text, scrollback text, and
    /// all metadata/style fields, with a compact row-level summary of styled span changes. It
    /// deliberately does not expand compact spans into a cell grid.
    #[must_use]
    pub fn between(before: &TerminalState, after: &TerminalState) -> Self {
        let mut differences = Vec::new();

        if before.size != after.size {
            differences.push(StateDifference::Size {
                before: before.size,
                after: after.size,
            });
        }

        if before.cursor != after.cursor {
            differences.push(StateDifference::Cursor {
                before: before.cursor.clone(),
                after: after.cursor.clone(),
            });
        }

        for (field, before, after) in [
            (
                "total_rows",
                before.total_rows.to_string(),
                after.total_rows.to_string(),
            ),
            (
                "scrollback_rows",
                before.scrollback_rows.to_string(),
                after.scrollback_rows.to_string(),
            ),
            ("title", before.title.clone(), after.title.clone()),
            (
                "working_directory",
                before.working_directory.clone(),
                after.working_directory.clone(),
            ),
            (
                "default_style",
                style_summary(&before.default_style),
                style_summary(&after.default_style),
            ),
            (
                "styles",
                before
                    .styles
                    .iter()
                    .map(style_summary)
                    .collect::<Vec<_>>()
                    .join(", "),
                after
                    .styles
                    .iter()
                    .map(style_summary)
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ] {
            if before != after {
                differences.push(StateDifference::Field {
                    field,
                    before,
                    after,
                });
            }
        }

        for region in TextRegion::ALL {
            let before_text = region.text(before);
            let after_text = region.text(after);
            if before_text != after_text {
                differences.push(StateDifference::Text {
                    region,
                    hunks: text_hunks(before_text, after_text),
                });
            }
        }

        for region in TextRegion::ALL {
            let before_rows = region.rows(before);
            let after_rows = region.rows(after);
            if before_rows != after_rows {
                differences.push(StateDifference::Spans(SpanDiff {
                    region,
                    changed_rows: changed_row_count(before_rows, after_rows),
                    before_rows: before_rows.len(),
                    after_rows: after_rows.len(),
                }));
            }
        }

        Self { differences }
    }

    /// Return whether the compared states have no observed differences.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.differences.is_empty()
    }

    /// Return the observed differences in display order.
    #[must_use]
    pub fn differences(&self) -> &[StateDifference] {
        &self.differences
    }
}

impl fmt::Display for StateDiff {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return writeln!(formatter, "states match");
        }

        writeln!(formatter, "states differ")?;

        let mut wrote_section = false;
        let mut span_diffs = Vec::new();
        for difference in &self.differences {
            match difference {
                StateDifference::Field {
                    field,
                    before,
                    after,
                } => {
                    write_separator(formatter, &mut wrote_section)?;
                    writeln!(formatter, "{field}:\n- {before}\n+ {after}")?;
                }
                StateDifference::Size { before, after } => {
                    write_separator(formatter, &mut wrote_section)?;
                    writeln!(formatter, "size:")?;
                    writeln!(formatter, "- [{}, {}]", before[0], before[1])?;
                    writeln!(formatter, "+ [{}, {}]", after[0], after[1])?;
                }
                StateDifference::Cursor { before, after } => {
                    write_separator(formatter, &mut wrote_section)?;
                    writeln!(formatter, "cursor:")?;
                    writeln!(formatter, "- {}", cursor_summary(before))?;
                    writeln!(formatter, "+ {}", cursor_summary(after))?;
                }
                StateDifference::Text { region, hunks } => {
                    write_separator(formatter, &mut wrote_section)?;
                    writeln!(formatter, "{}:", region.text_label())?;
                    for hunk in hunks {
                        writeln!(
                            formatter,
                            "@@ -{},{} +{},{} @@",
                            hunk.before_start,
                            hunk.before_len(),
                            hunk.after_start,
                            hunk.after_len()
                        )?;
                        for line in &hunk.lines {
                            writeln!(formatter, "{}{}", line.kind.prefix(), line.text)?;
                        }
                    }
                }
                StateDifference::Spans(summary) => span_diffs.push(summary),
            }
        }

        if !span_diffs.is_empty() {
            write_separator(formatter, &mut wrote_section)?;
            writeln!(formatter, "styled spans:")?;
            for summary in span_diffs {
                writeln!(
                    formatter,
                    "~ {}: {} row(s) changed ({} before, {} after)",
                    summary.region.span_label(),
                    summary.changed_rows,
                    summary.before_rows,
                    summary.after_rows
                )?;
            }
        }

        Ok(())
    }
}

/// A single observed difference between two terminal states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateDifference {
    /// A captured metadata or style-table field changed.
    Field {
        /// Field name.
        field: &'static str,
        /// Expected field value.
        before: String,
        /// Actual field value.
        after: String,
    },
    /// The terminal grid size changed.
    Size {
        /// `[columns, rows]` from the before state.
        before: [u16; 2],
        /// `[columns, rows]` from the after state.
        after: [u16; 2],
    },
    /// The cursor position or visibility changed.
    Cursor {
        /// Cursor metadata from the before state.
        before: StateCursor,
        /// Cursor metadata from the after state.
        after: StateCursor,
    },
    /// Plain text changed in a compared state region.
    Text {
        /// Region whose text changed.
        region: TextRegion,
        /// Line-level diff hunks for the changed text.
        hunks: Vec<LineHunk>,
    },
    /// Compact styled spans changed in a compared state region.
    Spans(SpanDiff),
}

/// A plain-text terminal region that `StateDiff` compares directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRegion {
    /// Visible viewport text and spans.
    Viewport,
    /// Scrollback text and spans.
    Scrollback,
}

impl TextRegion {
    const ALL: [Self; 2] = [Self::Viewport, Self::Scrollback];

    fn text(self, state: &TerminalState) -> &str {
        match self {
            Self::Viewport => &state.viewport_text,
            Self::Scrollback => &state.scrollback_text,
        }
    }

    fn rows(self, state: &TerminalState) -> &[StateRow] {
        match self {
            Self::Viewport => &state.viewport,
            Self::Scrollback => &state.scrollback,
        }
    }

    fn text_label(self) -> &'static str {
        match self {
            Self::Viewport => "viewport_text",
            Self::Scrollback => "scrollback_text",
        }
    }

    fn span_label(self) -> &'static str {
        match self {
            Self::Viewport => "viewport",
            Self::Scrollback => "scrollback",
        }
    }
}

/// A contiguous line-level text diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineHunk {
    /// One-based starting line in the before text.
    pub before_start: usize,
    /// One-based starting line in the after text.
    pub after_start: usize,
    /// Lines shown in this hunk.
    pub lines: Vec<LineChange>,
}

impl LineHunk {
    fn before_len(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind != LineChangeKind::Added)
            .count()
    }

    fn after_len(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind != LineChangeKind::Removed)
            .count()
    }
}

/// A line in a text diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineChange {
    /// Whether the line is context, removed from before, or added in after.
    pub kind: LineChangeKind,
    /// Line text without a trailing newline.
    pub text: String,
}

/// The role of one line in a text diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChangeKind {
    /// Context line that is equal in both states.
    Equal,
    /// Line that appears only in the before state.
    Removed,
    /// Line that appears only in the after state.
    Added,
}

impl LineChangeKind {
    fn prefix(self) -> char {
        match self {
            Self::Equal => ' ',
            Self::Removed => '-',
            Self::Added => '+',
        }
    }
}

/// Row-level summary for changed styled spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanDiff {
    /// Region whose compact styled spans changed.
    pub region: TextRegion,
    /// Number of row positions with different compact spans.
    pub changed_rows: usize,
    /// Number of compact rows in the before state.
    pub before_rows: usize,
    /// Number of compact rows in the after state.
    pub after_rows: usize,
}

fn style_summary(style: &crate::ghostty::StateStyle) -> String {
    let mut style = style.clone();
    // State JSON omits `none` underline; deserializing that omission yields an empty string.
    if style.underline.is_empty() {
        style.underline = "none".into();
    }
    format!("{style:?}")
}

fn write_separator(formatter: &mut fmt::Formatter<'_>, wrote_section: &mut bool) -> fmt::Result {
    if *wrote_section {
        writeln!(formatter)?;
    }
    *wrote_section = true;
    Ok(())
}

fn cursor_summary(cursor: &StateCursor) -> String {
    format!("x={} y={} visible={}", cursor.x, cursor.y, cursor.visible)
}

fn text_hunks(before_text: &str, after_text: &str) -> Vec<LineHunk> {
    let before = before_text.lines().collect::<Vec<_>>();
    let after = after_text.lines().collect::<Vec<_>>();

    let common_prefix = common_prefix_len(&before, &after);
    let common_suffix = common_suffix_len(&before, &after, common_prefix);

    let before_change_end = before.len() - common_suffix;
    let after_change_end = after.len() - common_suffix;
    let context_start = common_prefix.saturating_sub(CONTEXT_LINES);
    let suffix_context = common_suffix.min(CONTEXT_LINES);
    let before_context_end = (before_change_end + suffix_context).min(before.len());

    let mut lines = Vec::new();
    lines.extend(
        before[context_start..common_prefix]
            .iter()
            .map(|line| line_change(LineChangeKind::Equal, line)),
    );
    lines.extend(
        before[common_prefix..before_change_end]
            .iter()
            .map(|line| line_change(LineChangeKind::Removed, line)),
    );
    lines.extend(
        after[common_prefix..after_change_end]
            .iter()
            .map(|line| line_change(LineChangeKind::Added, line)),
    );
    lines.extend(
        before[before_change_end..before_context_end]
            .iter()
            .map(|line| line_change(LineChangeKind::Equal, line)),
    );

    vec![LineHunk {
        before_start: context_start + 1,
        after_start: context_start + 1,
        lines,
    }]
}

fn line_change(kind: LineChangeKind, line: &&str) -> LineChange {
    LineChange {
        kind,
        text: (*line).to_string(),
    }
}

fn common_prefix_len(before: &[&str], after: &[&str]) -> usize {
    before
        .iter()
        .zip(after)
        .take_while(|(before, after)| before == after)
        .count()
}

fn common_suffix_len(before: &[&str], after: &[&str], common_prefix: usize) -> usize {
    let unmatched_before = before.len().saturating_sub(common_prefix);
    let unmatched_after = after.len().saturating_sub(common_prefix);
    before
        .iter()
        .rev()
        .zip(after.iter().rev())
        .take(unmatched_before.min(unmatched_after))
        .take_while(|(before, after)| before == after)
        .count()
}

fn changed_row_count(before: &[StateRow], after: &[StateRow]) -> usize {
    let row_count = before.len().max(after.len());
    (0..row_count)
        .filter(|row| before.get(*row) != after.get(*row))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghostty::{StateSpan, StateStyle};

    #[test]
    fn catches_style_table_only_changes_and_metadata() {
        let mut before = test_state("same\n", "");
        before.styles = vec![StateStyle {
            fg: Some("#ff0000".into()),
            ..StateStyle::default()
        }];
        before.viewport = vec![vec![StateSpan::Styled("same".into(), 0)]];
        let mut after = before.clone();
        after.styles[0].fg = Some("#00ff00".into());
        after.title = "changed".into();
        let diff = StateDiff::between(&before, &after).to_string();
        assert!(diff.contains("styles:"));
        assert!(diff.contains("title:"));
        assert!(diff.contains("#ff0000"));
        assert!(diff.contains("#00ff00"));
    }

    #[test]
    fn json_round_trip_treats_omitted_none_underline_equivalently() {
        let mut before = test_state("same\n", "");
        before.default_style.underline = "none".into();
        let json = serde_json::to_vec(&before).unwrap();
        let after: TerminalState = serde_json::from_slice(&json).unwrap();
        assert!(StateDiff::between(&before, &after).is_empty());
    }

    #[test]
    fn reports_matching_states() {
        let before = test_state("same\n", "");
        let after = before.clone();

        let diff = StateDiff::between(&before, &after);

        assert!(diff.is_empty());
        assert_eq!(diff.to_string(), "states match\n");
    }

    #[test]
    fn reports_size_cursor_and_text_differences() {
        let before = test_state("alpha\nbeta\ngamma\n", "");
        let mut after = test_state("alpha\nbravo\ngamma\n", "");
        after.size = [100, 24];
        after.cursor.x = 4;

        let diff = StateDiff::between(&before, &after);
        let output = diff.to_string();

        assert!(!diff.is_empty());
        assert!(output.contains("size:\n- [80, 24]\n+ [100, 24]"));
        assert!(output.contains("cursor:\n- x=0 y=0 visible=false\n+ x=4 y=0 visible=false"));
        assert!(output.contains("viewport_text:\n@@ -1,3 +1,3 @@"));
        assert!(output.contains(" alpha\n-beta\n+bravo\n gamma"));
    }

    #[test]
    fn reports_scrollback_text_differences() {
        let before = test_state("", "old prompt\nsame\n");
        let after = test_state("", "new prompt\nsame\n");

        let output = StateDiff::between(&before, &after).to_string();

        assert!(output.contains("scrollback_text:\n@@ -1,2 +1,2 @@"));
        assert!(output.contains("-old prompt\n+new prompt"));
    }

    #[test]
    fn summarizes_span_only_differences() {
        let before = test_state("same\n", "");
        let mut after = before.clone();
        after.styles = vec![StateStyle {
            fg: Some("#ff0000".to_string()),
            ..StateStyle::default()
        }];
        after.viewport = vec![vec![StateSpan::Styled("same".to_string(), 0)]];

        let output = StateDiff::between(&before, &after).to_string();

        assert!(!output.contains("viewport_text:"));
        assert!(output.contains("styled spans:\n~ viewport: 1 row(s) changed (1 before, 1 after)"));
    }

    fn test_state(viewport_text: &str, scrollback_text: &str) -> TerminalState {
        TerminalState {
            size: [80, 24],
            total_rows: 24,
            scrollback_rows: usize::from(!scrollback_text.is_empty()),
            title: String::new(),
            working_directory: String::new(),
            cursor: StateCursor {
                x: 0,
                y: 0,
                visible: false,
            },
            default_style: StateStyle::default(),
            styles: Vec::new(),
            viewport_text: viewport_text.to_string(),
            scrollback_text: scrollback_text.to_string(),
            viewport: rows_for_text(viewport_text),
            scrollback: rows_for_text(scrollback_text),
        }
    }

    fn rows_for_text(text: &str) -> Vec<StateRow> {
        text.lines()
            .map(|line| vec![StateSpan::Text(line.to_string())])
            .collect()
    }
}
