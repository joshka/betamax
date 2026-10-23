//! A `Set TypingSpeed` after runtime commands applies from that point, as in VHS.

use std::time::{Duration, Instant};

use betamax_core::{Runner, Tape};
use indoc::formatdoc;

#[test]
fn runtime_typing_speed_applies_to_later_type_commands() {
    let baseline = run_with_late_typing_speed("0ms");
    let slowed = run_with_late_typing_speed("300ms");

    // Five characters at 300ms each; allow slack for PTY and shell timing noise.
    assert!(
        slowed >= baseline + Duration::from_millis(1000),
        "baseline {baseline:?}, slowed {slowed:?}"
    );
}

fn run_with_late_typing_speed(speed: &str) -> Duration {
    let tape = Tape::parse(&formatdoc! {r#"
        Set Shell "bash"
        Set TypingSpeed 0ms
        Set WaitTimeout 5s
        Hide
        Type "echo $((40"
        Set TypingSpeed {speed}
        Type "+2))"
        Enter
        Wait+Screen "42"
        "#})
    .unwrap();

    let started = Instant::now();
    Runner::new().run_artifacts(&tape).unwrap();
    started.elapsed()
}
