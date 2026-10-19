use super::*;
use crate::media::PixelFormat;

#[test]
fn caption_changes_keep_repeated_frames_separate() {
    let frame = test_frame();
    let mut capture = CaptureState {
        caption: Some("First step".to_string()),
        ..Default::default()
    };

    append_visible_frame(&mut capture, frame.clone(), Duration::from_millis(20));
    capture.caption = Some("Second step".to_string());
    append_visible_frame(&mut capture, frame.clone(), Duration::from_millis(20));

    assert_eq!(capture.frames.len(), 2);
    assert_eq!(capture.frames[0].0.caption.as_deref(), Some("First step"));
    assert_eq!(capture.frames[1].0.caption.as_deref(), Some("Second step"));
}

#[test]
fn title_assignments_preserve_bursts_and_only_extend_short_holds() {
    let source = "Set WindowBar Colorful\nSet WindowTitle Initial\nSet Framerate 30";
    let tape = crate::tape::Tape::parse(&format!("Set Preset Default-0.1\n{source}")).unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let mut capture = CaptureState::default();
    for title in ["A", "B", "A", "A"] {
        assign_title(&mut capture, &settings, title.into(), Some(test_frame()));
    }
    assert_eq!(capture.frames.len(), 3);
    append_visible_frame(&mut capture, test_frame(), Duration::from_secs(1));
    protect_title_holds(&mut capture, &settings);
    assert!(capture.frames[0].1 >= Duration::from_secs_f64(1.0 / 30.0));
    assert_eq!(capture.frames[2].1, Duration::from_secs(1));
    assert_eq!(
        capture
            .frames
            .iter()
            .map(|(frame, _)| frame.window_title.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["A", "B", "A"]
    );
    capture.visible = false;
    assign_title(&mut capture, &settings, "hidden".into(), None);
    assert_eq!(capture.frames.len(), 3);
    assert_eq!(capture.window_title.as_deref(), Some("hidden"));
}

fn test_frame() -> Frame {
    Frame {
        width: 1,
        height: 1,
        stride: 4,
        format: PixelFormat::Rgba8,
        pixels: vec![255, 0, 0, 255],
    }
}

fn protected_burst(rate: u16, speed: f64) -> (Vec<(Frame, Duration)>, f64) {
    let source = format!("Set WindowBar Colorful\nSet Framerate {rate}\nSet PlaybackSpeed {speed}");
    let tape = crate::tape::Tape::parse(&format!("Set Preset Default-0.1\n{source}")).unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let mut capture = CaptureState::default();
    for (title, pixel) in [
        ("A", [255, 0, 0, 255]),
        ("B", [0, 255, 0, 255]),
        ("A", [255, 0, 0, 255]),
    ] {
        let frame = Frame {
            width: 16,
            height: 16,
            stride: 64,
            format: PixelFormat::Rgba8,
            pixels: pixel.repeat(16 * 16),
        };
        assign_title(&mut capture, &settings, title.into(), Some(frame));
    }
    protect_title_holds(&mut capture, &settings);
    let frames = capture
        .frames
        .into_iter()
        .map(|(frame, delay)| (frame.frame, delay))
        .collect();
    (frames, settings.output_framerate())
}

#[test]
fn title_transitions_survive_gif_webp_and_png_sequences() {
    use crate::media::{write_gif, write_png_sequence, write_webp_animation};
    let root = std::env::temp_dir().join(format!("betamax-title-formats-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let (frames, _) = protected_burst(30, 8.0);
    let gif = root.join("burst.gif");
    write_gif(&gif, &frames).unwrap();
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(std::fs::File::open(gif).unwrap())
        .unwrap();
    for (expected, _) in &frames {
        let actual = decoder.read_next_frame().unwrap().unwrap();
        assert_eq!(actual.buffer.as_ref(), expected.pixels);
        assert!(actual.delay >= 1);
    }
    assert!(decoder.read_next_frame().unwrap().is_none());
    let webp = root.join("burst.webp");
    write_webp_animation(&webp, &frames).unwrap();
    let mut decoder =
        image_webp::WebPDecoder::new(std::io::BufReader::new(std::fs::File::open(webp).unwrap()))
            .unwrap();
    assert_eq!(decoder.num_frames(), 3);
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    for (expected, _) in &frames {
        assert!(decoder.read_frame(&mut pixels).unwrap() >= 10);
        assert_eq!(pixels, expected.pixels);
    }
    let pngs = root.join("frames");
    write_png_sequence(&pngs, &frames).unwrap();
    assert_eq!(std::fs::read_dir(pngs).unwrap().count(), 3);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires ffmpeg with H.264/VP9 encoders"]
fn title_transitions_survive_video_quantization() {
    use crate::media::{write_mp4, write_webm};
    let root = std::env::temp_dir().join(format!("betamax-title-video-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for (rate, speed) in [(30, 1.0), (25, 0.5), (30, 8.0), (50, 0.01)] {
        let (frames, fps) = protected_burst(rate, speed);
        for ext in ["mp4", "webm"] {
            let path = root.join(format!("burst-{rate}-{speed}.{ext}"));
            if ext == "mp4" {
                write_mp4(&path, &frames, fps).unwrap();
            } else {
                write_webm(&path, &frames, fps).unwrap();
            }
            let output = std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&path)
                .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let mut titles = Vec::new();
            for pixels in output.stdout.as_chunks::<{ 16 * 16 * 3 }>().0 {
                let title = usize::from(pixels[1] > pixels[0]);
                if titles.last() != Some(&title) {
                    titles.push(title);
                }
            }
            assert_eq!(titles, [0, 1, 0], "{rate} x {speed}, {ext}");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
