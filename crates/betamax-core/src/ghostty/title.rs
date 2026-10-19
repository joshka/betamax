//! Ordered title notifications and event-boundary raster capture from Ghostty.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use libghostty_vt::Terminal;
use miette::miette;

use super::renderer::RasterRenderer;
use crate::runner::TitleChange;
use crate::Result;

/// Callback state shared with a session. No renderer borrow is held across `vt_write`.
#[derive(Debug, Default)]
pub(super) struct TitleCapture {
    cursor: Cell<Option<bool>>,
    changes: RefCell<Vec<Result<TitleChange>>>,
}

impl TitleCapture {
    pub(super) fn install(
        self: &Rc<Self>,
        terminal: &mut Terminal<'static, 'static>,
        renderer: Rc<RefCell<RasterRenderer>>,
    ) -> Result<()> {
        let capture = Rc::clone(self);
        terminal
            .on_title_changed(move |terminal| {
                // Failures cannot unwind through the C callback. Return them at the next runner
                // drain.
                let change = capture.snapshot(terminal, &renderer);
                capture.changes.borrow_mut().push(change);
            })
            .map_err(|error| miette!("failed to install title callback: {error}"))?;
        Ok(())
    }

    pub(super) fn set_cursor(&self, cursor: Option<bool>) {
        self.cursor.set(cursor);
    }

    pub(super) fn take(&self) -> Result<Vec<TitleChange>> {
        self.changes.take().into_iter().collect()
    }

    fn snapshot(
        &self,
        terminal: &Terminal<'static, 'static>,
        renderer: &RefCell<RasterRenderer>,
    ) -> Result<TitleChange> {
        let title = terminal
            .title()
            .map_err(|error| miette!("failed to read title event: {error}"))?
            .to_owned();
        let frame = match self.cursor.get() {
            Some(cursor) => {
                let mut renderer = renderer
                    .try_borrow_mut()
                    .map_err(|error| miette!("title renderer is already borrowed: {error}"))?;
                Some(renderer.render(terminal, cursor)?)
            }
            None => None,
        };
        Ok(TitleChange { title, frame })
    }
}

#[cfg(test)]
mod tests {
    use crate::ghostty::{
        CaptureRequest, GhosttyFrameCapture, GhosttySession, PixelSize, TerminalGrid,
        TerminalTheme, TextSettings,
    };
    use crate::runner::TerminalSession;

    fn session() -> GhosttySession {
        let mut session = GhosttyFrameCapture
            .open(CaptureRequest {
                canvas: PixelSize::new(160, 80),
                grid: TerminalGrid::new(12, 3),
                text: TextSettings {
                    padding: 0,
                    ..TextSettings::default()
                },
                theme: TerminalTheme::default(),
            })
            .unwrap();
        session.enable_title_capture().unwrap();
        session.set_title_capture_cursor(Some(false));
        session
    }

    #[test]
    fn title_callbacks_capture_each_screen_even_inside_one_write() {
        let mut terminal = session();
        terminal.write_vt(b"A\x1b]0;first\x07B\x1b]2;second\x1b\\C");
        let changes = terminal.take_title_changes().unwrap();
        assert_eq!(
            changes
                .iter()
                .map(|change| change.title.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        let mut reference = session();
        for (change, bytes) in changes.iter().zip([b"A".as_slice(), b"B".as_slice()]) {
            reference.write_vt(bytes);
            let expected = reference.capture_frame_with_cursor(false).unwrap();
            assert_eq!(change.frame.as_ref().unwrap().pixels, expected.pixels);
        }
        assert_ne!(
            changes[0].frame.as_ref().unwrap().pixels,
            changes[1].frame.as_ref().unwrap().pixels
        );
        assert_eq!(terminal.terminal_state().unwrap().title, "second");
    }

    #[test]
    fn title_events_ignore_chunk_partitioning_and_observe_empty_and_repeated_values() {
        let bytes = b"\x1b]0;A\x07\x1b]2;B\x1b\\\x1b]2;B\x07\x1b]1;icon\x07\x1b]2;\x07";
        let mut whole = session();
        whole.write_vt(bytes);
        let expected = whole.take_title_changes().unwrap();
        assert_eq!(
            expected
                .iter()
                .map(|change| change.title.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "B", ""]
        );
        let mut split = session();
        for byte in bytes {
            split.write_vt(&[*byte]);
        }
        let actual = split.take_title_changes().unwrap();
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.title, expected.title);
            assert_eq!(
                actual.frame.as_ref().unwrap().pixels,
                expected.frame.as_ref().unwrap().pixels
            );
        }
        assert_eq!(actual.len(), expected.len());
        split.write_vt(b"\x1b]2;unfinished");
        assert!(split.take_title_changes().unwrap().is_empty());
    }

    #[test]
    fn hidden_title_events_do_not_render_but_still_update_raw_state() {
        let mut terminal = session();
        terminal.set_title_capture_cursor(None);
        terminal.write_vt(b"\x1b]2;hidden\x07\x1b[6n");
        let changes = terminal.take_title_changes().unwrap();
        assert_eq!(changes[0].title, "hidden");
        assert!(changes[0].frame.is_none());
        assert_eq!(terminal.terminal_state().unwrap().title, "hidden");
        assert!(!terminal.take_pending_pty_reply().is_empty());
    }
}
