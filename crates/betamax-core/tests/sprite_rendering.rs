//! End-to-end sprite integration: cell placement, overhang, font fallback and encoded output.
//! The dependency tests its own geometry against Ghostty golden images; these tests exercise
//! Betamax's VT → metrics → coverage compositing → PNG/GIF path at every odd/even size pairing.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use betamax_core::ghostty::{
    CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, TerminalGrid, TerminalTheme,
    TextSettings,
};
use betamax_core::media::{write_gif, write_png, Frame};
use libghostty_vt::style::RgbColor;

const PADDING: u32 = 8;
const COLUMNS: u16 = 48;
const ROWS: u16 = 20;
const SAMPLE_COLUMN: u32 = 14;
const SIZES: [(u32, u32); 4] = [(12, 24), (13, 25), (12, 25), (13, 24)];
const FIXTURE: &[u8] = include_bytes!("fixtures/sprite-coverage.vt");

#[test]
fn sprite_families_png_and_gif_at_odd_and_even_cell_sizes() {
    for (width, height) in SIZES {
        let mut terminal = session(width, height);
        terminal.write_vt(FIXTURE);
        let frame = terminal.capture_frame().unwrap();
        let state = terminal.terminal_state().unwrap();
        assert!(state.viewport_text.contains("Powerline"));
        assert!(!state.cursor.visible);
        let name = format!("sprites-{width}x{height}");
        if let Some(directory) = artifact_directory() {
            write_png(&directory.join(format!("{name}.png")), &frame).unwrap();
            std::fs::write(
                directory.join(format!("{name}.json")),
                serde_json::to_vec_pretty(&state).unwrap(),
            )
            .unwrap();
        }

        // Labels exercise font fallback. Every nonblank sprite must leave visible ink in its
        // assigned terminal cell, even for symbols absent from the selected system font.
        let source = std::str::from_utf8(FIXTURE).unwrap();
        for (row, line) in source.lines().enumerate().skip(2) {
            for (column, character) in line.chars().enumerate().skip(SAMPLE_COLUMN as usize) {
                if character == ' ' || character == '⠀' {
                    continue;
                }
                assert!(
                    qwertty_term_sprite::has_codepoint(character as u32),
                    "{character}"
                );
                let pixels = cell(&frame, column as u32, row as u32, width, height);
                assert!(
                    pixels.iter().any(|p| *p != [0, 0, 0, 255]),
                    "missing {character} at {width}×{height}"
                );
            }
        }
        assert!(
            cell(&frame, 0, 0, width, height).iter().any(|p| p[0] != 0),
            "font-rendered label"
        );
        assert!(
            cell(&frame, SAMPLE_COLUMN, 10, width, height)
                .iter()
                .all(|p| *p == [0, 0, 0, 255]),
            "blank Braille remains blank"
        );
        assert!(
            cell(&frame, SAMPLE_COLUMN + 8, 8, width, height)
                .iter()
                .all(|p| *p == [255; 4]),
            "full block fills its cell"
        );
        for (column, coverage) in [(14, 64), (15, 128), (16, 192)] {
            assert!(
                cell(&frame, column, 9, width, height)
                    .iter()
                    .all(|p| *p == [coverage, coverage, coverage, 255]),
                "shade coverage"
            );
        }
        // Top and bottom edges of the rounded rectangle meet their straight segments.
        for x in PADDING + 15 * width..PADDING + 19 * width {
            assert_eq!(
                pixel(&frame, x, PADDING + 2 * height + height / 2),
                [255; 4]
            );
            assert_eq!(
                pixel(&frame, x, PADDING + 4 * height + height / 2),
                [255; 4]
            );
        }

        // Repaint with inverse colors to exercise reuse of cached masks with different colors.
        terminal.write_vt(b"\x1b[7m");
        terminal.write_vt(FIXTURE);
        let inverted = terminal.capture_frame().unwrap();
        assert_ne!(frame.pixels, inverted.pixels);
        assert_gif_round_trip(&name, &[frame, inverted]);
    }
}

