//! Recorded typing keeps `TypingSpeed` time, and GIF and WebP agree on it.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use betamax_core::{RunOptions, Runner, Tape};
use image::AnimationDecoder;
use indoc::formatdoc;

/// 40 characters at 35 ms each.
const TYPED: &str = "echo 0123456789012345678901234567890123";
const SPEED_MS: u64 = 35;
const NOMINAL_MS: u64 = 40 * SPEED_MS;

#[test]
fn typing_records_its_nominal_duration_in_gif_and_webp() {
    let root = std::env::temp_dir().join(format!(
        "betamax-typing-timing-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let (gif, webp) = (root.join("typing.gif"), root.join("typing.webp"));
    let tape = Tape::parse(&formatdoc! {r#"
        Output "{gif}"
        Output "{webp}"
        Set Shell "bash"
        Set Width 1200
        Set Height 720
        Set Framerate 60
        Set TypingSpeed {SPEED_MS}ms
        Hide
        Type "clear"
        Enter
        Sleep 300ms
        Show
        Type "{TYPED}"
        "#,
        gif = gif.display(),
        webp = webp.display(),
    })
    .unwrap();
    Runner::new(RunOptions {
        quiet: true,
        publish: false,
    })
    .run_artifacts(&tape)
    .unwrap();

    let webp_ms = total_ms(webp_frames(&webp));
    let gif_ms = total_ms(gif_frames(&gif));
    // At this size a render takes a large share of each keystroke. Recorded time must still
    // include it, and keystroke pacing must not let it slow typing down. The last keystroke's hold
    // may end just before its deadline; the upper bound leaves room for the opening frame and a
    // busy machine.
    assert!(
        (NOMINAL_MS - SPEED_MS..NOMINAL_MS + 1500).contains(&webp_ms),
        "WebP records {webp_ms} ms for {NOMINAL_MS} ms of typing"
    );
    // GIF rounds the running total to hundredths, so it stays within one unit of WebP.
    assert!(
        gif_ms.abs_diff(webp_ms) <= 10,
        "GIF records {gif_ms} ms, WebP {webp_ms} ms"
    );
    fs::remove_dir_all(root).unwrap();
}

/// The same text typed at two speeds records proportionally different durations.
///
/// Typing goes into `cat`, so each keystroke redraws one echoed character and nothing else. The
/// large canvas makes each render slow enough that losing it from the timeline would show.
#[test]
fn typing_duration_scales_with_typing_speed() {
    const TEXT: &str = "the quick brown fox jumps over the lazy dog";
    let chars = TEXT.chars().count() as u64;
    let [fast, slow] = [18, 54].map(|speed_ms| {
        let root = std::env::temp_dir().join(format!(
            "betamax-typing-scale-{speed_ms}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let webp = root.join("typing.webp");
        let tape = Tape::parse(&formatdoc! {r#"
            Output "{webp}"
            Set Shell "bash"
            Set Width 1200
            Set Height 720
            Set Framerate 60
            Set TypingSpeed {speed_ms}ms
            Hide
            Type "clear; cat"
            Enter
            Sleep 300ms
            Show
            Type "{TEXT}"
            "#,
            webp = webp.display(),
        })
        .unwrap();
        Runner::new(RunOptions {
            quiet: true,
            publish: false,
        })
        .run_artifacts(&tape)
        .unwrap();
        let recorded = total_ms(webp_frames(&webp));
        let nominal = chars * speed_ms;
        assert!(
            (nominal - speed_ms..nominal + 1000).contains(&recorded),
            "TypingSpeed {speed_ms}ms records {recorded} ms for {nominal} ms of typing"
        );
        fs::remove_dir_all(root).unwrap();
        recorded
    });
    // Tripling TypingSpeed must roughly triple the recording; shared overhead lowers the ratio.
    let ratio = slow as f64 / fast as f64;
    assert!(
        (2.2..3.3).contains(&ratio),
        "54 ms typing records {slow} ms, 18 ms records {fast} ms (ratio {ratio:.2})"
    );
}

fn total_ms(delays: Vec<u64>) -> u64 {
    delays.into_iter().sum()
}

fn webp_frames(path: &PathBuf) -> Vec<u64> {
    let decoder =
        image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(fs::read(path).unwrap()))
            .unwrap();
    delays(decoder.into_frames())
}

fn gif_frames(path: &PathBuf) -> Vec<u64> {
    let decoder =
        image::codecs::gif::GifDecoder::new(std::io::Cursor::new(fs::read(path).unwrap())).unwrap();
    delays(decoder.into_frames())
}

fn delays(frames: image::Frames<'_>) -> Vec<u64> {
    frames
        .map(|frame| {
            let (numer, denom) = frame.unwrap().delay().numer_denom_ms();
            u64::from(numer) / u64::from(denom)
        })
        .collect()
}
