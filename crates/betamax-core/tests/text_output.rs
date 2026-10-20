//! `Output demo.txt` writes the same bytes as VHS for the same tape.
//!
//! `fixtures/vhs-text-output.txt` was produced by VHS 0.12.1 from this tape, with the snapshots
//! VHS writes for the tape header removed (Betamax skips them). The `Hide`/`Show` pairs keep every
//! snapshot after the PTY has settled, so neither tool races shell output.

use betamax_core::{RunOptions, Runner, Tape};
use indoc::formatdoc;

#[test]
fn text_output_matches_vhs_golden() {
    let path = std::env::temp_dir().join(format!("betamax-text-output-{}.txt", std::process::id()));
    let tape = Tape::parse(&formatdoc! {r#"
        Output {path}
        Set Shell bash
        Set Columns 40
        Set Rows 6
        Set TypingSpeed 10ms
        Hide
        Type "export PS1='> '; clear"
        Enter
        Sleep 500ms
        Show
        Type "printf 'wide: %s\n' '你好世界'"
        Hide
        Enter
        Sleep 500ms
        Show
        Type "printf '\e[?1049h\e[Halternate screen'"
        Hide
        Enter
        Sleep 500ms
        Show
        Hide
        Type "printf '\e[?1049l'; clear"
        Enter
        Sleep 500ms
        Show
        "#,
        path = path.display(),
    })
    .unwrap();

    Runner::new(RunOptions {
        quiet: true,
        publish: false,
    })
    .run_artifacts(&tape)
    .unwrap();

    let actual = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(actual, include_str!("fixtures/vhs-text-output.txt"));
}