/// Compare adjoining edge profiles, including their blank pixels: matching only the center
/// pixel would miss shifted strokes, extra thickness and a missing rail in double borders.
#[test]
fn adjoining_sprite_edges_and_complementary_masks_align() {
    let fixture = include_bytes!("fixtures/sprite-alignment.vt");
    for (width, height) in SIZES {
        let mut terminal = session(width, height);
        terminal.write_vt(fixture);
        let frame = terminal.capture_frame().unwrap();
        let name = format!("alignment-{width}x{height}");
        if let Some(directory) = artifact_directory() {
            write_png(&directory.join(format!("{name}.png")), &frame).unwrap();
            std::fs::write(
                directory.join(format!("{name}.json")),
                serde_json::to_vec_pretty(&terminal.terminal_state().unwrap()).unwrap(),
            )
            .unwrap();
        }

        // Light, heavy, double and rounded corners join straight segments, tees and crosses.
        // Each 3×3 grid has six horizontal and six vertical joins; all must carry ink.
        for first_column in [0, 8, 16, 24] {
            for row in 2..5 {
                for column in first_column..first_column + 2 {
                    let x = PADDING + (column + 1) * width;
                    let y = PADDING + row * height;
                    let left: Vec<_> = (y..y + height).map(|y| pixel(&frame, x - 1, y)).collect();
                    let right: Vec<_> = (y..y + height).map(|y| pixel(&frame, x, y)).collect();
                    assert!(left.iter().any(|p| p[0] != 0), "empty horizontal join");
                    assert_eq!(
                        edge_profile(&left, first_column == 24),
                        edge_profile(&right, first_column == 24),
                        "horizontal join {column},{row} at {width}×{height}"
                    );
                }
            }
            for row in 2..4 {
                for column in first_column..first_column + 3 {
                    let x = PADDING + column * width;
                    let y = PADDING + (row + 1) * height;
                    let above: Vec<_> = (x..x + width).map(|x| pixel(&frame, x, y - 1)).collect();
                    let below: Vec<_> = (x..x + width).map(|x| pixel(&frame, x, y)).collect();
                    assert!(above.iter().any(|p| p[0] != 0), "empty vertical join");
                    assert_eq!(
                        edge_profile(&above, first_column == 24),
                        edge_profile(&below, first_column == 24),
                        "vertical join {column},{row} at {width}×{height}"
                    );
                }
            }
        }

        // Adjacent repetitions preserve the entire cell-local pattern (dash phase, fractional
        // height, shade coverage and dot positions), including intentional gaps in dashed lines.
        for row in 7..14 {
            let first = cell(&frame, 14, row, width, height);
            assert!(first.iter().any(|p| p[0] != 0));
            for column in 15..18 {
                assert_eq!(
                    first,
                    cell(&frame, column, row, width, height),
                    "repeat row {row}"
                );
            }
        }

        // These pairs partition a named target. Max, rather than sum, permits the deliberate
        // one-pixel overlap of complementary halves on odd dimensions without permitting gaps.
        // Braille tests common dot positions, not continuity between discrete dots.
        for row in 15..19 {
            let left = cell(&frame, 14, row, width, height);
            let right = cell(&frame, 15, row, width, height);
            let target = cell(&frame, 17, row, width, height);
            assert!(left.iter().any(|p| p[0] != 0));
            assert!(right.iter().any(|p| p[0] != 0));
            for ((left, right), expected) in left.iter().zip(&right).zip(&target) {
                assert_eq!(
                    left[0].max(right[0]),
                    expected[0],
                    "partition row {row} at {width}×{height}"
                );
            }
        }
        terminal.write_vt(b"\x1b[7m");
        terminal.write_vt(fixture);
        assert_gif_round_trip(&name, &[frame, terminal.capture_frame().unwrap()]);
    }
}

#[test]
fn diagonal_overhang_survives_backgrounds_and_redraw() {
    for (width, height) in SIZES {
        let mut terminal = session(width, height);
        terminal.write_vt("\x1b[?25l\x1b[3;3H╲".as_bytes());
        let frame = terminal.capture_frame().unwrap();
        let glyph = qwertty_term_sprite::render(
            '╲' as u32,
            &qwertty_term_sprite::Metrics::simple(width, height),
        )
        .unwrap();
        let left = PADDING as i32 + 2 * width as i32 + glyph.offset_x;
        let top = PADDING as i32 + 3 * height as i32 - glyph.offset_y;
        let mut overhang_pixels = 0;
        for y in 0..glyph.height {
            for x in 0..glyph.width {
                let alpha = glyph.alpha[(y * glyph.width + x) as usize];
                let px = (left + x as i32) as u32;
                let py = (top + y as i32) as u32;
                assert_eq!(pixel(&frame, px, py), [alpha, alpha, alpha, 255]);
                if alpha != 0 && !(PADDING + 2 * height..PADDING + 3 * height).contains(&py) {
                    overhang_pixels += 1;
                }
            }
        }
        assert!(overhang_pixels > 0, "test must exercise vertical overhang");
        terminal.write_vt(b"\x1b[3;3H ");
        let erased = terminal.capture_frame().unwrap();
        assert!(erased
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255]));

        // Remove padding to actually clip the negative overhang at the canvas edge.
        let mut at_edge = session_with_padding(width, height, 0);
        at_edge.write_vt("\x1b[?25l\x1b[1;1H╲".as_bytes());
        let edge = at_edge.capture_frame().unwrap();
        for y in 0..height {
            for x in 0..width {
                assert_eq!(
                    pixel(&edge, x, y),
                    pixel(&frame, PADDING + 2 * width + x, PADDING + 2 * height + y)
                );
            }
        }
    }
}

