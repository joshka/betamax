//! Compositing of terminal inline images into rendered frames.
//!
//! libghostty-vt parses kitty graphics commands and owns image storage, placement state, scrolling,
//! and deletion. This module reads that state at capture time and draws it onto the software
//! render target: direct placements at their resolved viewport position and size, and virtual
//! placements wherever the grid holds unicode placeholder cells (see [`placeholder`]).
//!
//! Sixel and iTerm2 images reach this module as kitty virtual placements; see
//! `inline_image`.

pub(super) mod placeholder;

use std::collections::HashMap;
use std::rc::Rc;

use image::imageops::{self, FilterType};
use image::RgbaImage;
use libghostty_vt::alloc::{Allocator, Bytes};
use libghostty_vt::kitty::graphics::{
    self, DecodePng, DecodedImage, Image, ImageFormat, Layer, PlacementIterator,
};
use libghostty_vt::Terminal;

use self::placeholder::PlaceholderCell;
use super::target::PixelTarget;
use crate::Result;

/// Kitty image storage limit per screen.
///
/// Ghostty's own default is 320 MB. Recordings are short-lived, so the same budget is generous
/// without letting a runaway program exhaust memory.
pub(super) const STORAGE_LIMIT_BYTES: u64 = 320 * 1024 * 1024;

/// Install the PNG decoder libghostty-vt uses for kitty `f=100` transmissions.
///
/// The decoder is thread-local in libghostty-vt, so this must run on the thread that writes VT
/// bytes into the terminal.
pub(super) fn install_png_decoder() -> Result<()> {
    graphics::set_png_decoder(Some(Box::new(ImagePngDecoder)))
        .map_err(|error| miette::miette!("failed to install kitty PNG decoder: {error}").into())
}

/// Decodes PNG payloads with the `image` crate into RGBA8.
struct ImagePngDecoder;

impl DecodePng for ImagePngDecoder {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>> {
        let decoded = image::load_from_memory_with_format(data, image::ImageFormat::Png)
            .inspect_err(|error| tracing::debug!(%error, "kitty PNG decode failed"))
            .ok()?
            .to_rgba8();
        let mut bytes = Bytes::new_with_alloc(alloc, decoded.as_raw().len()).ok()?;
        bytes.copy_from_slice(decoded.as_raw());
        Some(DecodedImage {
            width: decoded.width(),
            height: decoded.height(),
            data: bytes,
        })
    }
}

/// Where the terminal grid sits on the render target, in pixels.
#[derive(Debug, Clone, Copy)]
pub(super) struct GridGeometry {
    /// Left and top padding before the first cell.
    pub(super) padding: u32,
    /// Cell width.
    pub(super) cell_width: u32,
    /// Cell height.
    pub(super) cell_height: u32,
    /// Visible grid columns.
    pub(super) columns: u32,
    /// Visible grid rows.
    pub(super) rows: u32,
}

impl GridGeometry {
    /// Pixel rectangle covered by the grid, used to clip images that extend past the viewport.
    fn clip(&self) -> ClipRect {
        let left = self.padding as i64;
        let top = self.padding as i64;
        ClipRect {
            left,
            top,
            right: left + i64::from(self.columns) * i64::from(self.cell_width),
            bottom: top + i64::from(self.rows) * i64::from(self.cell_height),
        }
    }
}

/// Inclusive-exclusive clip rectangle in target pixels.
#[derive(Debug, Clone, Copy)]
struct ClipRect {
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
}

/// Direct placements split by the layer they draw on.
#[derive(Default)]
pub(super) struct PlacementLayers {
    /// Placements with negative z: drawn after cell backgrounds, before text.
    below_text: Vec<DirectPlacement>,
    /// Placements with z >= 0: drawn over text.
    above_text: Vec<DirectPlacement>,
    /// Virtual placements keyed by image id and placement id, for placeholder cells.
    virtual_grids: HashMap<(u32, u32), VirtualGrid>,
    /// First virtual placement per image, for placeholders without a placement id.
    virtual_by_image: HashMap<u32, VirtualGrid>,
    /// Scaled images ready to blend, keyed like the cache.
    images: HashMap<ScaleKey, Rc<RgbaImage>>,
}

/// A direct placement resolved to target pixels.
struct DirectPlacement {
    x: i64,
    y: i64,
    key: ScaleKey,
}

/// Grid size of a virtual placement plus the image it shows.
#[derive(Debug, Clone, Copy)]
struct VirtualGrid {
    image_id: u32,
    generation: u64,
    image_width: u32,
    image_height: u32,
    columns: u32,
    rows: u32,
}

