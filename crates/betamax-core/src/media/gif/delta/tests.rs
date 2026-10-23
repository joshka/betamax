use super::*;

const T: u8 = GIF_TRANSPARENT_INDEX;
const MS: Duration = Duration::from_millis(100);

/// Encode `frames` (each `width` pixels wide) with the same delay and a reserved clear index.
fn encode(width: usize, frames: &[Vec<u8>]) -> Vec<DeltaFrame> {
    encode_with(width, frames, true, MS)
}

fn encode_with(
    width: usize,
    frames: &[Vec<u8>],
    transparent: bool,
    delay: Duration,
) -> Vec<DeltaFrame> {
    let mut delta = FrameDelta::new(width, frames[0].len() / width, transparent);
    let mut encoded: Vec<_> = frames
        .iter()
        .filter_map(|frame| delta.push(frame.clone(), delay))
        .collect();
    encoded.extend(delta.finish());
    encoded
}

/// Draw `frame` onto `canvas` as a GIF decoder does, returning the displayed pixels and leaving
/// `canvas` disposed for the next frame. `T` pixels are transparent.
fn draw(canvas: &mut [u8], width: usize, frame: &DeltaFrame) -> Vec<u8> {
    for (row, pixels) in frame
        .rect
        .rows(width)
        .zip(frame.buffer.chunks_exact(frame.rect.width))
    {
        for (shown, &pixel) in canvas[row].iter_mut().zip(pixels) {
            if pixel != T {
                *shown = pixel;
            }
        }
    }
    let displayed = canvas.to_vec();
    if frame.dispose == DisposalMethod::Background {
        for row in frame.rect.rows(width) {
            canvas[row].fill(T);
        }
    }
    displayed
}

/// Composite two loops of `encoded` without clearing in between, as some decoders loop.
fn composite_twice(width: usize, len: usize, encoded: &[DeltaFrame]) -> Vec<Vec<u8>> {
    let mut canvas = vec![T; len];
    encoded
        .iter()
        .chain(encoded)
        .map(|frame| draw(&mut canvas, width, frame))
        .collect()
}

#[test]
fn crops_later_frames_to_the_changed_pixels() {
    let first = vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let mut second = first.clone();
    second[5] = 2;
    second[7] = 3;
    let encoded = encode(4, &[first, second]);

    assert_eq!(
        encoded[0].rect,
        Rect {
            left: 0,
            top: 0,
            width: 4,
            height: 3
        }
    );
    assert_eq!(
        encoded[1].rect,
        Rect {
            left: 1,
            top: 1,
            width: 3,
            height: 1
        }
    );
    // The unchanged pixel between the two edits keeps the previous color.
    assert_eq!(encoded[1].buffer, vec![2, T, 3]);
    assert!(encoded
        .iter()
        .all(|frame| frame.dispose == DisposalMethod::Keep));
}

#[test]
fn merges_unchanged_frames_into_the_previous_delay() {
    let encoded = encode(2, &[vec![1, 2], vec![1, 2], vec![1, 2], vec![2, 2]]);

    assert_eq!(encoded.len(), 2);
    assert_eq!(encoded[0].delay, 30);
    assert_eq!(encoded[1].delay, 10);
}

#[test]
fn keeps_a_one_pixel_frame_when_a_merged_delay_would_overflow() {
    let mut delta = FrameDelta::new(2, 1, true);
    assert!(delta.push(vec![1, 2], Duration::from_secs(600)).is_none());
    let first = delta.push(vec![1, 2], Duration::from_secs(600)).unwrap();
    let last = delta.finish().unwrap();

    assert_eq!(first.delay, 60_000);
    assert_eq!(
        (last.rect.width, last.rect.height, last.buffer.as_slice()),
        (1, 1, [T].as_slice())
    );
}

#[test]
fn clears_pixels_that_become_transparent() {
    let frames = [vec![T, 1, 1, T], vec![T, 1, T, T], vec![T, 2, T, T]];
    let encoded = encode(4, &frames);

    assert_eq!(encoded[0].dispose, DisposalMethod::Background);
    assert_eq!(encoded[1].dispose, DisposalMethod::Keep);
    let displayed = composite_twice(4, 4, &encoded);
    assert_eq!(displayed[..3], frames);
}

