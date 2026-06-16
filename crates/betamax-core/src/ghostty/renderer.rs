//! Software renderer for libghostty-vt render state.

use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style as TextStyle, SwashCache,
    Weight,
};
use libghostty_vt::render::{CellIterator, RowIterator};
use libghostty_vt::style::{RgbColor, Style};
use libghostty_vt::terminal::ScrollViewport;
use libghostty_vt::{RenderState, Terminal};
use miette::miette;

use super::engine::CaptureRequest;
use super::render_theme::{style_colors, RenderTheme};
use super::state::{
    compact_row, default_style, full_state_style, pending_rows_text, state_style, trim_empty_rows,
    PendingStateSpan, StateCellSnapshot, StateCursor, StyleTable, TerminalState,
};
use super::target::PixelTarget;
use super::theme::TextSettings;
use crate::media::Frame;
use crate::Result;

/// Minimum font size divisor used for letter-spacing normalization.
const MIN_FONT_SIZE_FOR_SPACING: f32 = 1.0;

/// Software renderer for libghostty-vt terminal state.
///
/// This is the current substitute for a future libghostty off-screen renderer. It reuses
/// libghostty-vt render iterators and text rasterization caches across frames so repeated GIF/video
/// captures do not rebuild the whole rendering stack for every PTY drain interval.
pub(super) struct RasterRenderer {
    /// Capture request retained for canvas, text, and theme settings.
    request: CaptureRequest,
    /// Cached cell width derived from text settings.
    cell_width: u32,
    /// Cached cell height derived from text settings.
    cell_height: u32,
    /// Reusable libghostty-vt render state.
    render_state: RenderState<'static>,
    /// Reusable row iterator.
    rows: RowIterator<'static>,
    /// Reusable cell iterator.
    cells: CellIterator<'static>,
    /// Text rasterizer and glyph caches.
    text_renderer: TextRenderer,
}

impl RasterRenderer {
    /// Create a renderer with reusable libghostty-vt and font resources.
    pub(super) fn new(request: CaptureRequest, cell_width: u32, cell_height: u32) -> Self {
        tracing::trace!("creating reusable libghostty-vt render helpers");
        let text_renderer = TextRenderer::new(request.text.clone(), cell_width, cell_height);
        Self {
            request,
            cell_width,
            cell_height,
            render_state: RenderState::new().expect("libghostty-vt render state"),
            rows: RowIterator::new().expect("libghostty-vt row iterator"),
            cells: CellIterator::new().expect("libghostty-vt cell iterator"),
            text_renderer,
        }
    }

