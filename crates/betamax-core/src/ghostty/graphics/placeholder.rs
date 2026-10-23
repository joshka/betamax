//! Kitty unicode-placeholder cells.
//!
//! A program using virtual placements (`U=1`) draws an image by printing U+10EEEE in each cell the
//! image should cover. The foreground color carries the low 24 bits of the image id, the underline
//! color carries an optional placement id, and up to three combining diacritics carry the image
//! row, image column, and the high 8 bits of the image id. Missing diacritics continue the run to
//! their left, so a program may only mark the first cell of each row.
//!
//! See <https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders>.

use libghostty_vt::style::StyleColor;

/// Codepoint printed in every cell covered by a virtual placement.
pub(crate) const PLACEHOLDER: char = '\u{10EEEE}';

/// Kitty's row/column diacritics; a diacritic's index in this table is the value it encodes.
///
/// From <https://sw.kovidgoyal.net/kitty/_downloads/1792bad15b12979994cd6ecc54c967a6/rowcolumn-diacritics.txt>.
const DIACRITICS: [u32; 297] = [
    0x00305, 0x0030D, 0x0030E, 0x00310, 0x00312, 0x0033D, 0x0033E, 0x0033F, 0x00346, 0x0034A,
    0x0034B, 0x0034C, 0x00350, 0x00351, 0x00352, 0x00357, 0x0035B, 0x00363, 0x00364, 0x00365,
    0x00366, 0x00367, 0x00368, 0x00369, 0x0036A, 0x0036B, 0x0036C, 0x0036D, 0x0036E, 0x0036F,
    0x00483, 0x00484, 0x00485, 0x00486, 0x00487, 0x00592, 0x00593, 0x00594, 0x00595, 0x00597,
    0x00598, 0x00599, 0x0059C, 0x0059D, 0x0059E, 0x0059F, 0x005A0, 0x005A1, 0x005A8, 0x005A9,
    0x005AB, 0x005AC, 0x005AF, 0x005C4, 0x00610, 0x00611, 0x00612, 0x00613, 0x00614, 0x00615,
    0x00616, 0x00617, 0x00657, 0x00658, 0x00659, 0x0065A, 0x0065B, 0x0065D, 0x0065E, 0x006D6,
    0x006D7, 0x006D8, 0x006D9, 0x006DA, 0x006DB, 0x006DC, 0x006DF, 0x006E0, 0x006E1, 0x006E2,
    0x006E4, 0x006E7, 0x006E8, 0x006EB, 0x006EC, 0x00730, 0x00732, 0x00733, 0x00735, 0x00736,
    0x0073A, 0x0073D, 0x0073F, 0x00740, 0x00741, 0x00743, 0x00745, 0x00747, 0x00749, 0x0074A,
    0x007EB, 0x007EC, 0x007ED, 0x007EE, 0x007EF, 0x007F0, 0x007F1, 0x007F3, 0x00816, 0x00817,
    0x00818, 0x00819, 0x0081B, 0x0081C, 0x0081D, 0x0081E, 0x0081F, 0x00820, 0x00821, 0x00822,
    0x00823, 0x00825, 0x00826, 0x00827, 0x00829, 0x0082A, 0x0082B, 0x0082C, 0x0082D, 0x00951,
    0x00953, 0x00954, 0x00F82, 0x00F83, 0x00F86, 0x00F87, 0x0135D, 0x0135E, 0x0135F, 0x017DD,
    0x0193A, 0x01A17, 0x01A75, 0x01A76, 0x01A77, 0x01A78, 0x01A79, 0x01A7A, 0x01A7B, 0x01A7C,
    0x01B6B, 0x01B6D, 0x01B6E, 0x01B6F, 0x01B70, 0x01B71, 0x01B72, 0x01B73, 0x01CD0, 0x01CD1,
    0x01CD2, 0x01CDA, 0x01CDB, 0x01CE0, 0x01DC0, 0x01DC1, 0x01DC3, 0x01DC4, 0x01DC5, 0x01DC6,
    0x01DC7, 0x01DC8, 0x01DC9, 0x01DCB, 0x01DCC, 0x01DD1, 0x01DD2, 0x01DD3, 0x01DD4, 0x01DD5,
    0x01DD6, 0x01DD7, 0x01DD8, 0x01DD9, 0x01DDA, 0x01DDB, 0x01DDC, 0x01DDD, 0x01DDE, 0x01DDF,
    0x01DE0, 0x01DE1, 0x01DE2, 0x01DE3, 0x01DE4, 0x01DE5, 0x01DE6, 0x01DFE, 0x020D0, 0x020D1,
    0x020D4, 0x020D5, 0x020D6, 0x020D7, 0x020DB, 0x020DC, 0x020E1, 0x020E7, 0x020E9, 0x020F0,
    0x02CEF, 0x02CF0, 0x02CF1, 0x02DE0, 0x02DE1, 0x02DE2, 0x02DE3, 0x02DE4, 0x02DE5, 0x02DE6,
    0x02DE7, 0x02DE8, 0x02DE9, 0x02DEA, 0x02DEB, 0x02DEC, 0x02DED, 0x02DEE, 0x02DEF, 0x02DF0,
    0x02DF1, 0x02DF2, 0x02DF3, 0x02DF4, 0x02DF5, 0x02DF6, 0x02DF7, 0x02DF8, 0x02DF9, 0x02DFA,
    0x02DFB, 0x02DFC, 0x02DFD, 0x02DFE, 0x02DFF, 0x0A66F, 0x0A67C, 0x0A67D, 0x0A6F0, 0x0A6F1,
    0x0A8E0, 0x0A8E1, 0x0A8E2, 0x0A8E3, 0x0A8E4, 0x0A8E5, 0x0A8E6, 0x0A8E7, 0x0A8E8, 0x0A8E9,
    0x0A8EA, 0x0A8EB, 0x0A8EC, 0x0A8ED, 0x0A8EE, 0x0A8EF, 0x0A8F0, 0x0A8F1, 0x0AAB0, 0x0AAB2,
    0x0AAB3, 0x0AAB7, 0x0AAB8, 0x0AABE, 0x0AABF, 0x0AAC1, 0x0FE20, 0x0FE21, 0x0FE22, 0x0FE23,
    0x0FE24, 0x0FE25, 0x0FE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186, 0x1D187, 0x1D188, 0x1D189,
    0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243, 0x1D244,
];

