//! Frames that repaint only dirty rows match a frame rendered from scratch, pixel for pixel.

use betamax_core::ghostty::{
    CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, TerminalGrid, TerminalTheme,
    TextSettings,
};

const COLUMNS: u16 = 40;
const ROWS: u16 = 12;

fn session() -> GhosttySession {
    let text = TextSettings::default();
    let canvas = PixelSize::new(
        u32::from(COLUMNS) * text.cell_width() + 2 * text.padding,
        u32::from(ROWS) * text.cell_height() + 2 * text.padding,
    );
    GhosttyFrameCapture
        .open(CaptureRequest {
            canvas,
            grid: TerminalGrid::new(COLUMNS, ROWS),
            text,
            theme: TerminalTheme::default(),
        })
        .unwrap()
}

/// Each step is rendered incrementally by one session and from scratch by a fresh session that
/// replays everything so far. Steps cover sprites that overhang neighboring rows, styled and
/// wide text, erasing, scrolling, cursor moves, and palette changes.
#[test]
fn incremental_frames_match_full_renders() {
    let steps: &[&[u8]] = &[
        b"\x1b[2J\x1b[Hplain text\r\n\x1b[1;31mbold red\x1b[0m \x1b[3;38;2;10;200;90mitalic\x1b[0m",
        "\x1b[4;1H╱╲╱╲ ▀▄█ ┌─┐ 界 🙂 ".as_bytes(),
        b"\x1b[5;3H\x1b[44mbackground\x1b[0m\x1b[2;1H\x1b[K",
        "\x1b[4;2H\x1b[2m╲╱\x1b[0m".as_bytes(),
        b"\x1b[12;1H\r\nscrolled\r\nagain",
        b"\x1b]4;1;rgb:ff/80/00\x07\x1b[6;1H\x1b[31mrecolored",
        b"\x1b[?25l\x1b[8;8Hhidden cursor",
    ];
    let mut incremental = session();
    let mut so_far = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        incremental.write_vt(step);
        so_far.extend_from_slice(step);
        let mut full = session();
        full.write_vt(&so_far);
        let (actual, expected) = (
            incremental.capture_frame().unwrap(),
            full.capture_frame().unwrap(),
        );
        assert!(
            actual.pixels == expected.pixels,
            "step {index}: incremental frame differs from a full render"
        );
    }
}
