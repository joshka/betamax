//! `Set Columns` and `Set Rows` give the shell exactly the requested terminal grid.

use betamax_core::{Runner, Tape};
use indoc::indoc;

#[test]
fn shell_sees_requested_columns_and_rows() {
    let tape = Tape::parse(indoc! {r#"
        Set Shell "bash"
        Set Columns 57
        Set Rows 13
        Set Padding 7
        Set TypingSpeed 0ms
        Set WaitTimeout 2s
        Hide
        Type 'echo "size:$(stty size | tr " " x)"'
        Enter
        Wait+Screen /size:[0-9]/
        AssertText "size:13x57"
        "#})
    .unwrap();
    Runner::new().run_artifacts(&tape).unwrap();
}
