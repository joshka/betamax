//! The tape shell starts in the directory Betamax was run from.

use std::path::PathBuf;

use betamax_core::{Runner, Tape};
use indoc::formatdoc;

#[test]
fn shell_starts_in_current_directory() {
    // Cargo runs integration tests from the package root.
    let expected = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .unwrap();
    let tape = Tape::parse(&formatdoc! {r#"
        Set Shell "bash"
        Set Width 1200
        Set Height 240
        Set TypingSpeed 0ms
        Set WaitTimeout 2s
        Hide
        Type '[ "$(pwd -P)" = "{expected}" ] && cwd=same || cwd=different; echo "cwd:$cwd"'
        Enter
        Wait+Screen /cwd:(same|different)/
        AssertText "cwd:same"
        "#,
        expected = expected.display(),
    })
    .unwrap();
    Runner::new().run_artifacts(&tape).unwrap();
}
