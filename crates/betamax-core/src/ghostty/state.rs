//! Terminal state snapshots and style compaction.

use std::collections::HashMap;

use libghostty_vt::style::{RgbColor, Style, Underline};
use serde::Serialize;

use super::color::hex_color;
use super::render_theme::RenderTheme;

/// JSON-serializable terminal snapshot.
///
/// The snapshot includes both plain text and styled spans. Plain text is convenient for direct
/// assertions; styled spans are intended for snapshot tools such as `insta` where color and
/// attribute changes matter.
///
/// `viewport_text` and `scrollback_text` are the easiest fields to assert against in tests. They
/// contain trimmed terminal regions and include a trailing newline when non-empty. The `viewport`
/// and `scrollback` fields contain the same trimmed rows as compact spans: string spans use
/// [`TerminalState::default_style`], and styled spans reference [`TerminalState::styles`] by index.
/// Style indexes are produced by Betamax and are only meaningful within the containing
/// `TerminalState`.
#[derive(Debug, Clone, Serialize)]
pub struct TerminalState {
    /// `[columns, rows]` terminal grid size.
    pub size: [u16; 2],
    /// Total terminal rows reported by libghostty-vt, including scrollback and viewport rows.
    pub total_rows: usize,
    /// Number of scrollback rows currently available.
    pub scrollback_rows: usize,
    /// Terminal title, omitted from JSON when empty.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Terminal working directory, omitted from JSON when empty.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub working_directory: String,
    /// Cursor position and visibility.
    pub cursor: StateCursor,
    /// Fully expanded default style.
    ///
    /// Rows omit this style by storing plain strings instead of styled spans.
    pub default_style: StateStyle,
    /// Non-default styles referenced by styled spans.
    ///
    /// Each entry is a delta from [`TerminalState::default_style`] to keep state JSON compact.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<StateStyle>,
    /// Plain text for the trimmed viewport, including a trailing newline when non-empty.
    pub viewport_text: String,
    /// Plain text for trimmed scrollback, including a trailing newline when non-empty.
    pub scrollback_text: String,
    /// Styled spans for the trimmed viewport.
    ///
    /// Trailing empty rows are omitted because they are implied by [`TerminalState::size`].
    pub viewport: Vec<StateRow>,
    /// Styled spans for trimmed scrollback.
    ///
    /// Trailing empty rows are omitted. Leading empty rows are preserved because they can be
    /// meaningful terminal layout.
    pub scrollback: Vec<StateRow>,
}

/// Cursor metadata in a [`TerminalState`].
#[derive(Debug, Clone, Serialize)]
pub struct StateCursor {
    /// Zero-based cursor column.
    pub x: u16,
    /// Zero-based cursor row within the viewport.
    pub y: u16,
    /// Whether libghostty-vt reports the cursor as visible.
    pub visible: bool,
}

/// A compact row of terminal text spans.
///
/// Rows preserve internal spaces and style boundaries. Empty rows are represented by an empty
/// vector.
pub type StateRow = Vec<StateSpan>;

/// Text span used by [`StateRow`].
///
/// Plain text spans use [`TerminalState::default_style`]. Styled spans reference an entry in
/// [`TerminalState::styles`] by index. Adjacent cells with identical style are merged before spans
/// are serialized, so a span may contain more than one terminal cell.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum StateSpan {
    /// Text using the default style.
    Text(String),
    /// Text using `TerminalState::styles[index]`.
    Styled(String, usize),
}

