//! Ghostty-backed terminal capture sessions.

use std::cell::Cell;
use std::rc::Rc;

use libghostty_vt::{Terminal, TerminalOptions};

use super::renderer::RasterRenderer;
use super::state::TerminalState;
use super::theme::{TerminalTheme, TextSettings};
use crate::media::Frame;
use crate::runner::{FrameCapture, TerminalSession};
use crate::Result;

/// Maximum scrollback rows retained by direct Ghostty sessions.
///
/// Betamax state output can include scrollback for testing. Ten thousand rows is intentionally
/// large enough for command demos and snapshot tests while still bounding memory use for runaway
/// terminal output.
const MAX_SCROLLBACK_ROWS: usize = 10_000;

/// Describes a Ghostty-backed capture session.
///
/// The dimensions describe the raw terminal canvas before the runner adds optional margins,
/// rounded corners, or synthetic window bars. Constructors for the nested types do not reject zero
/// values; [`GhosttySession`] clamps the terminal grid to at least one row and column before
/// initializing libghostty-vt. Canvas dimensions are validated by the renderer when frames are
/// captured.
#[derive(Debug, Clone)]
pub struct CaptureRequest {
    /// Output canvas size in pixels, excluding runner decoration.
    ///
    /// Margin, rounded corners, and synthetic window bars are applied after terminal rendering by
    /// the runner. Direct capture users should include only the terminal drawing area here.
    pub canvas: PixelSize,
    /// Terminal grid size in cells.
    ///
    /// Zero rows or columns are accepted at construction time and clamped to one when a
    /// [`GhosttySession`] is opened.
    pub grid: TerminalGrid,
    /// Text metrics and padding used by the software rasterizer.
    pub text: TextSettings,
    /// Target terminal theme used to map libghostty-vt colors into output colors.
    pub theme: TerminalTheme,
}

/// Pixel dimensions for a terminal canvas.
///
/// These dimensions describe only the terminal render area. Runner-level margin, rounded corners,
/// and synthetic window bars are applied later as frame decoration. The constructor is infallible
/// and does not validate that the dimensions can be allocated; allocation and overflow checks are
/// performed when rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl PixelSize {
    /// Constructs pixel dimensions.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Terminal grid dimensions in cells.
///
/// The runner derives this from [`PixelSize`] and [`TextSettings`]. Direct users can provide their
/// own grid when driving [`GhosttySession`] without a tape. The constructor is infallible; opening
/// a [`GhosttySession`] clamps zero rows or columns to one because libghostty-vt expects a
/// non-empty terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalGrid {
    /// Number of terminal columns.
    pub columns: u16,
    /// Number of terminal rows.
    pub rows: u16,
}

impl TerminalGrid {
    /// Constructs terminal grid dimensions.
    pub const fn new(columns: u16, rows: u16) -> Self {
        Self { columns, rows }
    }
}

/// Built-in capture backend used by [`Runner`](crate::Runner).
///
/// This type opens [`GhosttySession`] values backed by `libghostty-vt` plus Betamax's current
/// software rasterizer.
#[derive(Debug, Default)]
pub struct GhosttyFrameCapture;

impl GhosttyFrameCapture {
    /// Open a new libghostty-vt terminal session for capture.
    ///
    /// # Errors
    ///
    /// Returns an error if libghostty-vt fails to allocate or initialize the terminal model.
    pub fn open(&mut self, request: CaptureRequest) -> Result<GhosttySession> {
        <Self as FrameCapture>::open(self, request)
    }
}

impl FrameCapture for GhosttyFrameCapture {
    type Session = GhosttySession;

    fn open(&mut self, request: CaptureRequest) -> Result<Self::Session> {
        GhosttySession::new(request)
    }
}

/// Live terminal session backed by `libghostty-vt`.
///
/// A session accepts raw VT bytes from a PTY, exposes visible screen text for wait matching, can
/// capture raster frames, and can produce structured terminal state. It is lower level than
/// [`Runner`](crate::Runner); most users should prefer the runner unless they need direct control
/// over feeding terminal bytes.
pub struct GhosttySession {
    /// libghostty-vt terminal model receiving PTY bytes.
    ///
    /// Boxed to work around libghostty-vt 0.1.1 unsoundness: `on_pty_write`
    /// records `&self.vtable` (an interior field) as C userdata, so moving the
    /// `Terminal` after install — e.g. returning it from this constructor —
    /// dangles the pointer and segfaults on the next callback fire. Fixed
    /// upstream by Uzaaft/libghostty-rs#24 (boxes the VTable internally); drop
    /// this `Box` once we depend on a release containing that PR.
    terminal: Box<Terminal<'static, 'static>>,
    pending_pty_reply: Rc<Cell<Vec<u8>>>,
    /// Software renderer and reusable render iterators.
    renderer: RasterRenderer,
}

impl std::fmt::Debug for GhosttySession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GhosttySession")
            .field("terminal", &"libghostty-vt terminal")
            .field("renderer", &"software raster renderer")
            .finish_non_exhaustive()
    }
}

