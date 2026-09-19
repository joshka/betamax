//! Controlled VT acceptance fixtures. See docs/renderer-fidelity.md for scope and diagnostics.

use std::path::PathBuf;
use std::sync::OnceLock;

use betamax_core::ghostty::{
    CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, StateSpan, TerminalGrid,
    TerminalState, TerminalTheme, TextSettings,
};
use betamax_core::media::{write_png, Frame};

const COLUMNS: u16 = 32;
const ROWS: u16 = 10;
const CELL_WIDTH: u32 = 12;
const CELL_HEIGHT: u32 = 24;
const PADDING: u32 = 4;
const BACKGROUND: [u8; 4] = [16, 32, 64, 255];
const SELECTION: [u8; 4] = [40, 70, 100, 255];

// A hand-authored Ratatui-style file picker: border, tabs, selected row, details, footer.
// Explicit cursor addressing matches a full-screen app without a shell or application dependency.
const LAYOUT: &str = concat!(
    "\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H",
    "┌──────────────────────────────┐",
    "\x1b[2;1H│ Files | Preview              │",
    "\x1b[3;1H├──────────────────────────────┤",
    "\x1b[4;1H│ src/                         │",
    "\x1b[5;1H│\x1b[38;2;240;220;180;48;2;40;70;100m > main.rs                    \x1b[0m│",
    "\x1b[6;1H│ Cargo.toml                   │",
    "\x1b[7;1H│                              │",
    "\x1b[8;1H│ 2 files                      │",
    "\x1b[9;1H├──────────────────────────────┤",
    "\x1b[10;1H└─ q:quit ─────────────────────┘",
);

const UNICODE: &str = "\x1b[?25l\x1b[H界X\x1b[2;1He\u{301}X\x1b[3;1HéX\x1b[4;1HeX\x1b[5;3HX";

#[test]
fn full_screen_layout_and_selection() {
    let mut terminal = session();
    terminal.write_vt(LAYOUT.as_bytes());
    let (frame, state) = checkpoint(&mut terminal, "layout");
    assert_eq!(state.size, [32, 10]);
    assert_eq!(state.scrollback_rows, 0);
    assert!(!state.cursor.visible);
    assert_eq!(
        state.viewport_text,
        concat!(
            "┌──────────────────────────────┐\n",
            "│ Files | Preview              │\n",
            "├──────────────────────────────┤\n",
            "│ src/                         │\n",
            "│ > main.rs                    │\n",
            "│ Cargo.toml                   │\n",
            "│                              │\n",
            "│ 2 files                      │\n",
            "├──────────────────────────────┤\n",
            "└─ q:quit ─────────────────────┘\n",
        )
    );
    let selected_style = state.viewport[4].iter().find_map(|span| match span {
        StateSpan::Styled(text, index) if text.contains("> main.rs") => Some(&state.styles[*index]),
        _ => None,
    });
    let selected_style = selected_style.expect("selected row has a style");
    assert_eq!(selected_style.fg.as_deref(), Some("#f0dcb4"));
    assert_eq!(selected_style.bg.as_deref(), Some("#284664"));

    assert_solid(&cell(&frame, 20, 4), SELECTION);
    assert_solid(&cell(&frame, 20, 5), BACKGROUND);
    for (x, y) in [(0, 0), (31, 0), (0, 9), (31, 9), (2, 1), (4, 4)] {
        assert_ink(
            &cell(&frame, x, y),
            if y == 4 { SELECTION } else { BACKGROUND },
        );
    }
    assert_eq!(pixel(&frame, 0, 0), BACKGROUND, "padding remains untouched");

    // A partial redraw must erase old glyphs and selection backgrounds, including blank cells.
    terminal.write_vt(b"\x1b[5;2H\x1b[0m                              ");
    let (redrawn, _) = checkpoint(&mut terminal, "layout-cleared");
    for x in 1..31 {
        assert_solid(&cell(&redrawn, x, 4), BACKGROUND);
    }
    assert_eq!(cell(&frame, 0, 4), cell(&redrawn, 0, 4));
    assert_eq!(cell(&frame, 31, 4), cell(&redrawn, 31, 4));
}