#[test]
fn clears_again_before_looping_to_a_transparent_first_frame() {
    let frames = [vec![T, 1, T], vec![T, 1, 2]];
    let encoded = encode(3, &frames);

    assert_eq!(encoded[1].dispose, DisposalMethod::Background);
    let displayed = composite_twice(3, 3, &encoded);
    assert_eq!(displayed, [&frames[..], &frames[..]].concat());
}

#[test]
fn composited_frames_reproduce_every_input_frame() {
    // Deterministic pseudo-random edits of a small canvas, with clear pixels appearing and
    // disappearing, so cropping, keep, clear, merge and loop paths all run.
    let (width, height) = (7, 5);
    let mut state = 0x2545_f491_u32;
    let mut next = |bound: u32| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state % bound
    };
    for _ in 0..50 {
        let mut frame: Vec<u8> = (0..width * height).map(|_| next(4) as u8).collect();
        let mut frames = Vec::new();
        for _ in 0..12 {
            for _ in 0..next(4) {
                let pixel = next(frame.len() as u32) as usize;
                frame[pixel] = match next(5) {
                    0 => T,
                    value => value as u8,
                };
            }
            frames.push(frame.clone());
        }
        frames.dedup();
        let encoded = encode(width, &frames);

        assert_eq!(encoded.len(), frames.len());
        let displayed = composite_twice(width, width * height, &encoded);
        assert_eq!(displayed, [&frames[..], &frames[..]].concat());
    }
}

#[test]
fn short_frames_merge_into_later_ones_without_losing_time_or_pixels() {
    // 10 ms frames are too short for browsers, so most merge into the frame after them. Whatever
    // remains must be input frames in order, end on the last one, and keep about the total time.
    let (width, height) = (7, 5);
    let mut state = 0x9e37_79b9_u32;
    let mut next = |bound: u32| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state % bound
    };
    for transparent in [true, false] {
        for _ in 0..50 {
            let mut frame: Vec<u8> = (0..width * height).map(|_| 1 + next(4) as u8).collect();
            let mut frames = Vec::new();
            for _ in 0..12 {
                let pixel = next(frame.len() as u32) as usize;
                frame[pixel] = match next(5) {
                    0 if transparent => T,
                    value => 1 + value as u8,
                };
                frames.push(frame.clone());
            }
            frames.dedup();
            let encoded = encode_with(width, &frames, transparent, Duration::from_millis(10));

            let total: u32 = encoded.iter().map(|frame| u32::from(frame.delay)).sum();
            // Only the last frame and frames that clear pixels cannot merge, and each of those
            // runs at most one unit long.
            let clearing = frames.iter().filter(|frame| frame.contains(&T)).count() as u32;
            let nominal = frames.len() as u32;
            assert!(
                (nominal..=nominal + 1 + clearing).contains(&total),
                "{total} units for {nominal} frames, {clearing} clearing"
            );
            assert!(encoded.iter().all(|frame| frame.delay >= 2));
            let displayed = composite_twice(width, width * height, &encoded);
            let mut remaining = frames.iter();
            for shown in &displayed[..encoded.len()] {
                assert!(
                    remaining.any(|frame| frame == shown),
                    "out of order or unknown frame"
                );
            }
            assert_eq!(displayed[encoded.len() - 1], frames[frames.len() - 1]);
            assert_eq!(
                displayed[encoded.len()],
                displayed[0],
                "loop restarts on the first frame"
            );
        }
    }
}

#[test]
fn crops_without_a_clear_index_when_every_index_is_a_color() {
    // Index 255 is an opaque color here, so it must be drawn rather than treated as clear.
    let frames = [
        vec![T, T, 1, T],
        vec![T, 2, 1, T],
        vec![T, 2, 1, T],
        vec![T, 2, 1, 1],
    ];
    let encoded = encode_with(2, &frames, false, MS);

    assert_eq!(encoded.len(), 3);
    assert_eq!(encoded[0].buffer, frames[0]);
    assert_eq!(
        (
            encoded[1].rect,
            encoded[1].buffer.as_slice(),
            encoded[1].delay
        ),
        (
            Rect {
                left: 1,
                top: 0,
                width: 1,
                height: 1
            },
            [2].as_slice(),
            20
        )
    );
    assert_eq!(encoded[2].buffer, [1]);
    assert!(encoded
        .iter()
        .all(|frame| frame.dispose == DisposalMethod::Keep));
}