/// Serializable terminal style.
///
/// In [`TerminalState::default_style`], fields are expanded so the default is self-describing. In
/// [`TerminalState::styles`], fields are deltas from the default and omitted when unchanged.
///
/// Boolean fields default to false and are omitted from JSON when false. The empty underline value
/// represents `none` and is also omitted. Color fields are lowercase `#RRGGBB` strings when
/// present.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct StateStyle {
    /// Foreground color as `#RRGGBB`; omitted for default-style deltas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    /// Background color as `#RRGGBB`; omitted for default-style deltas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    /// Bold text attribute.
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    /// Italic text attribute.
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
    /// Faint text attribute.
    #[serde(skip_serializing_if = "is_false")]
    pub faint: bool,
    /// Blink text attribute.
    #[serde(skip_serializing_if = "is_false")]
    pub blink: bool,
    /// Inverse text attribute after libghostty-vt style/color resolution.
    #[serde(skip_serializing_if = "is_false")]
    pub inverse: bool,
    /// Invisible text attribute.
    #[serde(skip_serializing_if = "is_false")]
    pub invisible: bool,
    /// Strikethrough text attribute.
    #[serde(skip_serializing_if = "is_false")]
    pub strikethrough: bool,
    /// Overline text attribute.
    #[serde(skip_serializing_if = "is_false")]
    pub overline: bool,
    /// Underline style such as `single`, `double`, `curly`, `dotted`, `dashed`, or `none`.
    #[serde(skip_serializing_if = "is_none_underline")]
    pub underline: String,
}

/// Fully resolved style used before terminal-state JSON compaction.
///
/// This type deliberately stores colors as hex strings rather than `RgbColor` values because it is
/// the comparison form used for snapshot serialization. The public [`StateStyle`] type has optional
/// fields so it can represent deltas; this internal type is always complete.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct FullStateStyle {
    /// Fully resolved foreground color.
    fg: String,
    /// Fully resolved background color.
    bg: String,
    /// Fully resolved bold flag.
    bold: bool,
    /// Fully resolved italic flag.
    italic: bool,
    /// Fully resolved faint flag.
    faint: bool,
    /// Fully resolved blink flag.
    blink: bool,
    /// Fully resolved inverse flag.
    inverse: bool,
    /// Fully resolved invisible flag.
    invisible: bool,
    /// Fully resolved strikethrough flag.
    strikethrough: bool,
    /// Fully resolved overline flag.
    overline: bool,
    /// Fully resolved underline value.
    underline: String,
}

/// One normalized cell before adjacent cells are compacted into spans.
///
/// Empty libghostty-vt cells are represented as a single space here so span compaction can preserve
/// internal layout while still trimming trailing blank cells at the row boundary.
#[derive(Debug, Clone)]
pub(super) struct StateCellSnapshot {
    /// Cell text. Empty libghostty-vt cells have already been normalized to a single space.
    pub(super) text: String,
    /// Fully resolved style for this cell.
    pub(super) style: FullStateStyle,
}

/// Span produced after row compaction but before style-table interning.
///
/// Keeping the full style attached until the end lets the compactor merge adjacent text with the
/// same style before deciding whether the style should become a default plain-text span or a
/// non-default style-table reference.
#[derive(Debug, Clone)]
pub(super) enum PendingStateSpan {
    /// Text associated with a full style before style-table interning.
    Styled(String, FullStateStyle),
}

/// Intern table for non-default terminal-state styles.
///
/// The state JSON format intentionally emits rows as strings or `[text, style_index]` tuples. This
/// table owns the conversion from fully resolved styles into compact deltas and guarantees repeated
/// style runs share the same index.
pub(super) struct StyleTable {
    /// Full default style; matching spans become plain string spans.
    default: FullStateStyle,
    /// Reverse lookup from full style to compact style index.
    indexes: HashMap<FullStateStyle, usize>,
    /// Compact style deltas emitted in terminal-state JSON.
    pub(super) styles: Vec<StateStyle>,
}

impl StyleTable {
    /// Create an empty style table with a known default style.
    pub(super) fn new(default: FullStateStyle) -> Self {
        Self {
            default,
            indexes: HashMap::new(),
            styles: Vec::new(),
        }
    }

