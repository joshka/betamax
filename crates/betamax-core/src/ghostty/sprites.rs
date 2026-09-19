//! Adapter for the published Ghostty-derived sprite rasterizer.
//!
//! `qwertty-term-sprite` owns character coverage and geometry: box drawing (including rounded,
//! double, dashed and diagonal strokes), blocks/shades/quadrants, Braille, and selected geometric,
//! Powerline, branch and legacy-computing symbols. Its dispatch table is authoritative; we do not
//! duplicate ranges or rounding rules here. Ordinary text and multi-codepoint graphemes retain
//! cosmic-text shaping. Sprite strokes come from the codepoint, not bold/italic font styles.
//! Cursor/decorations remain in Betamax's existing rendering paths.

use std::collections::HashMap;

use libghostty_vt::style::RgbColor;
use qwertty_term_sprite::{Glyph, Metrics};

use super::target::PixelTarget;

/// Session-local, color-independent sprite cache. Metrics never change during a capture session,
/// so the character alone identifies a bitmap; foreground changes reuse the same alpha coverage.
/// Only supported characters enter the cache, bounding it to the dependency's finite repertoire.
pub(super) struct SpriteRenderer {
    metrics: Metrics,
    glyphs: HashMap<char, Glyph>,
}

impl SpriteRenderer {
    pub(super) fn new(cell_width: u32, cell_height: u32) -> Self {
        Self {
            // These are the exact integer dimensions used for backgrounds, rows and columns,
            // including configured letter/line spacing. We have no measured decoration metrics;
            // use the crate's documented thickness heuristic (height / 12, minimum one pixel).
            // Fraction rounding, curve antialiasing and junction alignment belong to the crate.
            metrics: Metrics::simple(cell_width, cell_height),
            glyphs: HashMap::new(),
        }
    }

    /// Return true when this grapheme was handled, including intentionally blank sprites.
    /// Unsupported characters and combining sequences return false without painting anything.
    pub(super) fn draw(
        &mut self,
        target: &mut PixelTarget,
        text: &str,
        origin: (u32, u32),
        color: RgbColor,
    ) -> bool {
        let mut characters = text.chars();
        let Some(character) = characters.next() else {
            return false;
        };
        if characters.next().is_some() || !qwertty_term_sprite::has_codepoint(character as u32) {
            return false;
        }
        let glyph = self.glyphs.entry(character).or_insert_with(|| {
            qwertty_term_sprite::render(character as u32, &self.metrics)
                .expect("supported sprite codepoint")
        });

        // Verified against 0.4.0's Canvas::into_glyph, rather than Glyph's ambiguous baseline docs:
        // region_height = cell_height + 2*padding_y - clip_top - clip_bottom
        // offset_y = region_height + clip_bottom - padding_y
        // Therefore cell_height - offset_y = clip_top - padding_y (bitmap top relative to cell).
        // No text baseline adjustment is needed. Negative origins are valid for overhang, and
        // trimming is already reflected in the offsets; do not center or stretch the bitmap.
        let x = origin.0 as i32 + glyph.offset_x;
        let y = origin.1 as i32 + self.metrics.cell_height as i32 - glyph.offset_y;
        target.blend_alpha((x, y), (glyph.width, glyph.height), &glyph.alpha, color);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: RgbColor = RgbColor {
        r: 255,
        g: 255,
        b: 255,
    };
    const BLACK: RgbColor = RgbColor { r: 0, g: 0, b: 0 };

    #[test]
    fn blocks_use_cell_bottom_offsets_not_text_baseline() {
        for (width, height) in [(12, 24), (13, 25), (12, 25), (13, 24)] {
            for character in ["█", "▀", "▄"] {
                let mut renderer = SpriteRenderer::new(width, height);
                // A deliberately implausible baseline makes an accidental baseline adjustment fail.
                renderer.metrics.cell_baseline = 1;
                let mut target = PixelTarget::new(width + 8, height + 8).unwrap();
                target.clear(BLACK);
                assert!(renderer.draw(&mut target, character, (4, 4), WHITE));
                let frame = target.into_frame();
                let half = height.div_ceil(2);
                for y in 0..frame.height {
                    for x in 0..frame.width {
                        let inside_x = (4..4 + width).contains(&x);
                        let inside_y = match character {
                            "▀" => (4..4 + half).contains(&y),
                            "▄" => (4 + height - half..4 + height).contains(&y),
                            _ => (4..4 + height).contains(&y),
                        };
                        let expected = if inside_x && inside_y { 255 } else { 0 };
                        assert_eq!(
                            frame.pixels[((y * frame.width + x) * 4) as usize],
                            expected,
                            "{character} at ({x}, {y}) in {width}×{height}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cached_coverage_is_recolored_and_blank_sprites_are_handled() {
        let mut renderer = SpriteRenderer::new(12, 24);
        let mut target = PixelTarget::new(24, 24).unwrap();
        target.clear(BLACK);
        assert!(renderer.draw(&mut target, "▒", (0, 0), WHITE));
        assert!(renderer.draw(&mut target, "▒", (12, 0), RgbColor { r: 0, g: 255, b: 0 }));
        assert_eq!(renderer.glyphs.len(), 1);
        let frame = target.into_frame();
        assert_eq!(&frame.pixels[0..4], &[128, 128, 128, 255]);
        assert_eq!(&frame.pixels[48..52], &[0, 128, 0, 255]);
        let mut target = PixelTarget::new(12, 24).unwrap();
        target.clear(BLACK);
        assert!(renderer.draw(&mut target, "⠀", (0, 0), WHITE));
        for text in ["A", "界", "─\u{301}", "\u{e0c0}", ""] {
            assert!(!renderer.draw(&mut target, text, (0, 0), WHITE));
        }
        assert!(target
            .into_frame()
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255]));
    }
}
