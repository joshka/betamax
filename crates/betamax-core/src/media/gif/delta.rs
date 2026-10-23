//! Frame differencing for animated GIFs.
//!
//! Terminal recordings change a few cells per frame, so storing every frame as a full canvas spends
//! most of the file on pixels a viewer already shows. [`FrameDelta`] tracks what a decoder displays
//! and turns each full indexed frame into the smallest rectangle that changed. Inside that
//! rectangle, pixels that already show the right color use the transparent index, which leaves the
//! previous pixel visible and gives LZW long runs to compress. Frames identical to the displayed
//! canvas extend the previous frame's delay instead of adding a frame.
//!
//! The transparent index therefore means "keep" as well as "clear". A pixel that must become clear
//! while the canvas shows an opaque color cannot be expressed with "keep", so the previous frame is
//! widened to cover it and disposed to the background (clear) before the next frame is drawn.

use std::time::Duration;

use ::gif::DisposalMethod;

use super::{gif_delay, gif_units, GIF_TRANSPARENT_INDEX, MIN_GIF_DELAY_UNITS};

/// Rectangle of a GIF frame within the logical screen, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rect {
    pub(super) left: usize,
    pub(super) top: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl Rect {
    /// Smallest rectangle containing both `self` and `other`.
    fn union(self, other: Self) -> Self {
        let left = self.left.min(other.left);
        let top = self.top.min(other.top);
        let right = (self.left + self.width).max(other.left + other.width);
        let bottom = (self.top + self.height).max(other.top + other.height);
        Self {
            left,
            top,
            width: right - left,
            height: bottom - top,
        }
    }

    /// Canvas offsets of each row inside the rectangle, for a canvas `stride` pixels wide.
    fn rows(self, stride: usize) -> impl Iterator<Item = std::ops::Range<usize>> {
        (self.top..self.top + self.height).map(move |y| {
            let start = y * stride + self.left;
            start..start + self.width
        })
    }
}

/// One encoded GIF frame: indexed pixels for `rect`, with its delay and disposal.
#[derive(Debug)]
pub(super) struct DeltaFrame {
    pub(super) rect: Rect,
    pub(super) buffer: Vec<u8>,
    pub(super) delay: u16,
    pub(super) dispose: DisposalMethod,
}

/// Frame written last, held back until the next frame settles its delay and disposal.
#[derive(Debug)]
struct Pending {
    rect: Rect,
    delay: Duration,
    dispose: DisposalMethod,
}

/// Converts full indexed frames into cropped GIF frames that reproduce them when composited.
///
/// Feed every frame through [`Self::push`], then call [`Self::finish`]. When the palette reserves
/// [`GIF_TRANSPARENT_INDEX`], indices equal to it are clear pixels and unchanged pixels reuse it.
/// Otherwise frames are only cropped, and every index is an opaque color.
pub(super) struct FrameDelta {
    width: usize,
    /// Whether [`GIF_TRANSPARENT_INDEX`] is reserved for clear and kept pixels.
    transparent: bool,
    /// Canvas before the pending frame is drawn, after earlier disposals.
    under: Vec<u8>,
    /// Canvas after the pending frame is drawn.
    canvas: Vec<u8>,
    pending: Option<Pending>,
    /// First frame, kept when it has clear pixels so the loop back to it can clear them again.
    first_with_clear: Option<Vec<u8>>,
    /// Captured time covered by settled frames, before rounding.
    elapsed: Duration,
    /// Hundredths of a second already written, so rounding error does not accumulate.
    written: u128,
}

impl FrameDelta {
    /// Start with a clear canvas of `width` × `height` pixels, as decoders do.
    ///
    /// `transparent` says whether the palette reserves [`GIF_TRANSPARENT_INDEX`].
    pub(super) fn new(width: usize, height: usize, transparent: bool) -> Self {
        let clear = vec![GIF_TRANSPARENT_INDEX; width * height];
        Self {
            width,
            transparent,
            under: clear.clone(),
            canvas: clear,
            pending: None,
            first_with_clear: None,
            elapsed: Duration::ZERO,
            written: 0,
        }
    }