#[test]
fn wide_characters_and_combining_marks_preserve_columns() {
    let mut terminal = session();
    terminal.write_vt(UNICODE.as_bytes());
    let (frame, state) = checkpoint(&mut terminal, "unicode");
    // Current state JSON represents a wide character's continuation cell as a space.
    assert_eq!(state.viewport_text, "界 X\ne\u{301}X\néX\neX\n  X\n");
    assert_eq!((state.cursor.x, state.cursor.y), (3, 4));
    assert_eq!(
        cell(&frame, 2, 0),
        cell(&frame, 2, 4),
        "wide glyph advances two cells"
    );
    assert_eq!(
        cell(&frame, 1, 1),
        cell(&frame, 1, 3),
        "combining mark consumes no cell"
    );
    assert_ink(&cell(&frame, 1, 1), BACKGROUND);
    assert_eq!(
        cell(&frame, 0, 1),
        cell(&frame, 0, 2),
        "canonical accents render alike"
    );
    assert_ne!(
        cell(&frame, 0, 1),
        cell(&frame, 0, 3),
        "accent must be visible"
    );

    // Split UTF-8 and CSI sequences at every byte boundary, as a PTY is allowed to do.
    let mut fragmented = session();
    for byte in UNICODE.as_bytes() {
        fragmented.write_vt(&[*byte]);
    }
    let (fragmented_frame, fragmented_state) = checkpoint(&mut fragmented, "unicode-fragmented");
    assert_eq!(frame.pixels, fragmented_frame.pixels);
    assert_eq!(json(&state), json(&fragmented_state));
}

// Desired behavior, kept separate from the passing baseline until wide-cell paint ordering is
// fixed. Needs a CJK fallback font (PingFang on macOS, e.g. Noto Sans CJK on Linux).
#[test]
#[ignore = "known limitation: continuation-cell background erases the wide glyph's right half"]
fn wide_glyph_retains_ink_in_both_cells() {
    let mut terminal = session();
    terminal.write_vt("\x1b[?25l界".as_bytes());
    let (frame, _) = checkpoint(&mut terminal, "wide-glyph-known-limitation");
    assert_ink(&cell(&frame, 0, 0), BACKGROUND);
    assert_ink(&cell(&frame, 1, 0), BACKGROUND);
}

#[test]
fn alternate_screen_restores_primary_pixels_cursor_and_scrollback() {
    let mut terminal = session();
    terminal.write_vt(b"\x1b[?25l");
    for index in 0..14 {
        terminal.write_vt(format!("line {index:02}\r\n").as_bytes());
    }
    terminal.write_vt(b"\x1b[4;6H");
    let (primary, primary_state) = checkpoint(&mut terminal, "primary");
    assert_eq!(primary_state.scrollback_rows, 5);
    assert_eq!(
        primary_state.scrollback_text,
        "line 00\nline 01\nline 02\nline 03\nline 04\n"
    );
    assert_eq!(
        primary_state.viewport_text,
        "line 05\nline 06\nline 07\nline 08\nline 09\nline 10\nline 11\nline 12\nline 13\n"
    );
    assert_eq!((primary_state.cursor.x, primary_state.cursor.y), (5, 3));

    terminal.write_vt(LAYOUT.as_bytes());
    let (alternate, alternate_state) = checkpoint(&mut terminal, "alternate");
    assert_eq!(alternate_state.scrollback_rows, 0);
    assert!(alternate_state.scrollback_text.is_empty());
    assert!(!alternate_state.viewport_text.contains("line"));
    assert_ne!(alternate.pixels, primary.pixels);

    terminal.write_vt(b"\x1b[?1049l");
    let (restored, restored_state) = checkpoint(&mut terminal, "restored");
    assert_eq!(
        restored.pixels, primary.pixels,
        "alternate screen must not leak pixels"
    );
    assert_eq!(json(&restored_state), json(&primary_state));
    assert_eq!(
        terminal.capture_frame().unwrap().pixels,
        restored.pixels,
        "reading scrollback must restore the viewport"
    );
}

