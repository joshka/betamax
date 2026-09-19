//! Controlled VT acceptance fixtures. See docs/renderer-fidelity.md for scope and diagnostics.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use betamax_core::ghostty::{
    CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, StateSpan, TerminalGrid,
    TerminalState, TerminalTheme, TextSettings,
};
use betamax_core::media::{write_gif, write_png, Frame};

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
fn cell_graphics_demo_image() {
    let mut terminal = GhosttyFrameCapture
        .open(CaptureRequest {
            canvas: PixelSize::new(780, 350),
            grid: TerminalGrid::new(53, 11),
            text: TextSettings {
                font_family: None,
                font_size: 20.0,
                letter_spacing: 1.0,
                line_height: 1.2,
                padding: 40,
            },
            theme: TerminalTheme::from_name("Catppuccin Mocha").unwrap(),
        })
        .unwrap();
    terminal.write_vt(include_bytes!("fixtures/cell-graphics.vt"));
    let (frame, state) = checkpoint_with_size(&mut terminal, "cell-graphics-demo", (780, 350));
    assert!(state.viewport_text.contains("Betamax · Terminal previews"));
    assert!(!state.cursor.visible);
    // The demo uses 13×24 cells. Check the complete fill and uninterrupted border spans.
    let green = [166, 227, 161, 255];
    let foreground = [205, 214, 244, 255];
    for y in 40 + 4 * 24..40 + 5 * 24 {
        for x in 40 + 13..40 + 45 * 13 {
            assert_eq!(pixel(&frame, x, y), green);
        }
    }
    for x in 40 + 6..=40 + 45 * 13 + 6 {
        assert_eq!(pixel(&frame, x, 40 + 5 * 24 + 12), foreground);
    }
    for y in 40 + 3 * 24 + 12..=40 + 5 * 24 + 12 {
        assert_eq!(pixel(&frame, 40 + 6, y), foreground);
        assert_eq!(pixel(&frame, 40 + 45 * 13 + 6, y), foreground);
    }

    terminal.write_vt(b"\x1b[5;24H\x1b[32m                      ");
    let half = terminal.capture_frame().unwrap();
    assert_demo_gif(&frame, &half);
}

/// Exercise the public GIF writer and inspect decoded animation pixels, not just file creation.
fn assert_demo_gif(full: &Frame, half: &Frame) {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temporary =
        std::env::temp_dir().join(format!("betamax-demo-{}-{suffix}.gif", std::process::id()));
    let path = std::env::var_os("BETAMAX_FIDELITY_OUTPUT")
        .map(|directory| PathBuf::from(directory).join("cell-graphics-demo.gif"))
        .unwrap_or_else(|| temporary.clone());
    let frames = [
        (full.clone(), Duration::from_millis(100)),
        (half.clone(), Duration::from_millis(250)),
    ];
    write_gif(&path, &frames).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    if path == temporary {
        std::fs::remove_file(&path).unwrap();
    }
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(bytes.as_slice()).unwrap();
    assert_eq!((decoder.width(), decoder.height()), (780, 350));
    for (expected, delay) in [(full, 10), (half, 25)] {
        let decoded = decoder.read_next_frame().unwrap().expect("animation frame");
        assert_eq!(
            (decoded.left, decoded.top, decoded.width, decoded.height),
            (0, 0, 780, 350)
        );
        assert_eq!(decoded.delay, delay);
        // GIF palette quantization may shift colors slightly; every fill/border pixel must remain.
        for y in 40 + 4 * 24..=40 + 5 * 24 + 12 {
            for x in 40 + 6..=40 + 45 * 13 + 6 {
                let offset = ((y * 780 + x) * 4) as usize;
                let actual = &decoded.buffer[offset..offset + 4];
                let expected = pixel(expected, x, y);
                assert!(
                    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3),
                    "GIF pixel ({x}, {y}) differs: {actual:?} vs {expected:?}"
                );
            }
        }
    }
    assert!(decoder.read_next_frame().unwrap().is_none());
}

#[test]
fn blocks_and_borders_have_no_cell_seams() {
    let mut terminal = session();
    terminal.write_vt("\x1b[?25l\x1b[38;2;240;220;180m┌──┐\r\n│██│\r\n└──┘".as_bytes());
    let (frame, _) = checkpoint(&mut terminal, "cell-graphics");
    let ink = [240, 220, 180, 255];
    assert_solid(&cell(&frame, 1, 1), ink);
    assert_solid(&cell(&frame, 2, 1), ink);
    for x in PADDING + CELL_WIDTH / 2..=PADDING + 3 * CELL_WIDTH + CELL_WIDTH / 2 {
        assert_eq!(pixel(&frame, x, PADDING + CELL_HEIGHT / 2), ink);
        assert_eq!(
            pixel(&frame, x, PADDING + 2 * CELL_HEIGHT + CELL_HEIGHT / 2),
            ink
        );
    }
    for y in PADDING + CELL_HEIGHT / 2..=PADDING + 2 * CELL_HEIGHT + CELL_HEIGHT / 2 {
        assert_eq!(pixel(&frame, PADDING + CELL_WIDTH / 2, y), ink);
        assert_eq!(
            pixel(&frame, PADDING + 3 * CELL_WIDTH + CELL_WIDTH / 2, y),
            ink
        );
    }
}

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

