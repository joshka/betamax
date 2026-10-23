//! Snapshot a terminal program's screen with `insta`, using both a file and an inline snapshot.
//!
//! The tape also writes a screenshot beside the snapshot (ignored by git) so a reviewer can look at
//! the screen while running `cargo insta review`.

use betamax_core::{Runner, Tape};
use indoc::formatdoc;

#[test]
fn selection_moves_down() {
    // Swap in your own program here, such as `env!("CARGO_BIN_EXE_my-app")`.
    let app = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/task_list.py");
    let source = formatdoc! {r#"
        Set Shell "python3 {app}"
        Set Columns 30
        Set Rows 8
        Wait+Screen "> write tape"
        Type "j"
        Wait+Screen "> run test"
        Screenshot tests/snapshots/selection_moves_down.png
    "#};
    let tape = Tape::parse(&source).unwrap();
    let artifacts = Runner::new().run_artifacts(&tape).unwrap();
    let state = artifacts.final_state.unwrap();

    // The styled rows are long, so they go to a file in `tests/snapshots/`. Styles are written next
    // to their text, so unchanged rows stay unchanged in the diff.
    insta::assert_json_snapshot!(state.styled_viewport());

    // The text is short, so it stays inline next to the tape that produced it. Insta ignores
    // indentation shared by every line, so this misses a whole-screen shift; the file catches it.
    insta::assert_snapshot!(state.viewport_text, @"
    Tasks
      write tape
    > run test
      review snapshot

    j move · q quit
    ");
}
