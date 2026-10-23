use std::sync::atomic::{AtomicUsize, Ordering};

use ::image::Rgba;

use super::*;
use crate::runner::canvas::rgb;
use crate::runner::settings::Settings;
use crate::runner::Runner;
use crate::tape::Tape;

const RED: Rgba<u8> = Rgba([255, 0, 0, 255]);
const BLUE: Rgba<u8> = Rgba([0, 0, 255, 255]);
const CLEAR: Rgba<u8> = Rgba([0, 0, 0, 0]);
const THEME: libghostty_vt::style::RgbColor = rgb(1, 2, 3);

/// A fresh scratch directory per test, so parallel tests never share files.
fn scratch_dir() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "betamax-margin-image-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write a PNG built from rows of pixels and return its directory and file name.
fn write_png(rows: &[&[Rgba<u8>]]) -> (PathBuf, &'static str) {
    let dir = scratch_dir();
    let image = RgbaImage::from_fn(rows[0].len() as u32, rows.len() as u32, |x, y| {
        rows[y as usize][x as usize]
    });
    image.save(dir.join("fill.png")).unwrap();
    (dir, "fill.png")
}

fn backdrop(fill: &str, width: u32, height: u32, dir: &Path) -> Canvas {
    MarginImage::parse(fill)
        .unwrap()
        .unwrap()
        .backdrop(width, height, Some(dir), THEME)
        .unwrap()
}

fn pixel(canvas: &Canvas, x: u32, y: u32) -> [u8; 4] {
    let frame = canvas.clone().into_frame();
    let offset = ((y * frame.width + x) * 4) as usize;
    frame.pixels[offset..offset + 4].try_into().unwrap()
}

#[test]
fn bare_values_are_vhs_paths_and_image_takes_an_optional_fit() {
    for reserved in [
        "",
        " ",
        "#123456",
        "#zzz",
        "linear(#000000, #ffffff)",
        "Transparent",
    ] {
        assert_eq!(MarginImage::parse(reserved).unwrap(), None, "{reserved}");
    }
    assert_eq!(
        MarginImage::parse(" bg.png ").unwrap(),
        Some(MarginImage {
            path: PathBuf::from("bg.png"),
            fit: ImageFit::Stretch
        })
    );
    assert_eq!(
        MarginImage::parse("image( art/bg.jpg )")
            .unwrap()
            .unwrap()
            .fit,
        ImageFit::Cover
    );
    for (fit, expected) in [
        ("cover", ImageFit::Cover),
        ("Contain", ImageFit::Contain),
        ("TILE", ImageFit::Tile),
        ("stretch", ImageFit::Stretch),
    ] {
        let image = MarginImage::parse(&format!("image(bg.png, {fit})"))
            .unwrap()
            .unwrap();
        assert_eq!(image.fit, expected, "{fit}");
    }
}

#[test]
fn malformed_image_fills_are_errors() {
    for value in [
        "image()",
        "image(, cover)",
        "image(bg.png",
        "image(bg.png, fill)",
        "image(bg.png, cover, tile)",
    ] {
        let error = MarginImage::parse(value).unwrap_err().to_string();
        assert!(error.contains("expected image(<path>)"), "{value}: {error}");
    }
}

#[test]
fn cover_crops_overflow_equally_from_both_sides() {
    // A 4x2 image covering a 2x2 output keeps its scale and loses one column per side.
    let (dir, file) = write_png(&[&[BLUE, RED, BLUE, BLUE], &[BLUE, RED, BLUE, BLUE]]);
    let canvas = backdrop(&format!("image({file})"), 2, 2, &dir);
    assert_eq!(pixel(&canvas, 0, 0), RED.0);
    assert_eq!(pixel(&canvas, 1, 1), BLUE.0);
    assert!(!canvas.has_alpha());
}

#[test]
fn contain_centers_the_image_over_the_theme_background() {
    let (dir, file) = write_png(&[&[RED, RED]]);
    let canvas = backdrop(&format!("image({file}, contain)"), 4, 4, &dir);
    for x in 0..4 {
        assert_eq!(pixel(&canvas, x, 0), [1, 2, 3, 255]);
        assert_eq!(pixel(&canvas, x, 1), RED.0);
        assert_eq!(pixel(&canvas, x, 2), RED.0);
        assert_eq!(pixel(&canvas, x, 3), [1, 2, 3, 255]);
    }
}