#[test]
fn wide_glyph_retains_ink_in_both_cells() {
    let mut terminal = session();
    terminal.write_vt("\x1b[?25l界X".as_bytes());
    let (frame, state) = checkpoint(&mut terminal, "wide-glyph");
    assert_eq!(state.viewport_text, "界 X\n");
    assert_eq!(state.cursor.x, 3);
    assert_ink(&cell(&frame, 0, 0), BACKGROUND);
    assert_ink(&cell(&frame, 1, 0), BACKGROUND);
}

#[test]
fn wide_glyph_preserves_adjacent_styles_and_cursor() {
    let mut terminal = session();
    terminal.write_vt(
        concat!(
            "\x1b[?25l\x1b[48;2;40;70;100m界",
            "\x1b[48;2;100;30;50mX ",
            "\x1b[2;1H\x1b[48;2;40;70;100m界",
            "\x1b[48;2;100;30;50m  ",
            "\x1b[3;3HX ",
        )
        .as_bytes(),
    );
    let (hidden, _) = checkpoint(&mut terminal, "wide-styled");
    for x in 0..2 {
        assert_ink(&cell(&hidden, x, 0), SELECTION);
        assert_eq!(cell(&hidden, x, 0), cell(&hidden, x, 1));
        assert_eq!(pixel(&hidden, PADDING + x * CELL_WIDTH, PADDING), SELECTION);
    }
    assert_eq!(cell(&hidden, 2, 0), cell(&hidden, 2, 2));
    assert_solid(&cell(&hidden, 3, 0), [100, 30, 50, 255]);
    assert_solid(&cell(&hidden, 2, 1), [100, 30, 50, 255]);

    // The cursor must overlay even the continuation half of a wide glyph.
    terminal.write_vt(b"\x1b[1;2H\x1b[?25h\x1b[2 q");
    let (cursor, _) = checkpoint(&mut terminal, "wide-cursor");
    assert_solid(&cell(&cursor, 1, 0), [0, 122, 204, 255]);
    assert_eq!(cell(&cursor, 0, 0), cell(&hidden, 0, 0));
    assert_eq!(
        terminal.capture_frame_with_cursor(false).unwrap().pixels,
        hidden.pixels
    );
}

#[test]
fn wide_glyph_erasure_and_replacement_leave_no_ink() {
    // Exercise edits at both halves, and compare the entire frame to a fresh terminal.
    for (name, edit, expected) in [
        ("replace", "\x1b[1;1HA", "A X"),
        ("erase", "\x1b[1;1H\x1b[2X", "  X"),
        ("erase-continuation", "\x1b[1;2H\x1b[X", "  X"),
        ("replace-continuation", "\x1b[1;2HA", " AX"),
    ] {
        let mut terminal = session();
        terminal.write_vt("\x1b[?25l\x1b[48;2;40;70;100m界X".as_bytes());
        let (before, _) = checkpoint(&mut terminal, &format!("wide-{name}-before"));
        assert_ink(&cell(&before, 1, 0), SELECTION);
        terminal.write_vt(edit.as_bytes());
        let (after, _) = checkpoint(&mut terminal, &format!("wide-{name}"));
        let mut reference = session();
        reference.write_vt(format!("\x1b[?25l\x1b[48;2;40;70;100m{expected}").as_bytes());
        assert_eq!(
            after.pixels,
            reference.capture_frame().unwrap().pixels,
            "{name}"
        );
        assert_eq!(cell(&before, 2, 0), cell(&after, 2, 0));
    }
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
        let mut fonts = cosmic_text::FontSystem::new();
        assert!(
            fonts
                .db()
                .faces()
                .any(|face| face.families.iter().any(|(name, _)| name == family)),
            "renderer fidelity tests require {family}; see docs/renderer-fidelity.md"
        );
        assert_cjk_fallback(&mut fonts, family);
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

// Inspect the actual fallback chosen by the same shaper/settings as the renderer. Merely
// finding an installed CJK family or visible pixels would also allow a .notdef box to pass.
fn assert_cjk_fallback(fonts: &mut cosmic_text::FontSystem, family: &str) {
    use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping};

    let mut buffer = Buffer::new(fonts, Metrics::new(20.0, CELL_HEIGHT as f32));
    buffer.set_size(Some(CELL_WIDTH as f32 * 2.0), Some(CELL_HEIGHT as f32));
    let attrs = Attrs::new()
        .family(Family::Name(family))
        .letter_spacing(0.8 / 20.0);
    buffer.set_text("界", &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(fonts, false);
    let glyphs: Vec<_> = buffer.layout_runs().flat_map(|run| run.glyphs).collect();
    assert_eq!(glyphs.len(), 1, "expected one shaped CJK glyph");
    let glyph = glyphs[0];
    assert_ne!(glyph.glyph_id, 0,
        "missing CJK fallback for 界; install fonts-noto-cjk on Linux; see docs/renderer-fidelity.md");
    let font = fonts.get_font(glyph.font_id, glyph.font_weight).unwrap();
    assert_ne!(
        font.as_swash().charmap().map('界'),
        0,
        "selected font must cover 界"
    );
    let face = fonts.db().face(glyph.font_id).unwrap();
    eprintln!(
        "CJK fallback: {:?}, glyph {}",
        face.families, glyph.glyph_id
    );
}

fn checkpoint(terminal: &mut GhosttySession, name: &str) -> (Frame, TerminalState) {
    checkpoint_with_size(terminal, name, (392, 248))
}

fn checkpoint_with_size(
    terminal: &mut GhosttySession,
    name: &str,
    size: (u32, u32),
) -> (Frame, TerminalState) {
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
    assert_eq!((frame.width, frame.height), size);
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