/// Identifies one scaled rendering of an image source rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ScaleKey {
    image_id: u32,
    generation: u64,
    source: (u32, u32, u32, u32),
    width: u32,
    height: u32,
}

/// Scales kitty images for compositing and caches the results across frames.
///
/// Images are immutable for a given generation, so a scaled copy stays valid until the program
/// retransmits the image or the placement size changes. Entries unused in a frame are dropped.
#[derive(Default)]
pub(super) struct ImageCompositor {
    cache: HashMap<ScaleKey, Rc<RgbaImage>>,
}

impl ImageCompositor {
    /// Resolve every placement in the terminal's active screen against the current viewport.
    ///
    /// Placeholder cells collected while iterating rows are resolved here too, so all image data
    /// is read while the terminal is borrowed and drawing can proceed afterwards.
    pub(super) fn prepare(
        &mut self,
        terminal: &Terminal<'static, 'static>,
        geometry: GridGeometry,
        placeholders: &[PlaceholderCell],
    ) -> Result<PlacementLayers> {
        let mut layers = PlacementLayers::default();
        let storage = terminal
            .kitty_graphics()
            .map_err(vt_error("failed to read kitty graphics storage"))?;
        if storage
            .generation()
            .map_err(vt_error("failed to read kitty graphics generation"))?
            == 0
        {
            self.cache.clear();
            return Ok(layers);
        }

        let mut iterator =
            PlacementIterator::new().map_err(vt_error("failed to create placement iterator"))?;
        let mut placements = iterator
            .update(&storage)
            .map_err(vt_error("failed to iterate kitty placements"))?;
        placements
            .set_layer(Layer::All)
            .map_err(vt_error("failed to set kitty placement layer"))?;
        while let Some(placement) = placements.next() {
            let image_id = placement
                .image_id()
                .map_err(vt_error("failed to read placement image id"))?;
            let Some(image) = storage.image(image_id) else {
                continue;
            };
            let generation = image
                .generation()
                .map_err(vt_error("failed to read kitty image generation"))?;
            if placement
                .is_virtual()
                .map_err(vt_error("failed to read placement kind"))?
            {
                let grid = virtual_grid(
                    image_id,
                    generation,
                    &image,
                    placement.columns().unwrap_or(0),
                    placement.rows().unwrap_or(0),
                    geometry,
                )?;
                let placement_id = placement
                    .placement_id()
                    .map_err(vt_error("failed to read placement id"))?;
                layers.virtual_grids.insert((image_id, placement_id), grid);
                layers.virtual_by_image.entry(image_id).or_insert(grid);
                continue;
            }

            let info = placement
                .placement_render_info(&image, terminal)
                .map_err(vt_error("failed to read kitty placement geometry"))?;
            if !info.viewport_visible || info.pixel_width == 0 || info.pixel_height == 0 {
                continue;
            }
            let x_offset = placement.x_offset().unwrap_or(0);
            let y_offset = placement.y_offset().unwrap_or(0);
            let key = ScaleKey {
                image_id,
                generation,
                source: (
                    info.source_x,
                    info.source_y,
                    info.source_width,
                    info.source_height,
                ),
                width: info.pixel_width,
                height: info.pixel_height,
            };
            self.ensure_scaled(&mut layers, key, &image)?;
            let direct = DirectPlacement {
                x: i64::from(geometry.padding)
                    + i64::from(info.viewport_col) * i64::from(geometry.cell_width)
                    + i64::from(x_offset),
                y: i64::from(geometry.padding)
                    + i64::from(info.viewport_row) * i64::from(geometry.cell_height)
                    + i64::from(y_offset),
                key,
            };
            let z = placement.z().unwrap_or(0);
            if z < 0 {
                layers.below_text.push(direct);
            } else {
                layers.above_text.push(direct);
            }
        }

        // Placeholder cells show a slice of the image fitted into the virtual placement's grid.
        for cell in placeholders {
            let Some(grid) = layers.lookup_virtual(cell) else {
                continue;
            };
            let key = grid.fitted_key(geometry);
            if !layers.images.contains_key(&key) {
                if let Some(image) = storage.image(grid.image_id) {
                    self.ensure_scaled(&mut layers, key, &image)?;
                }
            }
        }

        // Keep only what this frame uses; scrolled-away images release their scaled copies.
        self.cache.retain(|key, _| layers.images.contains_key(key));
        Ok(layers)
    }

