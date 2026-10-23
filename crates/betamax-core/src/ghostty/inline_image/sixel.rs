//! Sixel image decoding.
//!
//! Decodes the data part of a sixel DCS (`ESC P <params> q <data> ESC \`) into RGBA pixels. Unset
//! pixels stay transparent whatever the background-select parameter says: a terminal would fill
//! them with its background, which the composited frame already shows underneath.

use image::RgbaImage;

/// Largest width or height accepted, matching common terminal sixel limits.
const MAX_DIMENSION: usize = 10_000;
/// Number of palette registers.
const PALETTE_SIZE: usize = 1024;

/// VT340 default colors for the first 16 registers, as percentages.
const VT340_PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],
    [20, 20, 80],
    [80, 13, 13],
    [20, 80, 20],
    [80, 20, 80],
    [20, 80, 80],
    [80, 80, 20],
    [53, 53, 53],
    [26, 26, 26],
    [33, 33, 60],
    [60, 26, 26],
    [33, 60, 33],
    [60, 33, 60],
    [33, 60, 60],
    [60, 60, 33],
    [80, 80, 80],
];

/// Decode sixel data (the bytes after the `q` introducer, without the terminator).
///
/// Returns `None` when the data draws no pixels.
pub(super) fn decode(data: &[u8]) -> Option<RgbaImage> {
    let mut decoder = Decoder::new();
    decoder.run(data);
    decoder.finish()
}

struct Decoder {
    palette: Vec<[u8; 4]>,
    color: usize,
    x: usize,
    y: usize,
    /// Size declared by raster attributes, if any.
    declared: (usize, usize),
    /// Extent of drawn pixels.
    drawn: (usize, usize),
    width: usize,
    pixels: Vec<[u8; 4]>,
}

impl Decoder {
    fn new() -> Self {
        let mut palette = vec![[0, 0, 0, 255]; PALETTE_SIZE];
        for (slot, [r, g, b]) in palette.iter_mut().zip(VT340_PALETTE) {
            *slot = [percent(r), percent(g), percent(b), 255];
        }
        Self {
            palette,
            color: 0,
            x: 0,
            y: 0,
            declared: (0, 0),
            drawn: (0, 0),
            width: 0,
            pixels: Vec::new(),
        }
    }

    fn run(&mut self, data: &[u8]) {
        let mut index = 0;
        while index < data.len() {
            let byte = data[index];
            index += 1;
            match byte {
                b'"' => {
                    let (params, next) = parse_params(data, index);
                    index = next;
                    if let [_, _, width, height, ..] = params[..] {
                        self.declared = (
                            (width as usize).min(MAX_DIMENSION),
                            (height as usize).min(MAX_DIMENSION),
                        );
                        self.reserve(self.declared.0, self.declared.1);
                    }
                }
                b'#' => {
                    let (params, next) = parse_params(data, index);
                    index = next;
                    self.select_or_define(&params);
                }
                b'!' => {
                    let (params, next) = parse_params(data, index);
                    index = next;
                    let count = params.first().copied().unwrap_or(1).max(1) as usize;
                    if let Some(&sixel) = data.get(index) {
                        if (0x3f..=0x7e).contains(&sixel) {
                            index += 1;
                            self.draw(sixel - 0x3f, count);
                        }
                    }
                }
                b'$' => self.x = 0,
                b'-' => {
                    self.x = 0;
                    self.y += 6;
                }
                0x3f..=0x7e => self.draw(byte - 0x3f, 1),
                _ => {}
            }
        }
    }

    fn select_or_define(&mut self, params: &[u32]) {
        let Some(&register) = params.first() else {
            return;
        };
        let register = register as usize % PALETTE_SIZE;
        self.color = register;
        if let [_, space, a, b, c, ..] = *params {
            let rgb = match space {
                1 => hls_to_rgb(a, b, c),
                2 => [
                    percent(a.min(100) as u8),
                    percent(b.min(100) as u8),
                    percent(c.min(100) as u8),
                ],
                _ => return,
            };
            self.palette[register] = [rgb[0], rgb[1], rgb[2], 255];
        }
    }

    fn draw(&mut self, bits: u8, count: usize) {
        let end = (self.x + count).min(MAX_DIMENSION);
        if bits != 0 && self.y < MAX_DIMENSION {
            let bottom = (self.y + 6).min(MAX_DIMENSION);
            self.reserve(end, bottom);
            let color = self.palette[self.color];
            for row in 0..6 {
                if bits & (1 << row) == 0 || self.y + row >= MAX_DIMENSION {
                    continue;
                }
                let offset = (self.y + row) * self.width;
                self.pixels[offset + self.x..offset + end].fill(color);
                self.drawn.1 = self.drawn.1.max(self.y + row + 1);
            }
            self.drawn.0 = self.drawn.0.max(end);
        }
        self.x = end;
    }