// Curved corners approach the edge with antialiasing. Compare the half-coverage contour
// to require identical stroke positions; straight borders retain exact alpha comparisons.
fn edge_profile(pixels: &[[u8; 4]], rounded: bool) -> Vec<u8> {
    pixels
        .iter()
        .map(|p| if rounded { u8::from(p[0] >= 128) } else { p[0] })
        .collect()
}

fn session(width: u32, height: u32) -> GhosttySession {
    session_with_padding(width, height, PADDING)
}

fn session_with_padding(width: u32, height: u32, padding: u32) -> GhosttySession {
    let text = TextSettings {
        font_family: None,
        font_size: 20.0,
        letter_spacing: width as f32 - 20.0 * 0.56,
        line_height: height as f32 / 20.0,
        padding,
    };
    assert_eq!((text.cell_width(), text.cell_height()), (width, height));
    let theme = TerminalTheme {
        background: RgbColor { r: 0, g: 0, b: 0 },
        foreground: RgbColor {
            r: 255,
            g: 255,
            b: 255,
        },
        ..TerminalTheme::default()
    };
    GhosttyFrameCapture
        .open(CaptureRequest {
            canvas: PixelSize::new(
                2 * padding + u32::from(COLUMNS) * width,
                2 * padding + u32::from(ROWS) * height,
            ),
            grid: TerminalGrid::new(COLUMNS, ROWS),
            text,
            theme,
        })
        .unwrap()
}

fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let offset = y as usize * frame.stride + x as usize * 4;
    frame.pixels[offset..offset + 4].try_into().unwrap()
}

fn cell(frame: &Frame, column: u32, row: u32, width: u32, height: u32) -> Vec<[u8; 4]> {
    let x = PADDING + column * width;
    let y = PADDING + row * height;
    (y..y + height)
        .flat_map(|yy| (x..x + width).map(move |xx| pixel(frame, xx, yy)))
        .collect()
}

fn artifact_directory() -> Option<PathBuf> {
    let directory = PathBuf::from(std::env::var_os("BETAMAX_FIDELITY_OUTPUT")?);
    std::fs::create_dir_all(&directory).unwrap();
    Some(directory)
}

fn assert_gif_round_trip(name: &str, frames: &[Frame]) {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temporary = std::env::temp_dir().join(format!(
        "betamax-{name}-{}-{suffix}.gif",
        std::process::id()
    ));
    let path = artifact_directory()
        .map(|dir| dir.join(format!("{name}.gif")))
        .unwrap_or_else(|| temporary.clone());
    let timed: Vec<_> = frames
        .iter()
        .cloned()
        .map(|frame| (frame, Duration::from_millis(500)))
        .collect();
    write_gif(&path, &timed).unwrap();
    let encoded = std::fs::read(&path).unwrap();
    if path == temporary {
        std::fs::remove_file(&path).unwrap();
    }
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(encoded.as_slice()).unwrap();
    assert_eq!(
        (u32::from(decoder.width()), u32::from(decoder.height())),
        (frames[0].width, frames[0].height)
    );
    for expected in frames {
        let decoded = decoder.read_next_frame().unwrap().expect("GIF frame");
        assert_eq!((decoded.left, decoded.top), (0, 0));
        assert_eq!(
            (u32::from(decoded.width), u32::from(decoded.height)),
            (expected.width, expected.height)
        );
        assert_eq!(decoded.delay, 50);
        // Monochrome coverage has at most 256 colors, so GIF must retain every pixel exactly.
        assert_eq!(
            decoded.buffer.as_ref(),
            expected.pixels,
            "GIF coverage changed for {name}"
        );
    }
    assert!(decoder.read_next_frame().unwrap().is_none());
    assert_png_round_trip(&path.with_extension("png"), &frames[0]);
}

fn assert_png_round_trip(path: &Path, expected: &Frame) {
    // When artifacts are disabled the PNG is temporary, just like the GIF.
    write_png(path, expected).unwrap();
    let bytes = std::fs::read(path).unwrap();
    if artifact_directory().is_none() {
        std::fs::remove_file(path).unwrap();
    }
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (expected.width, expected.height));
    assert_eq!(&pixels[..info.buffer_size()], expected.pixels);
}