    /// Insert the scaled image for `key` into this frame's set, reusing the cache when possible.
    fn ensure_scaled(
        &mut self,
        layers: &mut PlacementLayers,
        key: ScaleKey,
        image: &Image<'_>,
    ) -> Result<()> {
        if layers.images.contains_key(&key) {
            return Ok(());
        }
        let scaled = match self.cache.get(&key) {
            Some(scaled) => Rc::clone(scaled),
            None => {
                let Some(rgba) = image_rgba(image)? else {
                    return Ok(());
                };
                let (x, y, width, height) = key.source;
                let source = imageops::crop_imm(&rgba, x, y, width, height).to_image();
                let scaled = if (source.width(), source.height()) == (key.width, key.height) {
                    source
                } else {
                    imageops::resize(&source, key.width, key.height, FilterType::Triangle)
                };
                let scaled = Rc::new(scaled);
                self.cache.insert(key, Rc::clone(&scaled));
                scaled
            }
        };
        layers.images.insert(key, scaled);
        Ok(())
    }
}

impl PlacementLayers {
    /// Draw placements that sit between cell backgrounds and text.
    pub(super) fn draw_below_text(&self, target: &mut PixelTarget, geometry: GridGeometry) {
        self.draw_direct(&self.below_text, target, geometry);
    }

    /// Draw placements above text, then placeholder cells.
    pub(super) fn draw_above_text(
        &self,
        target: &mut PixelTarget,
        geometry: GridGeometry,
        placeholders: &[PlaceholderCell],
    ) {
        self.draw_direct(&self.above_text, target, geometry);
        let clip = geometry.clip();
        for cell in placeholders {
            let Some(grid) = self.lookup_virtual(cell) else {
                continue;
            };
            let key = grid.fitted_key(geometry);
            let Some(image) = self.images.get(&key) else {
                continue;
            };
            // The fitted image is centred in the placement's grid; this cell shows one cell-sized
            // window of that grid.
            let cell_width = i64::from(geometry.cell_width);
            let cell_height = i64::from(geometry.cell_height);
            let grid_x = i64::from(cell.column) * cell_width;
            let grid_y = i64::from(cell.row) * cell_height;
            let (offset_x, offset_y) = grid.fitted_offset(geometry);
            let cell_left = i64::from(geometry.padding) + i64::from(cell.x) * cell_width;
            let cell_top = i64::from(geometry.padding) + i64::from(cell.y) * cell_height;
            let cell_clip = ClipRect {
                left: cell_left.max(clip.left),
                top: cell_top.max(clip.top),
                right: (cell_left + cell_width).min(clip.right),
                bottom: (cell_top + cell_height).min(clip.bottom),
            };
            blend_image(
                target,
                image,
                cell_left - grid_x + offset_x,
                cell_top - grid_y + offset_y,
                cell_clip,
            );
        }
    }

    fn draw_direct(
        &self,
        placements: &[DirectPlacement],
        target: &mut PixelTarget,
        geometry: GridGeometry,
    ) {
        let clip = geometry.clip();
        for placement in placements {
            if let Some(image) = self.images.get(&placement.key) {
                blend_image(target, image, placement.x, placement.y, clip);
            }
        }
    }

    fn lookup_virtual(&self, cell: &PlaceholderCell) -> Option<VirtualGrid> {
        if cell.placement_id == 0 {
            self.virtual_by_image.get(&cell.image_id).copied()
        } else {
            self.virtual_grids
                .get(&(cell.image_id, cell.placement_id))
                .copied()
        }
    }
}

impl VirtualGrid {
    /// Size of the image scaled to fit the grid with its aspect ratio preserved.
    fn fitted_size(&self, geometry: GridGeometry) -> (u32, u32) {
        let grid_width = f64::from(self.columns) * f64::from(geometry.cell_width);
        let grid_height = f64::from(self.rows) * f64::from(geometry.cell_height);
        let image_width = f64::from(self.image_width.max(1));
        let image_height = f64::from(self.image_height.max(1));
        let scale = (grid_width / image_width).min(grid_height / image_height);
        (
            ((image_width * scale).round() as u32).max(1),
            ((image_height * scale).round() as u32).max(1),
        )
    }

    /// Offset that centres the fitted image inside the grid.
    fn fitted_offset(&self, geometry: GridGeometry) -> (i64, i64) {
        let (width, height) = self.fitted_size(geometry);
        let grid_width = i64::from(self.columns) * i64::from(geometry.cell_width);
        let grid_height = i64::from(self.rows) * i64::from(geometry.cell_height);
        (
            (grid_width - i64::from(width)) / 2,
            (grid_height - i64::from(height)) / 2,
        )
    }

