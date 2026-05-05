//! libghostty-vt integration and software raster rendering.
//!
//! Betamax uses libghostty-vt as the terminal model and render-state source. Because libghostty
//! does not yet expose a stable off-screen screenshot renderer, this module currently rasterizes
//! the libghostty-vt render state with `cosmic-text` and `swash`. The important contract is that
//! terminal parsing, scrollback, styles, colors, cursor state, title, and working directory come
//! from libghostty-vt rather than a JavaScript terminal emulator.
//!
//! # Examples
//!
//! ```
//! use betamax_core::ghostty::{theme_names, PixelSize, TerminalGrid, TerminalTheme};
//!
//! # fn main() -> betamax_core::Result<()> {
//! let names = theme_names()?;
//! assert!(names.iter().any(|name| name == "Aardvark Blue"));
//!
//! let theme = TerminalTheme::from_name("Aardvark Blue")?;
//! assert_eq!(theme.background_hex(), "#102040");
//!
//! let canvas = PixelSize::new(1200, 600);
//! let grid = TerminalGrid::new(80, 24);
//! assert_eq!(canvas.width, 1200);
//! assert_eq!(grid.rows, 24);
//! # Ok(())
//! # }
//! ```

mod color;
mod engine;
mod render_theme;
mod renderer;
mod state;
mod target;
mod theme;

#[doc(inline)]
pub use engine::{CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, TerminalGrid};
#[doc(inline)]
pub use state::{StateCursor, StateRow, StateSpan, StateStyle, TerminalState};
#[doc(inline)]
pub use theme::{theme_names, TerminalTheme, TextSettings};
