//! Wait regexes reach the terminal as waits, not as key presses.

use betamax_core::{Runner, Tape};

/// Run `commands` in bash after it prints `v1.23 ready` following a short delay.
fn run_after_delayed_version(commands: &str) {
    let tape = Tape::parse(&format!(
        r#"
        Set Shell "bash"
        Set Width 800
        Set Height 240
        Set TypingSpeed 0ms
        Set WaitTimeout 2s
        Hide
        Type "sleep 0.3; printf 'v%s.%s %s\n' 1 23 ready"
        Enter
        {commands}
        "#
    ))
    .unwrap();
    Runner::new().run_artifacts(&tape).unwrap();
}

#[test]
fn wait_regex_with_plus_waits_without_typing() {
    // `+` once turned `/v[0-9]+\.[0-9]+/` into the key `/`, which landed on the prompt.
    run_after_delayed_version(
        r#"
        Wait+Screen "/v[0-9]+\.[0-9]+/"
        Wait+Line
        AssertText "v1.23"
        AssertAbsent "/"
        "#,
    );
}

#[test]
fn unquoted_wait_regex_keeps_spaces() {
    // Shell-style splitting once ended this regex at the space and rejected `ready/`.
    run_after_delayed_version(
        r#"
        Wait+Screen /v[0-9]+\.[0-9]+ ready/
        AssertText "v1.23 ready"
        "#,
    );
}
