//! Tapes without `Set Shell` run in bash, like VHS, whatever the host user's `$SHELL` is.

use betamax_core::{Runner, Tape};
use indoc::indoc;

#[test]
fn tape_without_set_shell_runs_bash() {
    let tape = Tape::parse(indoc! {r#"
        Set Width 1200
        Set Height 240
        Set TypingSpeed 0ms
        Set WaitTimeout 2s
        Hide
        Type 'echo "shell:${BASH_VERSION:+bash}"'
        Enter
        Wait+Screen /shell:bash/
        "#})
    .unwrap();
    Runner::new().run_artifacts(&tape).unwrap();
}