impl GhosttySession {
    /// Create and size the libghostty-vt terminal.
    ///
    /// The terminal is resized with both cell and pixel dimensions so libghostty-vt can report
    /// render-state geometry consistently.
    fn new(request: CaptureRequest) -> Result<Self> {
        let cell_width = request.text.cell_width();
        let cell_height = request.text.cell_height();

        let mut terminal = Box::new(
            Terminal::new(TerminalOptions {
                cols: request.grid.columns.max(1),
                rows: request.grid.rows.max(1),
                max_scrollback: MAX_SCROLLBACK_ROWS,
            })
            .map_err(vt_error("failed to create libghostty-vt terminal"))?,
        );
        terminal
            .resize(
                request.grid.columns.max(1),
                request.grid.rows.max(1),
                cell_width,
                cell_height,
            )
            .map_err(vt_error("failed to size libghostty-vt terminal"))?;

        let pending_pty_reply = Rc::new(Cell::new(Vec::new()));
        let reply_for_callback = Rc::clone(&pending_pty_reply);
        terminal
            .on_pty_write(move |_terminal, bytes| {
                let mut buf = reply_for_callback.take();
                buf.extend_from_slice(bytes);
                reply_for_callback.set(buf);
            })
            .map_err(vt_error(
                "failed to install libghostty-vt write_pty callback",
            ))?;

        Ok(Self {
            terminal,
            pending_pty_reply,
            renderer: RasterRenderer::new(request, cell_width, cell_height),
        })
    }
}

impl TerminalSession for GhosttySession {
    /// Feed raw PTY bytes into libghostty-vt.
    fn write_vt(&mut self, bytes: &[u8]) {
        self.terminal.vt_write(bytes);
    }

    fn capture_frame_with_cursor(&mut self, cursor_visible: bool) -> Result<Frame> {
        self.renderer.render(&self.terminal, cursor_visible)
    }

    fn screen_text(&mut self) -> Result<String> {
        self.renderer.screen_text(&self.terminal)
    }

    fn terminal_state(&mut self) -> Result<TerminalState> {
        self.renderer.terminal_state(&mut self.terminal)
    }

    fn take_pending_pty_reply(&mut self) -> Vec<u8> {
        self.pending_pty_reply.take()
    }
}

impl GhosttySession {
    /// Feed raw PTY bytes into libghostty-vt.
    pub fn write_vt(&mut self, bytes: &[u8]) {
        <Self as TerminalSession>::write_vt(self, bytes);
    }

    /// Capture a frame with the cursor visible when libghostty-vt says it is visible.
    ///
    /// # Errors
    ///
    /// Returns an error if libghostty-vt cannot produce render state or if the raster target would
    /// exceed addressable memory for the requested dimensions.
    pub fn capture_frame(&mut self) -> Result<Frame> {
        self.capture_frame_with_cursor(true)
    }

    /// Capture a frame with caller-controlled cursor blink visibility.
    ///
    /// The terminal can still hide the cursor; this flag only suppresses drawing for blink timing.
    ///
    /// # Errors
    ///
    /// Returns an error if libghostty-vt cannot produce render state or if the raster target would
    /// exceed addressable memory for the requested dimensions.
    pub fn capture_frame_with_cursor(&mut self, cursor_visible: bool) -> Result<Frame> {
        <Self as TerminalSession>::capture_frame_with_cursor(self, cursor_visible)
    }

    /// Return visible screen text with spaces for empty cells and newlines between rows.
    ///
    /// # Errors
    ///
    /// Returns an error if libghostty-vt cannot produce render state or cell data.
    pub fn screen_text(&mut self) -> Result<String> {
        <Self as TerminalSession>::screen_text(self)
    }

    /// Return a compact JSON-serializable snapshot of terminal state.
    ///
    /// This includes plain text for easy assertions and span/style data for richer snapshot tests.
    ///
    /// # Errors
    ///
    /// Returns an error if libghostty-vt cannot report viewport, scrollback, style, cursor, title,
    /// or working-directory state.
    pub fn terminal_state(&mut self) -> Result<TerminalState> {
        <Self as TerminalSession>::terminal_state(self)
    }
}

fn vt_error(context: &'static str) -> impl Fn(libghostty_vt::Error) -> miette::Report {
    move |error| miette::miette!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_request() -> CaptureRequest {
        CaptureRequest {
            canvas: PixelSize::new(640, 360),
            grid: TerminalGrid::new(80, 24),
            text: TextSettings::default(),
            theme: TerminalTheme::default(),
        }
    }

    #[test]
    fn cpr_query_produces_forwardable_reply() {
        // We assert shape (CSI...R), not exact bytes — libghostty owns the format.
        let mut session = GhosttySession::new(small_request()).expect("open session");
        session.write_vt(b"\x1b[6n");
        let reply = session.take_pending_pty_reply();
        assert!(
            reply.starts_with(b"\x1b["),
            "reply must begin with CSI, got {reply:?}"
        );
        assert!(
            reply.ends_with(b"R"),
            "CPR reply must end with R, got {reply:?}"
        );
    }
}