    /// Render the visible terminal viewport to an RGBA frame.
    ///
    /// libghostty-vt provides rows, cells, styles, resolved colors, and cursor metadata. This
    /// method maps colors into the selected theme, paints cell backgrounds, draws text with
    /// cosmic-text, and overlays the cursor when requested.
    ///
    /// The result is the raw terminal canvas only. Runner-level decoration such as margins,
    /// rounded corners, and synthetic window bars is applied after this method returns.
    pub(super) fn render(
        &mut self,
        terminal: &Terminal<'static, 'static>,
        cursor_visible: bool,
    ) -> Result<Frame> {
        let span = tracing::trace_span!(
            "ghostty_render_frame",
            cursor_visible,
            canvas.width = self.request.canvas.width,
            canvas.height = self.request.canvas.height,
            cell.width = self.cell_width,
            cell.height = self.cell_height,
        );
        let _enter = span.enter();
        let mut target = PixelTarget::new(self.request.canvas.width, self.request.canvas.height)?;
        tracing::trace!("updating libghostty-vt render state");
        let snapshot = self
            .render_state
            .update(terminal)
            .map_err(vt_error("failed to update libghostty-vt render state"))?;
        tracing::trace!("updated libghostty-vt render state");
        tracing::trace!("reading libghostty-vt colors");
        let colors = snapshot
            .colors()
            .map_err(vt_error("failed to read libghostty-vt colors"))?;
        tracing::trace!("read libghostty-vt colors");
        let theme = RenderTheme::new(
            &self.request.theme,
            colors.background,
            colors.foreground,
            colors.palette,
        );
        target.clear(theme.background);

        {
            tracing::trace!("iterating libghostty-vt render rows");
            let mut row_iter = self
                .rows
                .update(&snapshot)
                .map_err(vt_error("failed to iterate libghostty-vt rows"))?;
            let mut y = 0u16;
            while let Some(row) = row_iter.next() {
                let mut cell_iter = self
                    .cells
                    .update(row)
                    .map_err(vt_error("failed to iterate libghostty-vt cells"))?;
                let mut x = 0u16;
                while let Some(cell) = cell_iter.next() {
                    let style = cell
                        .style()
                        .map_err(vt_error("failed to read libghostty-vt cell style"))?;
                    let background = cell
                        .bg_color()
                        .map_err(vt_error("failed to read libghostty-vt background"))?
                        .unwrap_or(colors.background);
                    let foreground = cell
                        .fg_color()
                        .map_err(vt_error("failed to read libghostty-vt foreground"))?
                        .unwrap_or(colors.foreground);
                    let graphemes = cell
                        .graphemes()
                        .map_err(vt_error("failed to read libghostty-vt graphemes"))?;

                    let (foreground, background) =
                        style_colors(style, foreground, background, colors.background);
                    let foreground = theme.map_color(foreground);
                    let background = theme.map_color(background);
                    let x_px = self.request.text.padding + u32::from(x) * self.cell_width;
                    let y_px = self.request.text.padding + u32::from(y) * self.cell_height;
                    target.fill_rect(x_px, y_px, self.cell_width, self.cell_height, background);
                    if !style.invisible && !graphemes.is_empty() {
                        let text: String = graphemes.into_iter().collect();
                        self.text_renderer.draw_text(
                            &mut target,
                            &text,
                            x_px,
                            y_px,
                            foreground,
                            style,
                        );
                    }

                    x = x.saturating_add(1);
                }
                y = y.saturating_add(1);
            }
            tracing::trace!(rows = y, "iterated libghostty-vt render rows");
        }

        if cursor_visible
            && snapshot
                .cursor_visible()
                .map_err(vt_error("failed to read libghostty-vt cursor visibility"))?
        {
            tracing::trace!("reading libghostty-vt cursor");
            if let Some(cursor) = snapshot
                .cursor_viewport()
                .map_err(vt_error("failed to read libghostty-vt cursor position"))?
            {
                let color = snapshot
                    .cursor_color()
                    .map_err(vt_error("failed to read libghostty-vt cursor color"))?
                    .map(|color| theme.map_color(color))
                    .unwrap_or(theme.cursor);
                let style = snapshot
                    .cursor_visual_style()
                    .map_err(vt_error("failed to read libghostty-vt cursor style"))?;
                target.draw_cursor(
                    self.request.text.padding + u32::from(cursor.x) * self.cell_width,
                    self.request.text.padding + u32::from(cursor.y) * self.cell_height,
                    self.cell_width,
                    self.cell_height,
                    color,
                    style,
                );
            }
            tracing::trace!("read libghostty-vt cursor");
        }

        Ok(target.into_frame())
    }

    /// Extract visible screen text from the render state.
    ///
    /// Empty cells become spaces so substring matching sees the same column layout as the renderer.
    pub(super) fn screen_text(&mut self, terminal: &Terminal<'static, 'static>) -> Result<String> {
        let span = tracing::trace_span!("ghostty_screen_text");
        let _enter = span.enter();
        tracing::trace!("updating libghostty-vt render state for screen text");
        let snapshot = self
            .render_state
            .update(terminal)
            .map_err(vt_error("failed to update libghostty-vt render state"))?;
        tracing::trace!("updated libghostty-vt render state for screen text");
        let mut text = String::new();
        tracing::trace!("iterating libghostty-vt rows for screen text");
        let mut row_iter = self
            .rows
            .update(&snapshot)
            .map_err(vt_error("failed to iterate libghostty-vt rows"))?;

        while let Some(row) = row_iter.next() {
            let mut cell_iter = self
                .cells
                .update(row)
                .map_err(vt_error("failed to iterate libghostty-vt cells"))?;
            while let Some(cell) = cell_iter.next() {
                let graphemes = cell
                    .graphemes()
                    .map_err(vt_error("failed to read libghostty-vt graphemes"))?;
                if graphemes.is_empty() {
                    text.push(' ');
                } else {
                    text.extend(graphemes);
                }
            }
            text.push('\n');
        }

        tracing::trace!(
            bytes = text.len(),
            "built screen text from libghostty-vt rows"
        );
        Ok(text)
    }