/// Return the value a row/column diacritic encodes.
fn diacritic_index(ch: char) -> Option<u32> {
    DIACRITICS
        .binary_search(&u32::from(ch))
        .ok()
        .and_then(|index| u32::try_from(index).ok())
}

/// Return the diacritic that encodes `index`, if the table is long enough.
pub(crate) fn diacritic(index: u32) -> Option<char> {
    DIACRITICS
        .get(usize::try_from(index).ok()?)
        .and_then(|&code| char::from_u32(code))
}

/// One placeholder cell with its run inheritance resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaceholderCell {
    /// Viewport column of the cell.
    pub(crate) x: u16,
    /// Viewport row of the cell.
    pub(crate) y: u16,
    /// Full image id, including the high byte from the third diacritic.
    pub(crate) image_id: u32,
    /// Placement id from the underline color, or zero for "any virtual placement".
    pub(crate) placement_id: u32,
    /// Image row this cell shows, in cells.
    pub(crate) row: u32,
    /// Image column this cell shows, in cells.
    pub(crate) column: u32,
}

/// Placeholder fields read directly from one cell, before run inheritance.
#[derive(Debug, Clone, Copy)]
struct RawPlaceholder {
    x: u16,
    id_low: u32,
    placement_id: u32,
    row: Option<u32>,
    column: Option<u32>,
    id_high: Option<u32>,
}

/// Collects the placeholder cells of one row and resolves inherited diacritics.
///
/// Cells must be pushed left to right. Call [`PlaceholderRow::finish`] at the end of each row.
#[derive(Debug, Default)]
pub(crate) struct PlaceholderRow {
    previous: Option<PlaceholderCell>,
    cells: Vec<PlaceholderCell>,
}

impl PlaceholderRow {
    /// Record a cell if its graphemes start with the placeholder codepoint.
    ///
    /// Returns `true` when the cell is a placeholder, so the caller can skip drawing its glyph.
    pub(crate) fn push(
        &mut self,
        x: u16,
        y: u16,
        graphemes: &[char],
        foreground: StyleColor,
        underline: StyleColor,
    ) -> bool {
        let Some(raw) = RawPlaceholder::parse(x, graphemes, foreground, underline) else {
            self.previous = None;
            return false;
        };
        let cell = match self.previous {
            Some(previous) if raw.continues(&previous) => PlaceholderCell {
                x,
                y,
                image_id: raw.image_id(previous.image_id >> 24),
                placement_id: raw.placement_id,
                row: raw.row.unwrap_or(previous.row),
                column: raw.column.unwrap_or(previous.column + 1),
            },
            _ => PlaceholderCell {
                x,
                y,
                image_id: raw.image_id(0),
                placement_id: raw.placement_id,
                row: raw.row.unwrap_or(0),
                column: raw.column.unwrap_or(0),
            },
        };
        self.previous = Some(cell);
        self.cells.push(cell);
        true
    }

    /// End the current row, moving its cells into `out`.
    pub(crate) fn finish(&mut self, out: &mut Vec<PlaceholderCell>) {
        self.previous = None;
        out.append(&mut self.cells);
    }
}

