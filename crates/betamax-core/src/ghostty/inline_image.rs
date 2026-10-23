//! Sixel and iTerm2 inline images, translated into kitty virtual placements.
//!
//! libghostty-vt implements the kitty graphics protocol but neither sixel nor iTerm2's
//! `OSC 1337 ; File=`. [`InlineImages`] sits between the PTY and the terminal: it passes bytes
//! through unchanged until one of those sequences appears, decodes the image, and replaces the
//! sequence with a kitty transmission plus a block of unicode placeholder cells at the cursor.
//!
//! Placeholders make the image part of the cell grid, which matches how sixel and iTerm2 images
//! behave in a real terminal: text written over the cells replaces those parts of the image, and
//! the image scrolls, clears, and reflows with the cells. Compositing then follows the same path
//! as program-written kitty placeholders (see `graphics`).

mod iterm2;
mod sixel;

use std::fmt::Write as _;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use image::RgbaImage;
use libghostty_vt::style::{Style, StyleColor, Underline};
use libghostty_vt::Terminal;

use super::graphics::placeholder::{diacritic, PLACEHOLDER};

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
/// OSC prefix after `ESC ]` that introduces an iTerm2 file.
const ITERM2_PREFIX: &[u8] = b"1337;File=";
/// Longest sixel parameter string accepted before treating a DCS as something else.
const MAX_DCS_PARAMS: usize = 32;
/// Largest sequence body buffered for decoding; larger images are dropped.
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
/// Base64 bytes per kitty transmission chunk, the protocol's recommended maximum.
const KITTY_CHUNK: usize = 4096;
/// Most placeholder rows or columns a diacritic can address.
const MAX_PLACEHOLDER_CELLS: u32 = 297;
/// High byte of image ids assigned to translated images, keeping them clear of the small ids
/// programs choose for their own kitty images.
const IMAGE_ID_HIGH: u32 = 0xb7;

/// Width and height pair, in cells or pixels depending on use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Extent {
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Scanner position within the PTY byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Passing bytes through.
    Ground,
    /// Saw `ESC`; the next byte decides whether this is a sequence of interest.
    Escape,
    /// Inside `ESC P`, reading numeric parameters that may precede the sixel `q`.
    DcsParams,
    /// Matching the `1337;File=` prefix of an OSC; the value is the number matched so far.
    OscPrefix(usize),
    /// Buffering a sequence body until its terminator.
    Body(Kind),
    /// Saw `ESC` inside a body; `\` completes the string terminator.
    BodyEscape(Kind),
}

/// The kind of image sequence being buffered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Sixel,
    Iterm2,
}

/// Where the cursor goes after an image, which differs by protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorAfter {
    /// Sixel: the start column of the line below the image.
    NextLine,
    /// iTerm2: just right of the image on its last row.
    AfterImage,
    /// iTerm2 `doNotMoveCursor=1`: where it was before the image.
    Unmoved,
}

/// Translates sixel and iTerm2 images in a PTY byte stream into kitty placeholders.
#[derive(Debug)]
pub(super) struct InlineImages {
    state: State,
    /// Bytes of a possible sequence start, replayed if it turns out not to be an image.
    pending: Vec<u8>,
    /// Body of the image sequence being buffered.
    body: Vec<u8>,
    /// Set when a body exceeded [`MAX_PAYLOAD_BYTES`]; the rest is skipped and nothing drawn.
    oversized: bool,
    /// Low 24 bits of the next translated image id.
    next_id: u32,
    /// Cell size in pixels, used to size images in cells.
    cell: Extent,
}

impl InlineImages {
    /// Create a translator for a terminal with the given cell size in pixels.
    pub(super) fn new(cell: Extent) -> Self {
        Self {
            state: State::Ground,
            pending: Vec::new(),
            body: Vec::new(),
            oversized: false,
            next_id: 1,
            cell,
        }
    }