    /// Grow the pixel buffer to at least `width` x `height`, keeping drawn pixels in place.
    fn reserve(&mut self, width: usize, height: usize) {
        let rows = self.pixels.len().checked_div(self.width).unwrap_or(0);
        if width <= self.width && height <= rows {
            return;
        }
        let new_width = width.max(self.width);
        let new_rows = height.max(rows);
        let mut pixels = vec![[0u8; 4]; new_width * new_rows];
        for row in 0..rows {
            pixels[row * new_width..row * new_width + self.width]
                .copy_from_slice(&self.pixels[row * self.width..(row + 1) * self.width]);
        }
        self.width = new_width;
        self.pixels = pixels;
    }

    fn finish(self) -> Option<RgbaImage> {
        let width = self.declared.0.max(self.drawn.0);
        let height = self.declared.1.max(self.drawn.1);
        if self.drawn == (0, 0) || width == 0 || height == 0 {
            return None;
        }
        let mut image = RgbaImage::new(width as u32, height as u32);
        for (y, row) in self
            .pixels
            .chunks_exact(self.width.max(1))
            .take(height)
            .enumerate()
        {
            for (x, pixel) in row.iter().take(width).enumerate() {
                image.put_pixel(x as u32, y as u32, image::Rgba(*pixel));
            }
        }
        Some(image)
    }
}

/// Parse `;`-separated decimal parameters starting at `index`; empty parameters are zero.
fn parse_params(data: &[u8], mut index: usize) -> (Vec<u32>, usize) {
    let mut params = vec![0u32];
    while let Some(&byte) = data.get(index) {
        match byte {
            b'0'..=b'9' => {
                let last = params.last_mut().expect("params starts non-empty");
                *last = last
                    .saturating_mul(10)
                    .saturating_add(u32::from(byte - b'0'));
            }
            b';' => params.push(0),
            _ => break,
        }
        index += 1;
    }
    (params, index)
}

fn percent(value: u8) -> u8 {
    ((u16::from(value) * 255 + 50) / 100) as u8
}

/// Convert DEC HLS (hue 0 = blue, lightness and saturation in percent) to RGB.
fn hls_to_rgb(hue: u32, lightness: u32, saturation: u32) -> [u8; 3] {
    let hue = f64::from((hue % 360 + 240) % 360);
    let lightness = f64::from(lightness.min(100)) / 100.0;
    let saturation = f64::from(saturation.min(100)) / 100.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let base = lightness - chroma / 2.0;
    let channel = |value: f64| ((value + base) * 255.0).round().clamp(0.0, 255.0) as u8;
    [channel(r), channel(g), channel(b)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_bands_colors_and_repeats() {
        // Register 1 is pure red; "~" sets all six bits; "!3~" repeats it three times. "-" moves to
        // the next band where "@" sets only the top bit.
        let image = decode(b"\"1;1;4;7#1;2;100;0;0#1!3~~-@").unwrap();
        assert_eq!(image.dimensions(), (4, 7));
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(3, 5).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(0, 6).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(1, 6).0, [0, 0, 0, 0]);
    }

    #[test]
    fn carriage_return_overlays_colors_in_one_band() {
        let image = decode(b"#1;2;0;0;100#2;2;0;100;0#1A$#2@").unwrap();
        // "A" sets bit 1 (second row) in blue; "$" returns; "@" sets bit 0 in green.
        assert_eq!(image.get_pixel(0, 0).0, [0, 255, 0, 255]);
        assert_eq!(image.get_pixel(0, 1).0, [0, 0, 255, 255]);
    }

    #[test]
    fn hls_uses_dec_hue_origin() {
        assert_eq!(hls_to_rgb(0, 50, 100), [0, 0, 255]);
        assert_eq!(hls_to_rgb(120, 50, 100), [255, 0, 0]);
        assert_eq!(hls_to_rgb(240, 50, 100), [0, 255, 0]);
    }

    #[test]
    fn empty_data_decodes_to_nothing() {
        assert!(decode(b"#0;2;0;0;0").is_none());
    }

    #[test]
    fn raster_size_grows_when_drawing_exceeds_it() {
        let image = decode(b"\"1;1;1;1#1;2;100;100;100!5~").unwrap();
        assert_eq!(image.dimensions(), (5, 6));
    }
}
