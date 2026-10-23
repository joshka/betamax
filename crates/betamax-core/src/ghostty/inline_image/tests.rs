//! Stream-level tests: sixel and iTerm2 sequences in, placeholder images out.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use image::{ImageFormat, Rgba, RgbaImage};

use crate::ghostty::{
    CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, TerminalGrid, TerminalTheme,
    TextSettings,
};

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

fn cell_pixel(session: &mut GhosttySession, text: &TextSettings, column: u32, row: u32) -> [u8; 4] {
    let frame = session.capture_frame_with_cursor(false).unwrap();
    let x = text.padding + column * text.cell_width() + text.cell_width() / 2;
    let y = text.padding + row * text.cell_height() + text.cell_height() / 2;
    let offset = y as usize * frame.stride + x as usize * 4;
    frame.pixels[offset..offset + 4].try_into().unwrap()
}

/// Solid red sixel of `width` x `height` pixels (height a multiple of 6).
fn red_sixel(width: u32, height: u32) -> Vec<u8> {
    let mut out = format!("\x1bP0;1;0q\"1;1;{width};{height}#1;2;100;0;0#1");
    for band in 0..height / 6 {
        if band > 0 {
            out.push('-');
        }
        out.push_str(&format!("!{width}~"));
    }
    out.push_str("\x1b\\");
    out.into_bytes()
}

#[test]
fn sixel_draws_at_cursor_and_moves_below_it() {
    let (mut session, text) = session();
    session.write_vt(b"\x1b[2;3H");
    session.write_vt(&red_sixel(28, 42));
    assert_eq!(cell_pixel(&mut session, &text, 2, 1), [255, 0, 0, 255]);
    assert_eq!(cell_pixel(&mut session, &text, 3, 2), [255, 0, 0, 255]);
    assert_ne!(cell_pixel(&mut session, &text, 4, 1), [255, 0, 0, 255]);
    let state = session.terminal_state().unwrap();
    assert_eq!((state.cursor.x, state.cursor.y), (2, 3));
}

#[test]
fn sixel_split_across_writes_and_text_around_it_survive() {
    let (mut session, text) = session();
    let sixel = red_sixel(14, 18);
    let mut stream = b"ab".to_vec();
    stream.extend_from_slice(&sixel);
    stream.extend_from_slice(b"cd");
    for chunk in stream.chunks(3) {
        session.write_vt(chunk);
    }
    assert_eq!(cell_pixel(&mut session, &text, 2, 0), [255, 0, 0, 255]);
    let screen = session.screen_text().unwrap();
    assert!(screen.starts_with("ab"), "{screen:?}");
    assert!(screen.lines().nth(1).unwrap().contains("cd"), "{screen:?}");
}

#[test]
fn text_overwriting_a_sixel_erases_that_cell() {
    let (mut session, text) = session();
    session.write_vt(&red_sixel(28, 18));
    session.write_vt(b"\x1b[1;1Hx");
    assert_ne!(cell_pixel(&mut session, &text, 0, 0), [255, 0, 0, 255]);
    assert_eq!(cell_pixel(&mut session, &text, 1, 0), [255, 0, 0, 255]);
}

#[test]
fn other_dcs_and_osc_sequences_pass_through() {
    let (mut session, _) = session();
    session.write_vt(b"\x1b]2;title\x07\x1bP$qm\x1b\\ok");
    assert!(session.screen_text().unwrap().starts_with("ok"));
    assert_eq!(session.terminal_state().unwrap().title, "title");
}

#[test]
fn iterm2_inline_png_draws_and_moves_right() {
    let (mut session, text) = session();
    let image = RgbaImage::from_pixel(28, 22, Rgba([0, 0, 255, 255]));
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, ImageFormat::Png).unwrap();
    let sequence = format!(
        "\x1b]1337;File=inline=1:{}\x07",
        STANDARD.encode(png.into_inner())
    );
    session.write_vt(sequence.as_bytes());
    assert_eq!(cell_pixel(&mut session, &text, 1, 0), [0, 0, 255, 255]);
    let state = session.terminal_state().unwrap();
    assert_eq!((state.cursor.x, state.cursor.y), (2, 0));
}