    /// Feed PTY bytes, writing everything except image sequences straight to `terminal`.
    ///
    /// Sequences may be split across calls; partial state is kept until the next call.
    pub(super) fn write(&mut self, terminal: &mut Terminal<'static, 'static>, bytes: &[u8]) {
        let mut start = 0;
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            match self.state {
                State::Ground => {
                    match memchr(ESC, &bytes[index..]) {
                        Some(offset) => {
                            index += offset;
                            terminal.vt_write(&bytes[start..index]);
                            self.pending.push(ESC);
                            self.state = State::Escape;
                        }
                        None => index = bytes.len(),
                    }
                    index += 1;
                    continue;
                }
                State::Escape => match byte {
                    b'P' => self.state = State::DcsParams,
                    b']' => self.state = State::OscPrefix(0),
                    _ => {
                        start = self.replay_pending(terminal, index);
                        continue;
                    }
                },
                State::DcsParams => match byte {
                    b'0'..=b'9' | b';' if self.pending.len() < MAX_DCS_PARAMS => {}
                    b'q' => {
                        self.begin_body(Kind::Sixel);
                        index += 1;
                        continue;
                    }
                    _ => {
                        start = self.replay_pending(terminal, index);
                        continue;
                    }
                },
                State::OscPrefix(matched) => {
                    if byte != ITERM2_PREFIX[matched] {
                        start = self.replay_pending(terminal, index);
                        continue;
                    }
                    if matched + 1 == ITERM2_PREFIX.len() {
                        self.begin_body(Kind::Iterm2);
                        index += 1;
                        continue;
                    }
                    self.state = State::OscPrefix(matched + 1);
                }
                State::Body(kind) => {
                    let rest = &bytes[index..];
                    let end = rest
                        .iter()
                        .position(|&byte| byte == ESC || (kind == Kind::Iterm2 && byte == BEL))
                        .unwrap_or(rest.len());
                    self.buffer_body(&rest[..end]);
                    index += end;
                    match rest.get(end) {
                        Some(&BEL) => {
                            self.finish_body(terminal, kind);
                            start = index + 1;
                        }
                        Some(_) => self.state = State::BodyEscape(kind),
                        None => {}
                    }
                    index += 1;
                    continue;
                }
                State::BodyEscape(kind) => {
                    self.finish_body(terminal, kind);
                    if byte == b'\\' {
                        start = index + 1;
                    } else {
                        // Any other escape cancels the string and starts a new sequence.
                        self.pending.push(ESC);
                        self.state = State::Escape;
                        continue;
                    }
                    index += 1;
                    continue;
                }
            }
            self.pending.push(byte);
            index += 1;
        }
        if self.state == State::Ground && start < bytes.len() {
            terminal.vt_write(&bytes[start..]);
        }
    }

    /// Give up on a possible sequence start: write the held bytes and resume in the ground state.
    ///
    /// Returns the index where passthrough resumes; the current byte is processed again there.
    fn replay_pending(&mut self, terminal: &mut Terminal<'static, 'static>, index: usize) -> usize {
        terminal.vt_write(&self.pending);
        self.pending.clear();
        self.state = State::Ground;
        index
    }

    fn begin_body(&mut self, kind: Kind) {
        self.pending.clear();
        self.body.clear();
        self.oversized = false;
        self.state = State::Body(kind);
    }

    fn buffer_body(&mut self, bytes: &[u8]) {
        if self.oversized {
            return;
        }
        if self.body.len() + bytes.len() > MAX_PAYLOAD_BYTES {
            tracing::debug!("dropping inline image larger than {MAX_PAYLOAD_BYTES} bytes");
            self.oversized = true;
            self.body = Vec::new();
            return;
        }
        self.body.extend_from_slice(bytes);
    }

    fn finish_body(&mut self, terminal: &mut Terminal<'static, 'static>, kind: Kind) {
        self.state = State::Ground;
        let body = std::mem::take(&mut self.body);
        if self.oversized {
            return;
        }
        let decoded = match kind {
            Kind::Sixel => sixel::decode(&body).map(|image| (image, CursorAfter::NextLine)),
            Kind::Iterm2 => {
                let grid = Extent {
                    width: terminal.cols().map_or(80, u32::from),
                    height: terminal.rows().map_or(24, u32::from),
                };
                iterm2::decode(&body, grid, self.cell).map(|decoded| {
                    let cursor = if decoded.keep_cursor {
                        CursorAfter::Unmoved
                    } else {
                        CursorAfter::AfterImage
                    };
                    (decoded.image, cursor)
                })
            }
        };
        match decoded {
            Some((image, cursor)) => self.place(terminal, image, cursor),
            None => tracing::debug!(?kind, "inline image produced no pixels"),
        }
    }

    /// Transmit `image` as a kitty virtual placement and cover its cells with placeholders.
    fn place(
        &mut self,
        terminal: &mut Terminal<'static, 'static>,
        image: RgbaImage,
        cursor: CursorAfter,
    ) {
        let (Ok(columns_total), Ok(x0), Ok(y0)) =
            (terminal.cols(), terminal.cursor_x(), terminal.cursor_y())
        else {
            return;
        };
        let columns = image
            .width()
            .div_ceil(self.cell.width.max(1))
            .clamp(1, MAX_PLACEHOLDER_CELLS);
        let rows = image
            .height()
            .div_ceil(self.cell.height.max(1))
            .clamp(1, MAX_PLACEHOLDER_CELLS);
        let image = pad_to_cells(image, columns, rows, self.cell);
        let id_low = self.next_id;
        self.next_id = self.next_id % 0x00ff_ffff + 1;
        let id = (IMAGE_ID_HIGH << 24) | id_low;
        terminal.vt_write(&transmit_virtual(&image, id, columns, rows));

        let pen = terminal.cursor_style().ok();
        let visible = columns
            .min(u32::from(columns_total.saturating_sub(x0)))
            .max(1);
        terminal.vt_write(&placeholder_block(id, rows, visible, x0));
        let last_row = terminal.cursor_y().unwrap_or(y0);

        let mut restore = pen.map(|pen| sgr(&pen)).unwrap_or_else(|| "\x1b[0m".into());
        let column = |x: u32| x.min(u32::from(columns_total.saturating_sub(1))) + 1;
        match cursor {
            CursorAfter::NextLine => {
                let _ = write!(restore, "\x1b[{};{}H\x1bD", last_row + 1, x0 + 1);
            }
            CursorAfter::AfterImage => {
                let _ = write!(
                    restore,
                    "\x1b[{};{}H",
                    last_row + 1,
                    column(u32::from(x0) + visible)
                );
            }
            CursorAfter::Unmoved => {
                let _ = write!(restore, "\x1b[{};{}H", y0 + 1, x0 + 1);
            }
        }
        terminal.vt_write(restore.as_bytes());
    }
}