impl RawPlaceholder {
    fn parse(
        x: u16,
        graphemes: &[char],
        foreground: StyleColor,
        underline: StyleColor,
    ) -> Option<Self> {
        let (&first, marks) = graphemes.split_first()?;
        if first != PLACEHOLDER {
            return None;
        }
        // Kitty ignores invalid diacritics rather than rejecting the cell.
        let mut marks = marks.iter().map(|&mark| diacritic_index(mark));
        Some(Self {
            x,
            id_low: color_id(foreground),
            placement_id: color_id(underline),
            row: marks.next().flatten(),
            column: marks.next().flatten(),
            id_high: marks.next().flatten().filter(|high| *high <= 0xff),
        })
    }

    /// Whether this cell extends the run ending at `previous`, following kitty's rules.
    fn continues(&self, previous: &PlaceholderCell) -> bool {
        previous.x.checked_add(1) == Some(self.x)
            && previous.image_id & 0x00ff_ffff == self.id_low
            && previous.placement_id == self.placement_id
            && self.row.is_none_or(|row| row == previous.row)
            && self
                .column
                .is_none_or(|column| column == previous.column + 1)
            && self
                .id_high
                .is_none_or(|high| high == previous.image_id >> 24)
    }

    fn image_id(&self, inherited_high: u32) -> u32 {
        (self.id_high.unwrap_or(inherited_high) << 24) | self.id_low
    }
}

/// Decode an id from a placeholder color: 24-bit RGB, or a 256-color palette index.
fn color_id(color: StyleColor) -> u32 {
    match color {
        StyleColor::Rgb(rgb) => {
            (u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b)
        }
        StyleColor::Palette(index) => u32::from(index.0),
        StyleColor::None => 0,
    }
}

#[cfg(test)]
mod tests {
    use libghostty_vt::style::{PaletteIndex, RgbColor};

    use super::*;

    fn rgb(id: u32) -> StyleColor {
        StyleColor::Rgb(RgbColor {
            r: (id >> 16) as u8,
            g: (id >> 8) as u8,
            b: id as u8,
        })
    }

    fn cell(marks: &[u32]) -> Vec<char> {
        std::iter::once(PLACEHOLDER)
            .chain(marks.iter().map(|&mark| diacritic(mark).unwrap()))
            .collect()
    }

    #[test]
    fn diacritic_table_round_trips() {
        assert_eq!(DIACRITICS.len(), 297);
        assert!(DIACRITICS.is_sorted());
        for index in [0, 1, 42, 255, 296] {
            assert_eq!(diacritic_index(diacritic(index).unwrap()), Some(index));
        }
        assert_eq!(diacritic(297), None);
    }

    #[test]
    fn explicit_diacritics_set_row_column_and_high_byte() {
        let mut row = PlaceholderRow::default();
        assert!(row.push(3, 7, &cell(&[2, 5, 0x12]), rgb(0x00_3456), StyleColor::None));
        let mut cells = Vec::new();
        row.finish(&mut cells);
        assert_eq!(
            cells,
            [PlaceholderCell {
                x: 3,
                y: 7,
                image_id: 0x1200_3456,
                placement_id: 0,
                row: 2,
                column: 5,
            }]
        );
    }

    #[test]
    fn bare_placeholders_continue_the_run_to_their_left() {
        let mut row = PlaceholderRow::default();
        row.push(0, 0, &cell(&[4, 0, 1]), rgb(9), rgb(3));
        row.push(1, 0, &cell(&[]), rgb(9), rgb(3));
        row.push(2, 0, &cell(&[4]), rgb(9), rgb(3));
        let mut cells = Vec::new();
        row.finish(&mut cells);
        let resolved: Vec<_> = cells
            .iter()
            .map(|cell| (cell.image_id, cell.placement_id, cell.row, cell.column))
            .collect();
        assert_eq!(
            resolved,
            [
                (0x0100_0009, 3, 4, 0),
                (0x0100_0009, 3, 4, 1),
                (0x0100_0009, 3, 4, 2)
            ]
        );
    }

    #[test]
    fn gaps_and_id_changes_start_a_new_run() {
        let mut row = PlaceholderRow::default();
        row.push(0, 0, &cell(&[1, 3]), rgb(9), StyleColor::None);
        assert!(!row.push(1, 0, &['x'], rgb(9), StyleColor::None));
        row.push(2, 0, &cell(&[]), rgb(9), StyleColor::None);
        row.push(
            3,
            0,
            &cell(&[]),
            StyleColor::Palette(PaletteIndex(7)),
            StyleColor::None,
        );
        let mut cells = Vec::new();
        row.finish(&mut cells);
        let resolved: Vec<_> = cells
            .iter()
            .map(|cell| (cell.x, cell.image_id, cell.row, cell.column))
            .collect();
        assert_eq!(resolved, [(0, 9, 1, 3), (2, 9, 0, 0), (3, 7, 0, 0)]);
    }

    #[test]
    fn ordinary_text_is_not_a_placeholder() {
        let mut row = PlaceholderRow::default();
        assert!(!row.push(0, 0, &['a'], rgb(1), StyleColor::None));
        assert!(!row.push(1, 0, &[], rgb(1), StyleColor::None));
    }
}
