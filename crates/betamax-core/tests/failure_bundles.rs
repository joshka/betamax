//! A failed tape leaves a bundle that explains the failure without a rerun.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use betamax_core::{RunOptions, Runner, Tape};
use serde_json::Value;

/// Run `source` with bundles under a fresh directory; return the error and the one bundle.
fn fail(name: &str, source: &str) -> (String, PathBuf) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("failure-bundles/{name}-{stamp}"));
    let tape = Tape::parse(source).unwrap();
    let error = Runner::new(RunOptions {
        quiet: true,
        publish: false,
    })
    .with_failure_dir(&root)
    .run_artifacts(&tape)
    .unwrap_err();
    let bundles: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(bundles.len(), 1, "{bundles:?}");
    let bundle = bundles.into_iter().next().unwrap();
    let error = error.to_string();
    assert!(
        error.ends_with(&format!("\nfailure bundle: {}", bundle.display())),
        "{error}"
    );
    (error, bundle)
}

fn report(bundle: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(bundle.join("report.json")).unwrap()).unwrap()
}

fn read(bundle: &Path, file: &str) -> String {
    std::fs::read_to_string(bundle.join(file)).unwrap()
}

#[test]
fn wait_timeout_reports_line_elapsed_time_and_awaited_pattern() {
    let (error, bundle) = fail(
        "timeout",
        r#"
Set Shell "bash"
Set TypingSpeed 0

Type "printf 'loading...\n'"
Enter
Wait+Screen@500ms "Ready"
"#,
    );
    assert!(
        error.contains("line 7: `Wait+Screen@500ms \"Ready\"` failed after"),
        "{error}"
    );

    let report = report(&bundle);
    assert_eq!(report["command"]["number"], 5);
    assert_eq!(report["command"]["location"], "line 7");
    assert_eq!(report["wait"]["target"], "screen");
    assert_eq!(report["wait"]["pattern"]["text"], "Ready");
    assert_eq!(report["wait"]["timeout_ms"], 500);
    assert!(report["elapsed_ms"].as_u64().unwrap() >= 500, "{report}");
    assert_eq!(report["shell"]["status"], "running");
    let recent: Vec<_> = report["recent_commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["location"].as_str().unwrap())
        .collect();
    assert_eq!(recent, ["line 2", "line 3", "line 5", "line 6", "line 7"]);

    let state: Value = serde_json::from_str(&read(&bundle, "actual.json")).unwrap();
    assert!(state["viewport_text"]
        .as_str()
        .unwrap()
        .contains("\nloading..."));
    assert!(bundle.join("actual.png").is_file());
    let text = read(&bundle, "report.txt");
    assert!(
        text.starts_with("Tape command 5 of 5 failed at line 7 after"),
        "{text}"
    );
    assert!(
        text.contains("Waiting:  screen to match text \"Ready\" within 0.50s"),
        "{text}"
    );
    assert!(
        text.contains("> line 7  Wait+Screen@500ms \"Ready\""),
        "{text}"
    );
}

#[test]
fn failed_assertion_saves_the_screen_it_checked() {
    let (error, bundle) = fail(
        "assertion",
        r#"
Set Shell "bash"
Set TypingSpeed 0
Type "printf 'total: 41\n'" Enter
Wait+Screen "total:"
AssertText "total: 42"
"#,
    );
    assert!(
        error.contains("expected visible text \"total: 42\""),
        "{error}"
    );

    let report = report(&bundle);
    assert_eq!(report["command"]["location"], "line 6");
    assert_eq!(report["command"]["text"], "AssertText \"total: 42\"");
    assert!(report["wait"].is_null());
    for file in [
        "actual.json",
        "actual.png",
        "cells.json",
        "diff.txt",
        "report.txt",
    ] {
        assert!(bundle.join(file).is_file(), "missing {file}");
    }
    let text = read(&bundle, "report.txt");
    assert!(text.contains("\n  total: 41\n"), "{text}");
}

#[test]
fn failure_message_keeps_the_underlying_cause() {
    let (error, bundle) = fail(
        "cause",
        r#"
Set Shell "bash"
AssertState betamax-test-missing-baseline.json
"#,
    );
    assert!(error.contains("failed to read baseline"), "{error}");
    assert!(error.contains("No such file"), "{error}");
    assert!(read(&bundle, "diff.txt").contains("No such file"));
}

#[test]
fn missing_required_program_writes_a_report_without_starting_the_shell() {
    let (error, bundle) = fail(
        "require",
        r#"
Require betamax-test-missing-program
Type "betamax-test-missing-program"
"#,
    );
    assert!(
        error.contains("line 2: `Require betamax-test-missing-program` failed"),
        "{error}"
    );
    assert!(error.contains("not found on PATH"), "{error}");

    let report = report(&bundle);
    assert_eq!(report["shell"]["status"], "not_started");
    assert!(report["elapsed_ms"].is_null());
    assert!(report["files"].get("screenshot").is_none());
    assert!(!bundle.join("actual.png").exists());
    assert!(read(&bundle, "report.txt").contains("Shell:    not started"));
}

#[test]
fn shell_exit_fails_a_wait_without_running_out_its_timeout() {
    let started = Instant::now();
    let (error, bundle) = fail(
        "exit",
        r#"
Set Shell "bash"
Set TypingSpeed 0
Type "exit 3" Enter
Wait+Screen@20s "Ready"
"#,
    );
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(
        error.contains("shell exited with status 3 before screen matched text `Ready`"),
        "{error}"
    );
    let report = report(&bundle);
    assert_eq!(report["shell"]["status"], "exited");
    assert_eq!(report["shell"]["code"], 3);
    assert!(bundle.join("actual.png").is_file());
}