    /// Add the next full-canvas frame, returning the previous frame once its encoding is final.
    pub(super) fn push(&mut self, frame: Vec<u8>, delay: Duration) -> Option<DeltaFrame> {
        debug_assert_eq!(frame.len(), self.canvas.len());
        let first = self.pending.is_none();
        if let Some(pending) = &mut self.pending {
            let merged = pending.delay.saturating_add(delay);
            if frame == self.canvas && gif_delay(merged) < u16::MAX {
                pending.delay = merged;
                return None;
            }
        } else if self.transparent && frame.contains(&GIF_TRANSPARENT_INDEX) {
            self.first_with_clear = Some(frame.clone());
        }

        let written = self.settle(&frame);
        let changed = if first && !self.transparent {
            // Without a clear index, the clear starting canvas is not comparable to any frame.
            Some(Rect {
                left: 0,
                top: 0,
                width: self.width,
                height: frame.len() / self.width,
            })
        } else {
            changed_bounds(self.width, &self.under, &frame, |under, next| under != next)
        };
        self.canvas = frame;
        self.pending = Some(Pending {
            // An unchanged frame still needs a pixel to carry its delay.
            rect: changed.unwrap_or(Rect {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            }),
            delay,
            dispose: DisposalMethod::Keep,
        });
        written
    }

    /// Return the last frame, disposed so that looping back to the first frame shows it exactly.
    pub(super) fn finish(mut self) -> Option<DeltaFrame> {
        let first = self.first_with_clear.take();
        match first {
            Some(first) => self.settle(&first),
            None => self.settle_pending(),
        }
    }

    /// Finalize the pending frame before `next` is drawn and update `under` to match.
    fn settle(&mut self, next: &[u8]) -> Option<DeltaFrame> {
        let clear = self.transparent.then(|| {
            changed_bounds(self.width, &self.canvas, next, |shown, next| {
                next == GIF_TRANSPARENT_INDEX && shown != GIF_TRANSPARENT_INDEX
            })
        });
        if let (Some(Some(clear)), Some(pending)) = (clear, &mut self.pending) {
            pending.rect = pending.rect.union(clear);
            pending.dispose = DisposalMethod::Background;
        }
        let written = self.settle_pending();
        self.under.clone_from(&self.canvas);
        if let Some(DeltaFrame {
            rect,
            dispose: DisposalMethod::Background,
            ..
        }) = &written
        {
            for row in rect.rows(self.width) {
                self.under[row].fill(GIF_TRANSPARENT_INDEX);
            }
        }
        written
    }

    /// Convert a frame hold to GIF units by rounding its cumulative end time.
    ///
    /// Truncating each hold separately loses up to 10 ms per frame, which shortened typing and
    /// other dense stretches by roughly a quarter. Rounding the running total keeps every frame
    /// within 10 ms of its captured timestamp. Frames reach here at least 20 ms long, so only a
    /// short last frame is stretched to the minimum.
    fn settle_delay(&mut self, delay: Duration) -> u16 {
        self.elapsed = self.elapsed.saturating_add(delay);
        let end = gif_units(self.elapsed).max(self.written + MIN_GIF_DELAY_UNITS);
        let units = u16::try_from(end - self.written).unwrap_or(u16::MAX);
        self.written += u128::from(units);
        units
    }

    /// Encode the pending frame from the canvas before and after it was drawn.
    fn settle_pending(&mut self) -> Option<DeltaFrame> {
        let pending = self.pending.take()?;
        let mut buffer = Vec::with_capacity(pending.rect.width * pending.rect.height);
        for row in pending.rect.rows(self.width) {
            buffer.extend(self.under[row.clone()].iter().zip(&self.canvas[row]).map(
                |(&under, &shown)| {
                    if self.transparent && under == shown {
                        GIF_TRANSPARENT_INDEX
                    } else {
                        shown
                    }
                },
            ));
        }
        Some(DeltaFrame {
            rect: pending.rect,
            buffer,
            delay: self.settle_delay(pending.delay),
            dispose: pending.dispose,
        })
    }
}

/// Bounding box of pixels where `hit(a, b)` holds; `hit` must be false wherever `a == b`.
fn changed_bounds(width: usize, a: &[u8], b: &[u8], hit: impl Fn(u8, u8) -> bool) -> Option<Rect> {
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for (y, (a_row, b_row)) in a.chunks_exact(width).zip(b.chunks_exact(width)).enumerate() {
        if a_row == b_row {
            continue;
        }
        let hits = || a_row.iter().zip(b_row).map(|(&a, &b)| hit(a, b));
        let Some(first) = hits().position(|hit| hit) else {
            continue;
        };
        let last = width - 1 - hits().rev().position(|hit| hit).unwrap_or(0);
        bounds = Some(match bounds {
            None => (first, y, last, y),
            Some((left, top, right, _)) => (left.min(first), top, right.max(last), y),
        });
    }
    bounds.map(|(left, top, right, bottom)| Rect {
        left,
        top,
        width: right - left + 1,
        height: bottom - top + 1,
    })
}

#[cfg(test)]
mod tests;
