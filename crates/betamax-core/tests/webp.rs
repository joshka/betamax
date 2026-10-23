//! Decode actual WebP output to verify pixels, animation compositing, and stored timing.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use betamax_core::media::{
    write_webp, write_webp_animation, write_webp_animation_with_options,
    write_webp_animation_with_progress, Frame, MediaProgress, MediaProgressKind,
    MediaProgressReporter, PixelFormat, WebpCompression,
};
use image_webp::{LoopCount, WebPDecoder};

#[test]
fn static_webp_preserves_dimensions_colors_alpha_and_padded_bgra() {
    let output = Output::new("static");
    let mut frame = colorful_frame(0);
    let expected = frame.rgba().unwrap();
    frame.format = PixelFormat::Bgra8;
    frame.stride += 8;
    frame.pixels = expected
        .chunks_exact(frame.width as usize * 4)
        .flat_map(|row| {
            let mut padded: Vec<_> = row
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0], p[3]])
                .collect();
            padded.extend_from_slice(&[99; 8]);
            padded
        })
        .collect();
    write_webp(&output.0, &frame).unwrap();
    let mut decoder = output.decoder();
    assert_eq!(decoder.dimensions(), (33, 17));
    assert!(!decoder.is_animated());
    assert!(decoder.has_alpha());
    assert!(!decoder.is_lossy());
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.read_image(&mut pixels).unwrap();
    assert_eq!(pixels, expected); // More than 256 colors, partial and fully transparent pixels.
}

#[test]
fn animation_preserves_every_pixel_uneven_holds_and_final_dwell() {
    let output = Output::new("animation");
    let frames: Vec<_> = [137, 423, 1940]
        .into_iter()
        .enumerate()
        .map(|(index, ms)| (colorful_frame(index as u8), Duration::from_millis(ms)))
        .collect();
    let mut progress = Progress::default();
    write_webp_animation_with_progress(&output.0, &frames, &mut progress).unwrap();
    let mut decoder = output.decoder();
    assert!(decoder.is_animated());
    assert_eq!(decoder.dimensions(), (33, 17));
    assert_eq!(decoder.num_frames(), 3);
    assert_eq!(decoder.loop_count(), LoopCount::Forever);
    assert_eq!(decoder.loop_duration(), 2500);
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    for (frame, delay) in &frames {
        assert_eq!(
            decoder.read_frame(&mut pixels).unwrap(),
            delay.as_millis() as u32
        );
        // Changing alpha must replace the previous frame, not blend onto it.
        assert_eq!(pixels, frame.rgba().unwrap());
    }
    assert_eq!(progress.0.len(), 3);
    assert_eq!(
        progress.0.last().unwrap(),
        &MediaProgress {
            kind: MediaProgressKind::Webp,
            position: 3,
            total: 3,
        }
    );
}

#[test]
fn animation_rounds_cumulative_timing_without_drift() {
    let output = Output::new("rounding");
    let frames: Vec<_> = (0..100)
        .map(|index| (colorful_frame(index), Duration::from_micros(16_667)))
        .collect();
    write_webp_animation(&output.0, &frames).unwrap();
    let mut decoder = output.decoder();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let mut elapsed = 0_i64;
    for index in 1..=100 {
        elapsed += i64::from(decoder.read_frame(&mut pixels).unwrap());
        // The format stores integer milliseconds: each cumulative boundary is within 0.5 ms.
        assert!((elapsed * 1000 - index * 16_667).abs() <= 500);
    }
    assert_eq!(elapsed, 1667);
}

#[test]
fn single_frame_animation_retains_hold_and_zero_delay_has_minimum() {
    for (delay, expected) in [(0, 1), (2500, 2500), (0xff_ffff, 0xff_ffff)] {
        let output = Output::new("single");
        write_webp_animation(
            &output.0,
            &[(colorful_frame(0), Duration::from_millis(delay))],
        )
        .unwrap();
        let mut decoder = output.decoder();
        assert!(decoder.is_animated());
        assert_eq!(decoder.num_frames(), 1);
        let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
        assert_eq!(decoder.read_frame(&mut pixels).unwrap(), expected);
    }
    let output = Output::new("single-lossy");
    write_webp_animation_with_options(
        &output.0,
        &[(colorful_frame(0), Duration::from_millis(2500))],
        WebpCompression::Lossy { quality: 85 },
        &mut Progress::default(),
    )
    .unwrap();
    let decoder = output.decoder();
    assert!(decoder.is_animated());
    assert_eq!(decoder.loop_duration(), 2500);
}

#[test]
fn lossless_animation_stores_only_the_changed_rectangle() {
    let output = Output::new("subframes");
    let first = colorful_frame(0);
    let mut second = first.clone();
    // Change two pixels at odd coordinates: (5, 3) and (8, 9).
    for (x, y) in [(5, 3), (8, 9)] {
        let offset = (y * 33 + x) * 4;
        second.pixels[offset..offset + 4].copy_from_slice(&[1, 2, 3, 255]);
    }
    let frames = [
        (first.clone(), Duration::from_millis(100)),
        (second.clone(), Duration::from_millis(100)),
        (second.clone(), Duration::from_millis(100)),
    ];
    write_webp_animation(&output.0, &frames).unwrap();

    // Offsets round down to even coordinates; an unchanged frame stores one pixel.
    assert_eq!(
        frame_rects(&fs::read(&output.0).unwrap()),
        [(0, 0, 33, 17), (4, 2, 5, 8), (0, 0, 1, 1)]
    );
    let mut decoder = output.decoder();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    for (frame, _) in &frames {
        decoder.read_frame(&mut pixels).unwrap();
        assert_eq!(pixels, frame.rgba().unwrap());
    }
}

