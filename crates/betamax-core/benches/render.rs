//! Frame render cost for a full screen of styled text, the case that made paced input lag.
//!
//! Run with `cargo bench -p betamax-core --bench render`.

use std::fmt::Write as _;
use std::hint::black_box;

use betamax_core::ghostty::{
    CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, TerminalGrid, TerminalTheme,
    TextSettings,
};
use criterion::{criterion_group, criterion_main, Criterion};

const COLUMNS: u16 = 190;
const ROWS: u16 = 56;

/// A TUI-like screen: every cell holds text in one of a few truecolor styles, with bold labels,
/// box-drawing borders, and punctuation, as a dashboard of colored panels would.
fn styled_screen() -> String {
    const WORDS: [&str; 8] = [
        "scan", "PORT", "open", "filtered", "TICK", "00239", "hop", "·",
    ];
    let mut screen = String::from("\x1b[2J\x1b[H");
    for row in 0..ROWS {
        write!(screen, "\x1b[{};1H", row + 1).unwrap();
        let mut column = 0usize;
        let mut word = usize::from(row);
        while column < usize::from(COLUMNS) {
            let text = if column.is_multiple_of(48) {
                "│"
            } else {
                WORDS[word % WORDS.len()]
            };
            let bold = if word.is_multiple_of(5) { "1;" } else { "" };
            let shade = (word * 37 % 200) as u8 + 40;
            write!(
                screen,
                "\x1b[0;{bold}38;2;{shade};{};{}m\x1b[48;2;{};18;13m{text} ",
                255 - shade,
                shade / 2 + 60,
                row * 3 % 40
            )
            .unwrap();
            column += text.chars().count() + 1;
            word += 1;
        }
    }
    screen.push_str("\x1b[0m");
    screen
}

fn session() -> GhosttySession {
    let text = TextSettings {
        font_size: 14.0,
        ..TextSettings::default()
    };
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

fn render(c: &mut Criterion) {
    let mut terminal = session();
    terminal.write_vt(styled_screen().as_bytes());
    // An animated app changes a little of the screen between frames; the rest is redrawn as is.
    let mut tick = 0u32;
    c.bench_function("styled screen frame", |b| {
        b.iter(|| {
            tick += 1;
            terminal.write_vt(format!("\x1b[1;1H\x1b[0mTICK {tick:05}").as_bytes());
            black_box(terminal.capture_frame().unwrap())
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = render
}
criterion_main!(benches);