    /// Build a compact terminal state snapshot suitable for JSON output and snapshot tests.
    ///
    /// libghostty-vt exposes scrollback through viewport scrolling. This method walks from the top
    /// of scrollback to the bottom, then restores the viewport before returning.
    ///
    /// The method reads the terminal in two forms: plain text for ergonomic string assertions and
    /// compact styled rows for snapshot tests. The styled form keeps the default style expanded and
    /// interns non-default styles into a side table, which keeps JSON shorter than per-cell style
    /// objects without losing style boundaries.
    pub(super) fn terminal_state(
        &mut self,
        terminal: &mut Terminal<'static, 'static>,
    ) -> Result<TerminalState> {
        let span = tracing::trace_span!("ghostty_terminal_state");
        let _enter = span.enter();
        tracing::trace!("reading libghostty-vt terminal metadata");
        let columns = terminal
            .cols()
            .map_err(vt_error("failed to read terminal columns"))?;
        let rows = terminal
            .rows()
            .map_err(vt_error("failed to read terminal rows"))?;
        let total_rows = terminal
            .total_rows()
            .map_err(vt_error("failed to read terminal row count"))?;
        let scrollback_rows = terminal
            .scrollback_rows()
            .map_err(vt_error("failed to read terminal scrollback rows"))?;
        let title = terminal
            .title()
            .map_err(vt_error("failed to read terminal title"))?
            .to_string();
        let working_directory = terminal
            .pwd()
            .map_err(vt_error("failed to read terminal working directory"))?
            .to_string();
        let cursor = StateCursor {
            x: terminal
                .cursor_x()
                .map_err(vt_error("failed to read cursor column"))?,
            y: terminal
                .cursor_y()
                .map_err(vt_error("failed to read cursor row"))?,
            visible: terminal
                .is_cursor_visible()
                .map_err(vt_error("failed to read cursor visibility"))?,
        };
        tracing::trace!(
            columns,
            rows,
            total_rows,
            scrollback_rows,
            "read libghostty-vt terminal metadata",
        );

        let mut all_rows = Vec::new();
        tracing::trace!("scrolling libghostty-vt viewport to collect state rows");
        terminal.scroll_viewport(ScrollViewport::Top);
        let top_rows = self.state_rows(terminal)?;
        all_rows.extend(top_rows);
        for _ in 1..=scrollback_rows {
            terminal.scroll_viewport(ScrollViewport::Delta(1));
            let rows = self.state_rows(terminal)?;
            if let Some(row) = rows.last() {
                all_rows.push(row.clone());
            }
        }
        terminal.scroll_viewport(ScrollViewport::Bottom);
        tracing::trace!("restored libghostty-vt viewport after state row collection");

        let viewport = trim_empty_rows(self.state_rows(terminal)?);
        let viewport_text = pending_rows_text(&viewport);
        let scrollback = all_rows
            .len()
            .checked_sub(usize::from(rows))
            .map(|viewport_start| trim_empty_rows(all_rows[..viewport_start].to_vec()))
            .unwrap_or_default();
        let scrollback_text = pending_rows_text(&scrollback);
        tracing::trace!("updating libghostty-vt render state for state colors");
        let snapshot = self
            .render_state
            .update(terminal)
            .map_err(vt_error("failed to update libghostty-vt render state"))?;
        tracing::trace!("updated libghostty-vt render state for state colors");
        let colors = snapshot
            .colors()
            .map_err(vt_error("failed to read libghostty-vt colors"))?;
        let theme = RenderTheme::new(
            &self.request.theme,
            colors.background,
            colors.foreground,
            colors.palette,
        );
        let default = default_style(&theme);
        let mut styles = StyleTable::new(default.clone());
        let viewport = styles.intern_pending_rows(viewport);
        let scrollback = styles.intern_pending_rows(scrollback);

        Ok(TerminalState {
            size: [columns, rows],
            total_rows,
            scrollback_rows,
            title,
            working_directory,
            cursor,
            default_style: state_style(&default),
            styles: styles.styles,
            viewport_text,
            scrollback_text,
            viewport,
            scrollback,
        })
    }