#[test]
fn inverse_colors_and_cursor_visibility() {
    let mut terminal = session();
    terminal.write_vt(
        concat!(
            "\x1b[?25l\x1b[38;2;240;220;180;48;2;40;70;100;7mX ",
            "\x1b[2;1H\x1b[0;38;2;40;70;100;48;2;240;220;180mX ",
            "\x1b[0m\x1b[4;4H",
        )
        .as_bytes(),
    );
    let (hidden, state) = checkpoint(&mut terminal, "inverse-hidden");
    assert!(!state.cursor.visible);
    assert_eq!(cell(&hidden, 0, 0), cell(&hidden, 0, 1));
    assert_solid(&cell(&hidden, 1, 0), [240, 220, 180, 255]);
    assert_ink(&cell(&hidden, 0, 0), [240, 220, 180, 255]);
    terminal.write_vt(b"\x1b[?25h\x1b[2 q");
    let (visible, state) = checkpoint(&mut terminal, "cursor-block");
    assert!(state.cursor.visible);
    assert_eq!((state.cursor.x, state.cursor.y), (3, 3));
    assert_solid(&cell(&visible, 3, 3), [0, 122, 204, 255]);
    assert_eq!(
        terminal.capture_frame_with_cursor(false).unwrap().pixels,
        hidden.pixels
    );
    // Cursor changes are confined to the requested cell.
    for y in 0..u32::from(ROWS) {
        for x in 0..u32::from(COLUMNS) {
            if (x, y) != (3, 3) {
                assert_eq!(cell(&visible, x, y), cell(&hidden, x, y));
            }
        }
    }
}

fn session() -> GhosttySession {
    // Do not silently pass image tests with missing fonts / an entirely blank raster.
    static FONT: OnceLock<String> = OnceLock::new();
    let family = FONT.get_or_init(|| {
        let family = if cfg!(target_os = "macos") {
            "Menlo"
        } else {
            "DejaVu Sans Mono"
        };
        let fonts = cosmic_text::FontSystem::new();
        assert!(
            fonts
                .db()
                .faces()
                .any(|face| face.families.iter().any(|(name, _)| name == family)),
            "renderer fidelity tests require {family}; see docs/renderer-fidelity.md"
        );
        family.to_owned()
    });
    let text = TextSettings {
        font_family: Some(family.clone()),
        font_size: 20.0,
        letter_spacing: 0.8,
        line_height: 1.2,
        padding: PADDING,
    };
    assert_eq!(
        (text.cell_width(), text.cell_height()),
        (CELL_WIDTH, CELL_HEIGHT)
    );
    GhosttyFrameCapture
        .open(CaptureRequest {
            canvas: PixelSize::new(392, 248),
            grid: TerminalGrid::new(COLUMNS, ROWS),
            text,
            theme: TerminalTheme::default(),
        })
        .unwrap()
}

fn checkpoint(terminal: &mut GhosttySession, name: &str) -> (Frame, TerminalState) {
    let frame = terminal.capture_frame().unwrap();
    let state = terminal.terminal_state().unwrap();
    // Opt-in artifacts are written before assertions so failures can be inspected too.
    if let Some(directory) = std::env::var_os("BETAMAX_FIDELITY_OUTPUT") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        write_png(&directory.join(format!("{name}.png")), &frame).unwrap();
        std::fs::write(
            directory.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&state).unwrap(),
        )
        .unwrap();
    }
    assert_eq!((frame.width, frame.height), (392, 248));
    assert!(frame
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| pixel[3] == 255));
    (frame, state)
}

fn json(state: &TerminalState) -> serde_json::Value {
    serde_json::to_value(state).unwrap()
}

fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let offset = y as usize * frame.stride + x as usize * 4;
    frame.pixels[offset..offset + 4].try_into().unwrap()
}

fn cell(frame: &Frame, column: u32, row: u32) -> Vec<[u8; 4]> {
    let x = PADDING + column * CELL_WIDTH;
    let y = PADDING + row * CELL_HEIGHT;
    (y..y + CELL_HEIGHT)
        .flat_map(|y| (x..x + CELL_WIDTH).map(move |x| pixel(frame, x, y)))
        .collect()
}

fn assert_solid(pixels: &[[u8; 4]], color: [u8; 4]) {
    assert!(
        pixels.iter().all(|pixel| *pixel == color),
        "expected solid {color:?}"
    );
}

fn assert_ink(pixels: &[[u8; 4]], background: [u8; 4]) {
    assert!(
        pixels.iter().filter(|pixel| **pixel != background).count() > 8,
        "expected visible glyph ink"
    );
}