    fn fitted_key(&self, geometry: GridGeometry) -> ScaleKey {
        let (width, height) = self.fitted_size(geometry);
        ScaleKey {
            image_id: self.image_id,
            generation: self.generation,
            source: (0, 0, self.image_width, self.image_height),
            width,
            height,
        }
    }
}

/// Resolve the grid of a virtual placement, defaulting unspecified sides to the image's own size.
fn virtual_grid(
    image_id: u32,
    generation: u64,
    image: &Image<'_>,
    columns: u32,
    rows: u32,
    geometry: GridGeometry,
) -> Result<VirtualGrid> {
    let image_width = image
        .width()
        .map_err(vt_error("failed to read kitty image width"))?;
    let image_height = image
        .height()
        .map_err(vt_error("failed to read kitty image height"))?;
    Ok(VirtualGrid {
        image_id,
        generation,
        image_width,
        image_height,
        columns: if columns == 0 {
            image_width.div_ceil(geometry.cell_width.max(1)).max(1)
        } else {
            columns
        },
        rows: if rows == 0 {
            image_height.div_ceil(geometry.cell_height.max(1)).max(1)
        } else {
            rows
        },
    })
}

/// Convert stored kitty image data to RGBA8.
///
/// libghostty-vt decodes PNG and inflates zlib payloads when an image is stored, so stored data is
/// normally raw pixels. Anything else is logged and skipped rather than failing the capture.
fn image_rgba(image: &Image<'_>) -> Result<Option<RgbaImage>> {
    let width = image
        .width()
        .map_err(vt_error("failed to read kitty image width"))?;
    let height = image
        .height()
        .map_err(vt_error("failed to read kitty image height"))?;
    let format = image
        .format()
        .map_err(vt_error("failed to read kitty image format"))?;
    let data = image
        .data()
        .map_err(vt_error("failed to read kitty image data"))?;
    let pixels = width as usize * height as usize;
    let rgba: Option<Vec<u8>> = match format {
        ImageFormat::Rgba => (data.len() >= pixels * 4).then(|| data[..pixels * 4].to_vec()),
        ImageFormat::Rgb => (data.len() >= pixels * 3).then(|| {
            data[..pixels * 3]
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&[r, g, b]| [r, g, b, 0xff])
                .collect()
        }),
        ImageFormat::GrayAlpha => (data.len() >= pixels * 2).then(|| {
            data[..pixels * 2]
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[gray, alpha]| [gray, gray, gray, alpha])
                .collect()
        }),
        ImageFormat::Gray => (data.len() >= pixels).then(|| {
            data[..pixels]
                .iter()
                .flat_map(|&gray| [gray, gray, gray, 0xff])
                .collect()
        }),
        ImageFormat::Png => {
            return Ok(
                image::load_from_memory_with_format(data, image::ImageFormat::Png)
                    .inspect_err(|error| tracing::debug!(%error, "stored kitty PNG decode failed"))
                    .ok()
                    .map(|decoded| decoded.to_rgba8()),
            );
        }
        _ => None,
    };
    let Some(rgba) = rgba else {
        tracing::debug!(
            ?format,
            width,
            height,
            bytes = data.len(),
            "skipping kitty image"
        );
        return Ok(None);
    };
    Ok(RgbaImage::from_raw(width, height, rgba))
}

/// Alpha-blend `image` with its top-left corner at `(x, y)`, limited to `clip`.
fn blend_image(target: &mut PixelTarget, image: &RgbaImage, x: i64, y: i64, clip: ClipRect) {
    let left = x.max(clip.left).max(0);
    let top = y.max(clip.top).max(0);
    let right = (x + i64::from(image.width()))
        .min(clip.right)
        .min(i64::from(target.width()));
    let bottom = (y + i64::from(image.height()))
        .min(clip.bottom)
        .min(i64::from(target.height()));
    for target_y in top..bottom {
        for target_x in left..right {
            let pixel = image.get_pixel((target_x - x) as u32, (target_y - y) as u32);
            target.blend_rgba(target_x as u32, target_y as u32, pixel.0);
        }
    }
}

fn vt_error(context: &'static str) -> impl Fn(libghostty_vt::Error) -> miette::Report {
    move |error| miette::miette!("{context}: {error}")
}

#[cfg(test)]
mod tests;