    /// Convert pending rows into JSON rows while interning non-default styles.
    pub(super) fn intern_pending_rows(
        &mut self,
        rows: Vec<Vec<PendingStateSpan>>,
    ) -> Vec<StateRow> {
        rows.into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|span| match span {
                        PendingStateSpan::Styled(text, style) => {
                            if style == self.default {
                                StateSpan::Text(text)
                            } else {
                                StateSpan::Styled(text, self.intern(style))
                            }
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// Return the compact style index for a full style, inserting it if needed.
    fn intern(&mut self, style: FullStateStyle) -> usize {
        if let Some(index) = self.indexes.get(&style) {
            return *index;
        }
        let index = self.styles.len();
        self.styles.push(style_delta(&style, &self.default));
        self.indexes.insert(style, index);
        index
    }
}

/// Join pending rows into plain text.
///
/// Empty input returns an empty string. Non-empty input always ends with a newline, matching common
/// terminal snapshot expectations and making line-oriented snapshot diffs easier to read.
pub(super) fn pending_rows_text(rows: &[Vec<PendingStateSpan>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut text = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|span| match span {
                    PendingStateSpan::Styled(text, _) => text.as_str(),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text
}

/// Drop empty rows from the bottom of a state region.
///
/// Leading empty rows are preserved because they can be meaningful in scrollback or viewport
/// layout.
pub(super) fn trim_empty_rows(mut rows: Vec<Vec<PendingStateSpan>>) -> Vec<Vec<PendingStateSpan>> {
    while rows.last().is_some_and(Vec::is_empty) {
        rows.pop();
    }
    rows
}

/// Build the fully resolved default style for terminal-state JSON.
///
/// The default style is expanded in JSON even when every row uses it. That makes state files
/// self-describing and gives snapshot tests a stable baseline for interpreting plain string spans.
pub(super) fn default_style(theme: &RenderTheme) -> FullStateStyle {
    FullStateStyle {
        fg: hex_color(theme.foreground),
        bg: hex_color(theme.background),
        bold: false,
        italic: false,
        faint: false,
        blink: false,
        inverse: false,
        invisible: false,
        strikethrough: false,
        overline: false,
        underline: "none".to_string(),
    }
}

/// Convert libghostty-vt style bits and resolved colors into a full state style.
pub(super) fn full_state_style(
    style: Style,
    foreground: RgbColor,
    background: RgbColor,
) -> FullStateStyle {
    FullStateStyle {
        fg: hex_color(foreground),
        bg: hex_color(background),
        bold: style.bold,
        italic: style.italic,
        faint: style.faint,
        blink: style.blink,
        inverse: style.inverse,
        invisible: style.invisible,
        strikethrough: style.strikethrough,
        overline: style.overline,
        underline: match style.underline {
            Underline::None => "none",
            Underline::Single => "single",
            Underline::Double => "double",
            Underline::Curly => "curly",
            Underline::Dotted => "dotted",
            Underline::Dashed => "dashed",
            _ => "unknown",
        }
        .to_string(),
    }
}

/// Convert a full style into an expanded serializable style.
pub(super) fn state_style(style: &FullStateStyle) -> StateStyle {
    StateStyle {
        fg: Some(style.fg.clone()),
        bg: Some(style.bg.clone()),
        bold: style.bold,
        italic: style.italic,
        faint: style.faint,
        blink: style.blink,
        inverse: style.inverse,
        invisible: style.invisible,
        strikethrough: style.strikethrough,
        overline: style.overline,
        underline: style.underline.clone(),
    }
}

/// Merge adjacent cells with identical styles and trim trailing whitespace-only cells.
///
/// The resulting row is compact without losing internal spaces or style changes. Empty rows become
/// an empty vector, which serializes compactly and can later be trimmed from region ends.
pub(super) fn compact_row(cells: Vec<StateCellSnapshot>) -> Vec<PendingStateSpan> {
    let last_visible = cells
        .iter()
        .rposition(|cell| !cell.text.chars().all(char::is_whitespace));
    let Some(last_visible) = last_visible else {
        return Vec::new();
    };

    let mut pending = Vec::new();
    let mut current_text = String::new();
    let mut current_style = cells[last_visible].style.clone();

    for cell in cells.into_iter().take(last_visible + 1) {
        if current_text.is_empty() {
            current_style = cell.style;
            current_text.push_str(&cell.text);
            continue;
        }
        if cell.style == current_style {
            current_text.push_str(&cell.text);
        } else {
            pending.push(PendingStateSpan::Styled(
                std::mem::take(&mut current_text),
                current_style,
            ));
            current_style = cell.style;
            current_text.push_str(&cell.text);
        }
    }
    if !current_text.is_empty() {
        pending.push(PendingStateSpan::Styled(current_text, current_style));
    }
    pending
}

/// Convert a full style into a delta from the default style.
///
/// Boolean fields are emitted only when they differ and are true. Color fields are emitted only
/// when changed. The default underline is represented by an empty string so serde can omit it.
///
/// This representation cannot express a false override of a true default boolean. That is
/// acceptable for the current defaults because all boolean attributes default to false. If Betamax
/// ever supports a non-false default text attribute, this function and [`StateStyle`] should grow
/// explicit tri-state fields.
fn style_delta(style: &FullStateStyle, default: &FullStateStyle) -> StateStyle {
    StateStyle {
        fg: (style.fg != default.fg).then(|| style.fg.clone()),
        bg: (style.bg != default.bg).then(|| style.bg.clone()),
        bold: style.bold != default.bold && style.bold,
        italic: style.italic != default.italic && style.italic,
        faint: style.faint != default.faint && style.faint,
        blink: style.blink != default.blink && style.blink,
        inverse: style.inverse != default.inverse && style.inverse,
        invisible: style.invisible != default.invisible && style.invisible,
        strikethrough: style.strikethrough != default.strikethrough && style.strikethrough,
        overline: style.overline != default.overline && style.overline,
        underline: if style.underline != default.underline {
            style.underline.clone()
        } else {
            String::new()
        },
    }
}

/// Serde helper for omitting false boolean fields.
fn is_false(value: &bool) -> bool {
    !*value
}

/// Serde helper for omitting default underline fields.
fn is_none_underline(value: &str) -> bool {
    value == "none" || value.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_state_uses_plain_text_for_default_spans() {
        let default = test_full_style("#dddddd", "#102040");
        let accent = test_full_style("#5a56e0", "#102040");
        let pending = vec![compact_row(vec![
            StateCellSnapshot {
                text: ">".to_string(),
                style: accent.clone(),
            },
            StateCellSnapshot {
                text: " ".to_string(),
                style: accent,
            },
            StateCellSnapshot {
                text: "e".to_string(),
                style: default.clone(),
            },
            StateCellSnapshot {
                text: "c".to_string(),
                style: default.clone(),
            },
            StateCellSnapshot {
                text: "h".to_string(),
                style: default.clone(),
            },
            StateCellSnapshot {
                text: "o".to_string(),
                style: default.clone(),
            },
            StateCellSnapshot {
                text: " ".to_string(),
                style: default.clone(),
            },
        ])];
        let mut table = StyleTable::new(default);
        let rows = table.intern_pending_rows(pending);

        assert_eq!(
            table.styles,
            vec![StateStyle {
                fg: Some("#5a56e0".to_string()),
                ..StateStyle::default()
            }]
        );
        let json = serde_json::to_value(&rows).unwrap();
        assert_eq!(json, serde_json::json!([[["> ", 0], "echo"]]));
    }

    fn test_full_style(fg: &str, bg: &str) -> FullStateStyle {
        FullStateStyle {
            fg: fg.to_string(),
            bg: bg.to_string(),
            bold: false,
            italic: false,
            faint: false,
            blink: false,
            inverse: false,
            invisible: false,
            strikethrough: false,
            overline: false,
            underline: "none".to_string(),
        }
    }
}
