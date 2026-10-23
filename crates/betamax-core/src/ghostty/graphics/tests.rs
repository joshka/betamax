//! Render-level tests: kitty graphics commands in, composited pixels out.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use image::{ImageFormat, Rgba, RgbaImage};

use super::placeholder::{diacritic, PLACEHOLDER};
use crate::ghostty::{
    CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, TerminalGrid, TerminalTheme,
    TextSettings,
};
use crate::media::Frame;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// A 20x6 grid with the default 14x22 cells and 60px padding.
fn session() -> (GhosttySession, TextSettings) {
    let text = TextSettings::default();
    let request = CaptureRequest {
        canvas: PixelSize::new(
            20 * text.cell_width() + 2 * text.padding,
            6 * text.cell_height() + 2 * text.padding,
        ),
        grid: TerminalGrid::new(20, 6),
        text: text.clone(),
        theme: TerminalTheme::default(),
    };
    (GhosttyFrameCapture.open(request).unwrap(), text)
}

/// Centre pixel of a cell.
fn cell_pixel(frame: &Frame, text: &TextSettings, column: u32, row: u32) -> [u8; 4] {
    pixel(
        frame,
        text.padding + column * text.cell_width() + text.cell_width() / 2,
        text.padding + row * text.cell_height() + text.cell_height() / 2,
    )
}

fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let offset = y as usize * frame.stride + x as usize * 4;
    frame.pixels[offset..offset + 4].try_into().unwrap()
}

fn png(width: u32, height: u32, color: [u8; 4]) -> String {
    let image = RgbaImage::from_pixel(width, height, Rgba(color));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    STANDARD.encode(bytes.into_inner())
}

fn kitty(control: &str, payload: &str) -> Vec<u8> {
    format!("\x1b_G{control};{payload}\x1b\\").into_bytes()
}

#[test]
fn png_placement_draws_at_the_cursor_and_moves_it() {
    let (mut session, text) = session();
    let background = cell_pixel(
        &session.capture_frame_with_cursor(false).unwrap(),
        &text,
        0,
        0,
    );
    session.write_vt(b"\x1b[2;3H");
    session.write_vt(&kitty("a=T,f=100,q=2", &png(28, 44, RED)));
    let frame = session.capture_frame_with_cursor(false).unwrap();
    // 28x44 pixels covers two 14x22 cells each way from row 1, column 2.
    assert_eq!(cell_pixel(&frame, &text, 2, 1), RED);
    assert_eq!(cell_pixel(&frame, &text, 3, 2), RED);
    assert_eq!(cell_pixel(&frame, &text, 4, 1), background);
    assert_eq!(cell_pixel(&frame, &text, 2, 3), background);
    let state = session.terminal_state().unwrap();
    assert_eq!((state.cursor.x, state.cursor.y), (4, 2));
}

#[test]
fn chunked_compressed_rgb_scales_to_requested_cells() {
    let (mut session, text) = session();
    let background = cell_pixel(
        &session.capture_frame_with_cursor(false).unwrap(),
        &text,
        0,
        0,
    );
    assert_ne!(background, [0, 0, 0, 255]);
    // zlib([0, 0, 0]) is one black RGB pixel, sent in two chunks.
    session.write_vt(&kitty("a=T,f=24,s=1,v=1,o=z,c=3,r=2,q=2,m=1", "eJxjYGAA"));
    session.write_vt(&kitty("m=0", "AAADAAE="));
    let frame = session.capture_frame_with_cursor(false).unwrap();
    for (column, row) in [(0, 0), (2, 0), (0, 1), (2, 1)] {
        assert_eq!(cell_pixel(&frame, &text, column, row), [0, 0, 0, 255]);
    }
    assert_eq!(cell_pixel(&frame, &text, 3, 0), background);
    assert_eq!(cell_pixel(&frame, &text, 0, 2), background);
}

#[test]
fn delete_removes_placements() {
    let (mut session, text) = session();
    let background = cell_pixel(
        &session.capture_frame_with_cursor(false).unwrap(),
        &text,
        0,
        0,
    );
    session.write_vt(&kitty("a=T,f=100,i=5,q=2", &png(14, 22, RED)));
    assert_eq!(
        cell_pixel(
            &session.capture_frame_with_cursor(false).unwrap(),
            &text,
            0,
            0
        ),
        RED
    );
    session.write_vt(&kitty("a=d,d=I,i=5,q=2", ""));
    assert_eq!(
        cell_pixel(
            &session.capture_frame_with_cursor(false).unwrap(),
            &text,
            0,
            0
        ),
        background
    );
}

#[test]
fn placements_scroll_with_the_grid_and_clip_at_the_top() {
    let (mut session, text) = session();
    let background = cell_pixel(
        &session.capture_frame_with_cursor(false).unwrap(),
        &text,
        0,
        0,
    );
    session.write_vt(b"\x1b[5;1H");
    session.write_vt(&kitty("a=T,f=100,q=2,C=1", &png(14, 44, RED)));
    // Five line feeds from the bottom row scroll the image's top row off screen.
    session.write_vt(b"\x1b[6;1H\n\n\n\n\n");
    let frame = session.capture_frame_with_cursor(false).unwrap();
    assert_eq!(cell_pixel(&frame, &text, 0, 0), RED);
    assert_eq!(cell_pixel(&frame, &text, 0, 1), background);
    // Nothing is drawn into the top padding above the grid.
    assert_ne!(pixel(&frame, text.padding + 7, text.padding - 2), RED);
}

#[test]
fn unicode_placeholders_draw_image_slices_instead_of_glyphs() {
    let (mut session, text) = session();
    // A 2x1 image, red then blue, shown in a 2-column by 1-row virtual placement.
    let mut pixels = RED.to_vec();
    pixels.extend_from_slice(&BLUE);
    session.write_vt(&kitty(
        "a=T,U=1,i=7,f=32,s=2,v=1,c=2,r=1,q=2",
        &STANDARD.encode(pixels),
    ));
    let row0 = diacritic(0).unwrap();
    let cells = format!("\x1b[38;2;0;0;7m{PLACEHOLDER}{row0}{row0}{PLACEHOLDER}\x1b[39m");
    session.write_vt(cells.as_bytes());
    let frame = session.capture_frame_with_cursor(false).unwrap();
    // The 2:1 image fits the 28x22 grid as 28x14, centred vertically.
    // Upscaling filters blend across the red/blue edge, so compare dominant channels.
    let [r, _, b, _] = cell_pixel(&frame, &text, 0, 0);
    assert!(r > 200 && b < 60, "left cell should be red, got {r}/{b}");
    let [r, _, b, _] = cell_pixel(&frame, &text, 1, 0);
    assert!(b > 200 && r < 60, "right cell should be blue, got {r}/{b}");
    let letterbox = pixel(&frame, text.padding + 7, text.padding + 1);
    assert_ne!(letterbox, RED);
}

#[test]
fn placeholders_without_an_image_draw_nothing() {
    let (mut session, text) = session();
    let background = cell_pixel(
        &session.capture_frame_with_cursor(false).unwrap(),
        &text,
        0,
        0,
    );
    let cells = format!("\x1b[38;5;9m{PLACEHOLDER}{PLACEHOLDER}");
    session.write_vt(cells.as_bytes());
    let frame = session.capture_frame_with_cursor(false).unwrap();
    // No tofu: the cell keeps its background.
    for x in 0..text.cell_width() {
        for y in 0..text.cell_height() {
            assert_eq!(
                pixel(&frame, text.padding + x, text.padding + y),
                background
            );
        }
    }
}
