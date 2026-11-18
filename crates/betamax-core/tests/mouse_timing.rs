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
    let root = std::env::temp_dir().join(format!(
        "betamax-mouse-timing-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let webp = root.join("drag.webp");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/mouse_trail.py");
    let mut drag = String::new();
    for column in 0..MOVES {
        writeln!(drag, "MouseMove {} 10", column + 2).unwrap();
        writeln!(drag, "Sleep {SLEEP_MS}ms").unwrap();
    }
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
    let nominal = MOVES * (FRAME_MS + SLEEP_MS);
    // The upper bound leaves room for the opening frame and a busy CI runner. Without pacing the
    // same drag records about five times its tape time, so half again still catches it.
    assert!(
        (nominal - FRAME_MS..nominal * 3 / 2).contains(&recorded),
        "recorded {recorded} ms for a {nominal} ms drag"
    );
    fs::remove_dir_all(root).unwrap();
}