/// Pad `image` with transparency to whole cells so the placement shows it at its own size.
///
/// Kitty fits a virtual placement's image into its cell grid; an image that already matches the
/// grid's pixel size is drawn 1:1, top-left aligned, as sixel and iTerm2 images are.
fn pad_to_cells(image: RgbaImage, columns: u32, rows: u32, cell: Extent) -> RgbaImage {
    let width = columns * cell.width;
    let height = rows * cell.height;
    if image.dimensions() == (width, height) {
        return image;
    }
    let mut padded = RgbaImage::new(width, height);
    image::imageops::replace(&mut padded, &image, 0, 0);
    padded
}

/// Kitty commands transmitting `image` as RGBA with a virtual placement of `columns` x `rows`.
fn transmit_virtual(image: &RgbaImage, id: u32, columns: u32, rows: u32) -> Vec<u8> {
    let encoded = STANDARD.encode(image.as_raw());
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(KITTY_CHUNK).collect();
    let mut out = Vec::with_capacity(encoded.len() + chunks.len() * 16 + 96);
    for (index, chunk) in chunks.iter().enumerate() {
        let more = u8::from(index + 1 < chunks.len());
        if index == 0 {
            let _ = write!(
                Bytes(&mut out),
                "\x1b_Ga=T,U=1,q=2,f=32,i={id},s={},v={},c={columns},r={rows},m={more};",
                image.width(),
                image.height(),
            );
        } else {
            let _ = write!(Bytes(&mut out), "\x1b_Gm={more};");
        }
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
    out
}

/// Placeholder cells for every row of the image, starting at the cursor.
///
/// Every cell carries explicit row, column, and id-high diacritics rather than inheriting them,
/// so overwriting one cell cannot shift the image slices shown by its neighbours. Rows after the
/// first start with index (`ESC D`), which scrolls at the bottom margin like the image rows a
/// terminal would draw.
fn placeholder_block(id: u32, rows: u32, columns: u32, x0: u16) -> Vec<u8> {
    let high = diacritic(id >> 24).expect("id high byte is below the diacritic count");
    let mut out = String::new();
    let _ = write!(
        out,
        "\x1b[0;38;2;{};{};{}m",
        (id >> 16) & 0xff,
        (id >> 8) & 0xff,
        id & 0xff
    );
    for row in 0..rows {
        if row > 0 {
            out.push_str("\x1bD\r");
            if x0 > 0 {
                let _ = write!(out, "\x1b[{x0}C");
            }
        }
        let row_mark = diacritic(row).expect("rows are clamped to the diacritic count");
        for column in 0..columns {
            out.push(PLACEHOLDER);
            out.push(row_mark);
            out.push(diacritic(column).expect("columns are clamped to the diacritic count"));
            out.push(high);
        }
    }
    out.into_bytes()
}

/// SGR sequence that restores `style` as the current pen.
fn sgr(style: &Style) -> String {
    let mut out = String::from("\x1b[0");
    let flags = [
        (style.bold, "1"),
        (style.faint, "2"),
        (style.italic, "3"),
        (style.blink, "5"),
        (style.inverse, "7"),
        (style.invisible, "8"),
        (style.strikethrough, "9"),
        (style.overline, "53"),
    ];
    for (_, code) in flags.iter().filter(|(set, _)| *set) {
        out.push(';');
        out.push_str(code);
    }
    let underline = match style.underline {
        Underline::None => None,
        Underline::Single => Some("4"),
        Underline::Double => Some("4:2"),
        Underline::Curly => Some("4:3"),
        Underline::Dotted => Some("4:4"),
        Underline::Dashed => Some("4:5"),
        _ => Some("4"),
    };
    if let Some(code) = underline {
        out.push(';');
        out.push_str(code);
    }
    for (color, base) in [
        (style.fg_color, 38),
        (style.bg_color, 48),
        (style.underline_color, 58),
    ] {
        match color {
            StyleColor::None => {}
            StyleColor::Palette(index) => {
                let _ = write!(out, ";{base};5;{}", index.0);
            }
            StyleColor::Rgb(rgb) => {
                let _ = write!(out, ";{base};2;{};{};{}", rgb.r, rgb.g, rgb.b);
            }
        }
    }
    out.push('m');
    out
}

fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&byte| byte == needle)
}

/// `fmt::Write` adapter over a byte vector.
struct Bytes<'a>(&'a mut Vec<u8>);

impl std::fmt::Write for Bytes<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0.extend_from_slice(text.as_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests;