    /// Convert the currently visible viewport into pending styled rows.
    ///
    /// Rows are "pending" because styles are still full values. A later interning step compares
    /// them against the default style and emits compact JSON spans.
    fn state_rows(
        &mut self,
        terminal: &Terminal<'static, 'static>,
    ) -> Result<Vec<Vec<PendingStateSpan>>> {
        tracing::trace!("updating libghostty-vt render state for state rows");
        let snapshot = self
            .render_state
            .update(terminal)
            .map_err(vt_error("failed to update libghostty-vt render state"))?;
        tracing::trace!("updated libghostty-vt render state for state rows");
        let colors = snapshot
            .colors()
            .map_err(vt_error("failed to read libghostty-vt colors"))?;
        let theme = RenderTheme::new(
            &self.request.theme,
            colors.background,
            colors.foreground,
            colors.palette,
        );
        let mut rows = Vec::new();
        let mut row_iter = self
            .rows
            .update(&snapshot)
            .map_err(vt_error("failed to iterate libghostty-vt rows"))?;

        while let Some(row) = row_iter.next() {
            let mut cells = Vec::new();
            let mut cell_iter = self
                .cells
                .update(row)
                .map_err(vt_error("failed to iterate libghostty-vt cells"))?;
            while let Some(cell) = cell_iter.next() {
                let style = cell
                    .style()
                    .map_err(vt_error("failed to read libghostty-vt cell style"))?;
                let background = cell
                    .bg_color()
                    .map_err(vt_error("failed to read libghostty-vt background"))?
                    .unwrap_or(colors.background);
                let foreground = cell
                    .fg_color()
                    .map_err(vt_error("failed to read libghostty-vt foreground"))?
                    .unwrap_or(colors.foreground);
                let graphemes = cell
                    .graphemes()
                    .map_err(vt_error("failed to read libghostty-vt graphemes"))?;
                let (foreground, background) =
                    style_colors(style, foreground, background, colors.background);
                let foreground = theme.map_color(foreground);
                let background = theme.map_color(background);
                let cell_text = if graphemes.is_empty() {
                    " ".to_string()
                } else {
                    graphemes.into_iter().collect()
                };
                cells.push(StateCellSnapshot {
                    text: cell_text,
                    style: full_state_style(style, foreground, background),
                });
            }
            rows.push(compact_row(cells));
        }

        tracing::trace!(
            rows = rows.len(),
            "built state rows from libghostty-vt rows"
        );
        Ok(rows)
    }
}

/// Text rasterizer used by [`RasterRenderer`].
///
/// This type owns the font-system and glyph caches so terminal iteration can borrow render-state
/// fields while text drawing mutates only this disjoint field. Keeping this as a real renderer
/// concept avoids passing every font, metric, and cache value through each cell draw call.
struct TextRenderer {
    /// Text metrics and font choice used for every terminal cell in this session.
    settings: TextSettings,
    /// Cached terminal cell width in pixels.
    cell_width: u32,
    /// Cached terminal cell height in pixels.
    cell_height: u32,
    /// Font database and shaping context.
    font_system: FontSystem,
    /// Glyph raster cache.
    swash_cache: SwashCache,
}

impl TextRenderer {
    /// Create a text renderer for one capture session.
    fn new(settings: TextSettings, cell_width: u32, cell_height: u32) -> Self {
        Self {
            settings,
            cell_width,
            cell_height,
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    /// Draw one terminal cell's text into the pixel target.
    ///
    /// The buffer width is deliberately wider than one cell to avoid clipping glyphs with
    /// overhangs. The terminal model still advances by one cell because libghostty-vt already
    /// decided cell occupancy.
    fn draw_text(
        &mut self,
        target: &mut PixelTarget,
        text: &str,
        x: u32,
        y: u32,
        color: RgbColor,
        style: Style,
    ) {
        let text_settings = &self.settings;
        let metrics = Metrics::new(text_settings.font_size, self.cell_height as f32);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut self.font_system);
        let family = text_settings
            .font_family
            .as_deref()
            .map(Family::Name)
            .unwrap_or(Family::Monospace);
        let attrs = Attrs::new()
            .family(family)
            .letter_spacing(
                text_settings.letter_spacing
                    / text_settings.font_size.max(MIN_FONT_SIZE_FOR_SPACING),
            )
            .weight(if style.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .style(if style.italic {
                TextStyle::Italic
            } else {
                TextStyle::Normal
            });

        buffer.set_size(
            Some(self.cell_width as f32 * 2.0),
            Some(self.cell_height as f32),
        );
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.draw(
            &mut self.swash_cache,
            Color::rgb(color.r, color.g, color.b),
            |glyph_x, glyph_y, width, height, glyph_color| {
                target.blend_rect(
                    x as i32 + glyph_x,
                    y as i32 + glyph_y,
                    width,
                    height,
                    glyph_color,
                );
            },
        );
    }
}

fn vt_error(context: &'static str) -> impl Fn(libghostty_vt::Error) -> miette::Report {
    move |error| miette!("{context}: {error}")
}
