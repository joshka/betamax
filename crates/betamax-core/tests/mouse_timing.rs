//! A drag made of many short mouse commands records the time the tape gives it.

use std::fmt::Write as _;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use betamax_core::{Runner, Tape};
use image::AnimationDecoder;
use indoc::formatdoc;

const MOVES: u64 = 60;
const FRAME_MS: u64 = 20;
const SLEEP_MS: u64 = 15;

/// Each `MouseMove` holds one capture interval and each `Sleep` its duration, however long a
/// render takes. Before mouse commands kept a schedule, render time at this size added several
/// milliseconds per command, so a smooth drag ran far longer than the tape said.
#[test]
#[ignore = "timing-sensitive; run in release with `mise run timing-test`"]
fn many_short_mouse_moves_keep_tape_time() {
    let mut drag = String::new();
    for column in 0..MOVES {
        writeln!(drag, "MouseMove {} 10", column + 2).unwrap();
        writeln!(drag, "Sleep {SLEEP_MS}ms").unwrap();
    }
    assert_drag_time("moves", &drag, MOVES * (FRAME_MS + SLEEP_MS));
}

/// `MouseMove@` replaces the capture-interval wait, so each move holds exactly its delay.
#[test]
#[ignore = "timing-sensitive; run in release with `mise run timing-test`"]
fn mouse_move_delay_replaces_the_frame_wait() {
    let step_ms = FRAME_MS + SLEEP_MS;
    let drag: String = (0..MOVES)
        .map(|column| format!("MouseMove@{step_ms}ms {} 10\n", column + 2))
        .collect();
    assert_drag_time("delayed", &drag, MOVES * step_ms);
}

/// A glide takes exactly its `duration:` and still reaches every frame's cell; at 50 FPS a
/// 60-cell glide over 1.2 s reports one new cell per frame.
#[test]
#[ignore = "timing-sensitive; run in release with `mise run timing-test`"]
fn mouse_move_glides_over_its_duration() {
    assert_drag_time(
        "glide",
        &format!(
            "MouseMove {} 10 duration: {}ms",
            MOVES + 1,
            MOVES * FRAME_MS
        ),
        MOVES * FRAME_MS,
    );
}

/// Record `drag` from cell (1, 10) with the left button held and check that it reaches cell
/// `MOVES + 1` one report per cell, in `nominal` ms of recorded time.
fn assert_drag_time(name: &str, drag: &str, nominal: u64) {
    let root = std::env::temp_dir().join(format!(
        "betamax-mouse-timing-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let webp = root.join("drag.webp");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mouse_trail.py");
    let tape = Tape::parse(&formatdoc! {r#"
        Require python3
        Output "{webp}"
        Set Shell "python3 {fixture}"
        Set Width 2400
        Set Height 1350
        Set Framerate 50
        Hide
        Wait+Screen@5s "ready"
        MouseMove 1 10
        MouseDown Left
        Show
        {drag}
        Hide
        # The press and the opening move also report, so the count is two more than the drag.
        Wait+Screen@5s "moves   {moves}"
        "#,
        webp = webp.display(),
        moves = MOVES + 2,
    })
    .unwrap();
    Runner::new().run_artifacts(&tape).unwrap();

    let decoder =
        image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(fs::read(&webp).unwrap()))
            .unwrap();
    let recorded: u64 = decoder
        .into_frames()
        .map(|frame| {
            let (numer, denom) = frame.unwrap().delay().numer_denom_ms();
            u64::from(numer) / u64::from(denom)
        })
        .sum();
    // The upper bound leaves room for the opening frame and a busy CI runner. Without pacing the
    // same drag records about five times its tape time, so half again still catches it.
    assert!(
        (nominal - FRAME_MS..nominal * 3 / 2).contains(&recorded),
        "{name}: recorded {recorded} ms for a {nominal} ms drag"
    );
    fs::remove_dir_all(root).unwrap();
}
