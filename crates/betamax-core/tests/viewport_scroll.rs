//! `ScrollUp`/`ScrollDown` move the viewport, and keyboard input returns it to the bottom.

use betamax_core::{Runner, Tape};
use indoc::indoc;

fn run(source: &str) -> betamax_core::ghostty::TerminalState {
    let tape = Tape::parse(source).unwrap();
    Runner::new()
        .run_artifacts(&tape)
        .unwrap()
        .final_state
        .unwrap()
}

const SETUP: &str = indoc! {r#"
    Set Shell "bash"
    Set Width 600
    Set Height 300
    Set TypingSpeed 0ms
    Set WaitTimeout 3s
    Hide
    Type "clear; seq -f 'row %02g' 1 40"
    Enter
    Wait+Screen "row 40"
"#};

#[test]
fn scroll_up_shows_scrollback_and_scroll_down_returns() {
    let state = run(&format!("{SETUP}ScrollUp 30\nAssertText \"row 05\"\n"));
    assert!(state.viewport_text.contains("row 05"));
    assert!(!state.viewport_text.contains("row 40"));

    let state = run(&format!("{SETUP}ScrollUp@0ms 30\nScrollDown 30\n"));
    assert!(state.viewport_text.contains("row 40"));
}

#[test]
fn keyboard_input_returns_the_viewport_to_the_bottom() {
    let state = run(&format!(
        "{SETUP}ScrollUp 30\nType \"echo typed-$((1+1))\"\nEnter\nWait+Screen \"typed-2\"\n"
    ));
    assert!(state.viewport_text.contains("typed-2"));
    assert!(!state.viewport_text.contains("row 05"));
}