#[test]
fn tile_repeats_at_native_size_from_the_top_left() {
    let (dir, file) = write_png(&[&[RED, BLUE]]);
    let canvas = backdrop(&format!("image({file}, tile)"), 3, 2, &dir);
    for y in 0..2 {
        assert_eq!(pixel(&canvas, 0, y), RED.0);
        assert_eq!(pixel(&canvas, 1, y), BLUE.0);
        assert_eq!(pixel(&canvas, 2, y), RED.0);
    }
}

#[test]
fn bare_paths_stretch_to_the_output_like_vhs() {
    let (dir, file) = write_png(&[&[RED]]);
    let canvas = backdrop(file, 5, 3, &dir);
    for (x, y) in [(0, 0), (4, 2), (2, 1)] {
        assert_eq!(pixel(&canvas, x, y), RED.0);
    }
}

#[test]
fn image_alpha_is_kept_and_clear_pixels_store_zero_color() {
    let (dir, file) = write_png(&[&[Rgba([9, 9, 9, 0]), Rgba([255, 0, 0, 128])]]);
    let canvas = backdrop(&format!("image({file}, tile)"), 2, 1, &dir);
    assert_eq!(pixel(&canvas, 0, 0), CLEAR.0);
    assert_eq!(pixel(&canvas, 1, 0), [255, 0, 0, 128]);
    assert!(canvas.has_alpha());
}

#[test]
fn premultiplied_resizing_keeps_edge_color_next_to_clear_pixels() {
    let (dir, file) = write_png(&[&[RED, CLEAR]]);
    let canvas = backdrop(&format!("image({file}, stretch)"), 8, 1, &dir);
    for x in 0..8 {
        let [r, g, b, a] = pixel(&canvas, x, 0);
        if a > 0 {
            assert_eq!((r, g, b), (255, 0, 0), "x={x} alpha={a}");
        }
    }
}

#[test]
fn missing_images_name_every_place_searched() {
    let dir = scratch_dir();
    let error = MarginImage::parse("nope.png")
        .unwrap()
        .unwrap()
        .backdrop(2, 2, Some(&dir), THEME)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("MarginFill image not found: nope.png"),
        "{error}"
    );
    assert!(
        error.contains(&dir.join("nope.png").display().to_string()),
        "{error}"
    );
}

#[test]
fn undecodable_images_are_errors() {
    let dir = scratch_dir();
    std::fs::write(dir.join("bad.png"), b"not an image").unwrap();
    let error = MarginImage::parse("bad.png")
        .unwrap()
        .unwrap()
        .backdrop(2, 2, Some(&dir), THEME)
        .unwrap_err()
        .to_string();
    assert!(error.contains("failed to read MarginFill image"), "{error}");
    assert!(error.contains("bad.png"), "{error}");
}

#[test]
fn tape_settings_resolve_images_beside_the_tape() {
    let (dir, file) = write_png(&[&[BLUE]]);
    let tape = Tape::parse(&format!(
        "Set Width 40\nSet Height 40\nSet Margin 4\nSet MarginFill \"image({file})\""
    ))
    .unwrap();
    let settings = Settings::from_tape_in(&tape, Some(&dir)).unwrap();
    assert!(!settings.presentation.margin_has_alpha());
    assert!(Settings::from_tape(&tape).is_err());
}

#[test]
fn translucent_images_reject_mp4_before_spawning_the_shell() {
    let (dir, file) = write_png(&[&[Rgba([0, 0, 255, 100])]]);
    let tape = Tape::parse(&format!(
        "Output target/betamax-image.mp4\nSet MarginFill \"{file}\"\nSleep 1ms"
    ))
    .unwrap();
    let error = Runner::new()
        .with_tape_dir(dir)
        .run_artifacts(&tape)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("MP4 cannot carry MarginFill transparency"),
        "{error}"
    );
}