#[test]
fn lossy_animation_is_smaller_and_keeps_timing() {
    let noisy = |seed: u32| {
        let mut state = seed;
        let pixels = (0..128 * 96)
            .flat_map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let [r, g, b, _] = state.to_le_bytes();
                [r, g, b, 255]
            })
            .collect();
        Frame {
            width: 128,
            height: 96,
            stride: 128 * 4,
            format: PixelFormat::Rgba8,
            pixels,
        }
    };
    let frames: Vec<_> = [120, 35, 845]
        .into_iter()
        .zip(1..)
        .map(|(ms, seed)| (noisy(seed), Duration::from_millis(ms)))
        .collect();
    let lossless = Output::new("noisy-lossless");
    write_webp_animation(&lossless.0, &frames).unwrap();
    let lossy = Output::new("noisy-lossy");
    let mut progress = Progress::default();
    write_webp_animation_with_options(
        &lossy.0,
        &frames,
        WebpCompression::Lossy { quality: 85 },
        &mut progress,
    )
    .unwrap();
    assert_eq!(progress.0.len(), 3);

    let size = |output: &Output| fs::metadata(&output.0).unwrap().len();
    assert!(size(&lossy) * 2 < size(&lossless));
    let mut decoder = lossy.decoder();
    assert!(decoder.is_animated());
    assert!(decoder.is_lossy());
    assert_eq!(decoder.num_frames(), 3);
    assert_eq!(decoder.loop_duration(), 1000);
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    for (_, delay) in &frames {
        assert_eq!(
            decoder.read_frame(&mut pixels).unwrap(),
            delay.as_millis() as u32
        );
    }
    assert!(write_webp_animation_with_options(
        &lossy.0,
        &frames,
        WebpCompression::Lossy { quality: 101 },
        &mut progress,
    )
    .is_err());
}

#[test]
fn writers_reject_invalid_geometry_buffers_and_unrepresentable_holds() {
    let output = Output::new("invalid");
    assert!(write_webp_animation(&output.0, &[]).is_err());
    for (width, height, stride, pixels) in [
        (0, 1, 4, vec![0; 4]),
        (16385, 1, 4, vec![0; 4]),
        (1, 0, 4, vec![0; 4]),
        (1, 16385, 4, vec![0; 4]),
        (2, 1, 4, vec![0; 8]),
        (1, 1, 0, vec![]),
        (1, 2, 4, vec![0; 4]),
    ] {
        let frame = Frame {
            width,
            height,
            stride,
            pixels,
            format: PixelFormat::Rgba8,
        };
        assert!(write_webp(&output.0, &frame).is_err());
        assert!(write_webp_animation(&output.0, &[(frame, Duration::from_secs(1))]).is_err());
    }
    let frame = colorful_frame(0);
    let mut different = frame.clone();
    different.width -= 1;
    assert!(write_webp_animation(
        &output.0,
        &[
            (frame.clone(), Duration::from_secs(1)),
            (different, Duration::from_secs(1)),
        ]
    )
    .is_err());
    assert!(
        write_webp_animation(&output.0, &[(frame, Duration::from_millis(0x100_0000))]).is_err()
    );
}

/// Read each ANMF chunk's rectangle as (left, top, width, height).
fn frame_rects(webp: &[u8]) -> Vec<(u32, u32, u32, u32)> {
    let u24 = |bytes: &[u8]| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
    let mut rects = Vec::new();
    let mut offset = 12;
    while offset + 8 <= webp.len() {
        let size = u32::from_le_bytes(webp[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let payload = &webp[offset + 8..offset + 8 + size];
        if &webp[offset..offset + 4] == b"ANMF" {
            rects.push((
                u24(&payload[0..]) * 2,
                u24(&payload[3..]) * 2,
                u24(&payload[6..]) + 1,
                u24(&payload[9..]) + 1,
            ));
        }
        offset += 8 + size + size % 2;
    }
    rects
}

fn colorful_frame(step: u8) -> Frame {
    let pixels = (0..33 * 17)
        .flat_map(|index| {
            [
                index as u8,
                (index / 3) as u8,
                step,
                (index as u8).wrapping_add(step),
            ]
        })
        .collect();
    Frame {
        width: 33,
        height: 17,
        stride: 33 * 4,
        format: PixelFormat::Rgba8,
        pixels,
    }
}

#[derive(Default)]
struct Progress(Vec<MediaProgress>);
impl MediaProgressReporter for Progress {
    fn report(&mut self, progress: MediaProgress) {
        self.0.push(progress);
    }
}

struct Output(PathBuf);
impl Output {
    fn new(name: &str) -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "betamax-webp-{}-{suffix}-{name}/image.webp",
            std::process::id()
        )))
    }
    fn decoder(&self) -> WebPDecoder<Cursor<Vec<u8>>> {
        WebPDecoder::new(Cursor::new(fs::read(&self.0).unwrap())).unwrap()
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}
