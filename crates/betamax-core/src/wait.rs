//! Wait command matching.
//!
//! Waits are expressed in tape terms (`Line` or `Screen`, contains or regex). This module converts
//! those terms into terminal-state text and match results while keeping the runner's wait loop
//! focused on timing and frame capture.

use miette::miette;
use regex::Regex;

use crate::runner::TerminalSession;
use crate::tape::{WaitPattern, WaitTarget};
use crate::Result;

/// Return the terminal text inspected by a wait command.
///
/// `Screen` uses the full visible viewport text. `Line` uses the row containing the cursor, which
/// matches the usual "wait for prompt" behavior without requiring a tape to know the full screen.
pub(crate) fn wait_target_text(
    terminal: &mut impl TerminalSession,
    target: WaitTarget,
) -> Result<String> {
    match target {
        WaitTarget::Screen => terminal.screen_text(),
        WaitTarget::Line => {
            let state = terminal.terminal_state()?;
            Ok(state
                .viewport_text
                .lines()
                .nth(usize::from(state.cursor.y))
                .unwrap_or_default()
                .to_string())
        }
    }
}

/// Evaluate a wait pattern against text.
///
/// Regex patterns are compiled on each call. This keeps the stored tape representation simple; if
/// wait matching becomes a performance issue, precompiled patterns can be introduced in runner
/// settings without changing the tape AST.
pub(crate) fn wait_pattern_matches(pattern: &WaitPattern, text: &str) -> Result<bool> {
    match pattern {
        WaitPattern::Contains(pattern) => Ok(text.contains(pattern)),
        WaitPattern::Regex(pattern) => Ok(Regex::new(pattern)
            .map_err(|error| miette!("invalid wait regex `{pattern}`: {error}"))?
            .is_match(text)),
    }
}

/// Human-readable name for wait timeout errors.
pub(crate) fn wait_target_name(target: WaitTarget) -> &'static str {
    match target {
        WaitTarget::Line => "current line",
        WaitTarget::Screen => "screen",
    }
}

/// Human-readable pattern description for wait timeout errors.
pub(crate) fn wait_pattern_name(pattern: &WaitPattern) -> String {
    match pattern {
        WaitPattern::Contains(pattern) => format!("text `{pattern}`"),
        WaitPattern::Regex(pattern) => format!("regex /{pattern}/"),
    }
}

/// Accept either raw regex text or a slash-delimited regex string.
///
/// `Set WaitPattern "/>$/"` and `Set WaitPattern ">$"` therefore mean the same thing, which makes
/// default configuration less surprising than requiring one specific spelling.
pub(crate) fn regex_source(pattern: &str) -> String {
    if pattern.len() >= 2 && pattern.starts_with('/') && pattern.ends_with('/') {
        pattern[1..pattern.len() - 1].to_string()
    } else {
        pattern.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_patterns_match_text_or_regex() {
        assert!(wait_pattern_matches(&WaitPattern::Contains("ok".to_string()), "api ok").unwrap());
        assert!(wait_pattern_matches(&WaitPattern::Regex("a.*ok".to_string()), "api ok").unwrap());
        assert!(!wait_pattern_matches(&WaitPattern::Contains("no".to_string()), "api ok").unwrap());
    }

    #[test]
    fn regex_source_accepts_slashes_or_raw_pattern() {
        assert_eq!(regex_source("/>$/"), ">$");
        assert_eq!(regex_source(">$"), ">$");
    }
}
