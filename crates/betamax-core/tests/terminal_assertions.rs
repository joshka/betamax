//! Assertions operate on settled cells, not transient matching output.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use betamax_core::{RunOptions, Runner, Tape};

fn run(commands: &str) -> betamax_core::Result<betamax_core::RunArtifacts> {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terminal_events.py");
    let tape = Tape::parse(&format!(
        r#"
        Set Shell "python3 {}"
        Set Width 400
        Set Height 240
        Set Padding 0
        Set FontSize 16
        Set TypingSpeed 0ms
        Set WaitTimeout 2s
        Hide
        Wait+Screen@2s "ready"
        {commands}
    "#,
        fixture.display()
    ))?;
    Runner::new(RunOptions {
        quiet: true,
        publish: false,
    })
    .run_artifacts(&tape)
}

#[test]
fn asserts_unicode_cells_full_attributes_and_explicit_state_baseline() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/terminal-input/assertions");
    std::fs::create_dir_all(&root).unwrap();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let baseline = root.join(format!("{stamp}.json"));
    run(&format!(
        r##"
        Type "h"
        AssertText "A界"
        AssertCells 0 0 '["A", "界", " ", "é"]'
        AssertStyle 0 0 4 '{{"fg":"#ff0000","bold":true}}'
        AssertStyle 4 0 1 '{{}}'
        AssertAbsent@50ms "FORBIDDEN"
        State {}
        AssertState {}
        Type "q"
    "##,
        baseline.display(),
        baseline.display()
    ))
    .unwrap();
}

#[test]
fn transient_positive_output_does_not_satisfy_checkpoint() {
    let error = run(r#"Type "d" AssertText "TRANSIENT""#).unwrap_err();
    let detail = format!("{error:?}");
    assert!(detail.contains("expected visible text"), "{detail}");
    assert!(detail.contains("FINAL"), "{detail}");
    check_diagnostics(&error);
}

#[test]
fn bounded_absence_fails_on_later_visible_output() {
    let error = run(r#"Type "b" Wait+Screen "armed" AssertAbsent@500ms "FORBIDDEN""#).unwrap_err();
    let detail = format!("{error:?}");
    assert!(detail.contains("to be absent"), "{detail}");
    check_diagnostics(&error);
}

#[test]
fn cell_mismatch_reports_coordinates_expected_actual_and_artifacts() {
    let error = run(r#"Type "h" AssertCells 1 0 '["X"]'"#).unwrap_err();
    let detail = format!("{error:?}");
    assert!(detail.contains("cell (1, 0)"), "{detail}");
    assert!(detail.contains("expected text"), "{detail}");
    assert!(detail.contains("界"), "{detail}");
    check_diagnostics(&error);
}

#[test]
fn continuous_output_cannot_extend_assertion_settling_forever() {
    for input in [r#"Type "o""#, r#"Type@50ms "o""#, r#"Type "o" Sleep 50ms"#] {
        let started = std::time::Instant::now();
        let error = run(&format!(r#"{input} AssertText "armed""#)).unwrap_err();
        assert!(format!("{error:?}").contains("did not settle"));
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }
}

#[test]
fn continuous_output_cannot_extend_output_wait_forever() {
    let started = std::time::Instant::now();
    let error = run(r#"Show Type "o" Wait+Screen@2s "never emitted""#).unwrap_err();
    assert!(format!("{error:?}").contains("timed out waiting"));
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

fn check_diagnostics(error: &betamax_core::Error) {
    let summary = error.to_string();
    let path = summary
        .split("diagnostics: ")
        .nth(1)
        .expect("artifact directory in error");
    for file in ["actual.json", "actual.png", "cells.json", "diff.txt"] {
        assert!(
            PathBuf::from(path).join(file).is_file(),
            "missing {file} in {path}"
        );
    }
}
