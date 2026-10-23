//! Wait regexes reach the terminal as waits, not as key presses.

use betamax_core::{RunOptions, Runner, Tape};

#[test]
fn wait_regex_with_plus_waits_without_typing() {
    // `+` once turned `/v[0-9]+\.[0-9]+/` into the key `/`, which landed on the prompt.
    let tape = Tape::parse(
        r#"
        Set Shell "bash"
        Set Width 800
        Set Height 240
        Set TypingSpeed 0ms
        Set WaitTimeout 2s
        Hide
        Type "sleep 0.3; printf 'v%s.%s\n' 1 23"
        Enter
        Wait+Screen "/v[0-9]+\.[0-9]+/"
        Wait+Line
        AssertText "v1.23"
        AssertAbsent "/"
        "#,
    )
    .unwrap();
    Runner::new(RunOptions {
        quiet: true,
        publish: false,
    })
    .run_artifacts(&tape)
    .unwrap();
}
