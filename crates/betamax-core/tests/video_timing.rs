//! Encoder integration tests. Run with `mise run video-test` (requires ffmpeg and ffprobe).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use betamax_core::media::{write_gif, write_mp4, write_webm, Frame, PixelFormat};
use serde_json::Value;

#[test]
#[ignore = "requires ffmpeg with H.264/VP9 encoders and ffprobe"]
fn videos_preserve_uneven_holds_and_final_frame() {
    let directory = TestDirectory::new();
    let frames = vec![
        solid_frame([255, 0, 0, 255], 137),
        solid_frame([0, 255, 0, 255], 423),
        solid_frame([0, 0, 255, 255], 1940),
    ];
    let gif = directory.0.join("reference.gif");
    write_gif(&gif, &frames).unwrap();
    let gif_duration = duration(&gif);
    assert!((gif_duration - 2.5).abs() < 0.03);

    for rate in [30.0, 12.5] {
        for extension in ["mp4", "webm"] {
            let path = directory.0.join(format!("uneven-{rate}.{extension}"));
            encode(&path, &frames, rate);
            let pixels = decode(&path);
            let count = pixels.len() / (16 * 16 * 3);
            assert_eq!(count, (2.5_f64 * rate).round() as usize);
            assert!((duration(&path) - gif_duration).abs() <= 1.0 / rate + 0.03);

            // Check every decoded frame, including both transitions and the final dwell.
            // Cumulative rounding must not give every source image just one output tick.
            let first_end = (0.137 * rate).round() as usize;
            let second_end = (0.560 * rate).round() as usize;
            for (index, frame) in pixels.as_chunks::<{ 16 * 16 * 3 }>().0.iter().enumerate() {
                let channel = if index < first_end {
                    0
                } else if index < second_end {
                    1
                } else {
                    2
                };
                assert!(
                    frame[channel] > 220,
                    "{extension} at {rate} FPS, frame {index}"
                );
                assert!(frame[(channel + 1) % 3] < 30);
                assert!(frame[(channel + 2) % 3] < 30);
            }
            let probe = probe(&path);
            let stream = &probe["streams"][0];
            assert_eq!(stream["pix_fmt"], "yuv420p");
            assert_eq!(
                stream["codec_name"],
                if extension == "mp4" { "h264" } else { "vp9" }
            );
        }
    }
}

#[test]
#[ignore = "requires ffmpeg with H.264/VP9 encoders and ffprobe"]
fn videos_keep_single_frame_holds_and_subframe_clips() {
    let directory = TestDirectory::new();
    for milliseconds in [0, 1, 2500] {
        for extension in ["mp4", "webm"] {
            let path = directory
                .0
                .join(format!("single-{milliseconds}.{extension}"));
            let frames = [solid_frame([0, 0, 255, 255], milliseconds)];
            encode(&path, &frames, 30.0);
            let expected = (milliseconds as f64 * 0.03).round().max(1.0) as usize;
            let pixels = decode(&path);
            assert_eq!(pixels.len(), expected * 16 * 16 * 3);
            assert!((duration(&path) - expected as f64 / 30.0).abs() < 0.002);
            assert!(pixels.as_chunks::<3>().0.iter().all(|pixel| pixel[2] > 220));
        }
    }
}

#[test]
#[ignore = "requires ffmpeg with H.264/VP9 encoders and ffprobe"]
fn video_rounding_does_not_accumulate_per_frame() {
    let directory = TestDirectory::new();
    let frames: Vec<_> = (0..100)
        .map(|_| solid_frame([255, 0, 0, 255], 17))
        .collect();
    for extension in ["mp4", "webm"] {
        let path = directory.0.join(format!("rounding.{extension}"));
        encode(&path, &frames, 30.0);
        assert_eq!(decode(&path).len(), 51 * 16 * 16 * 3);
        assert!((duration(&path) - 1.7).abs() < 0.002);
    }
}

#[test]
#[ignore = "requires ffmpeg with H.264/VP9 encoders and ffprobe"]
fn videos_preserve_motion_and_uneven_frame_holds() {
    let directory = TestDirectory::new();
    let holds = [300, 200, 300, 100, 400, 200, 500, 1500];
    let frames: Vec<_> = holds
        .iter()
        .enumerate()
        .map(|(step, &hold)| {
            let (mut frame, delay) = solid_frame([0, 0, 0, 255], hold);
            // A white square moves two pixels right on each source frame.
            for y in 6..10 {
                for x in step * 2..step * 2 + 2 {
                    let offset = (y * 16 + x) * 4;
                    frame.pixels[offset..offset + 4].copy_from_slice(&[255; 4]);
                }
            }
            (frame, delay)
        })
        .collect();
    for extension in ["mp4", "webm"] {
        let path = directory.0.join(format!("motion.{extension}"));
        encode(&path, &frames, 30.0);
        let pixels = decode(&path);
        let decoded = pixels.as_chunks::<{ 16 * 16 * 3 }>().0;
        assert_eq!(decoded.len(), 105);
        let mut start = 0;
        let mut elapsed = 0;
        for (step, hold) in holds.iter().enumerate() {
            elapsed += hold;
            let end = (elapsed as f64 * 0.03).round() as usize;
            for frame in &decoded[start..end] {
                for x in 0..16 {
                    let brightness = frame[(7 * 16 + x) * 3];
                    if (step * 2..step * 2 + 2).contains(&x) {
                        assert!(brightness > 200, "missing square at step {step}, x={x}");
                    } else {
                        assert!(brightness < 40, "stale square at step {step}, x={x}");
                    }
                }
            }
            start = end;
        }
        assert!((duration(&path) - 3.5).abs() < 0.002);
    }
}

fn solid_frame(color: [u8; 4], milliseconds: u64) -> (Frame, Duration) {
    (
        Frame {
            width: 16,
            height: 16,
            stride: 16 * 4,
            format: PixelFormat::Rgba8,
            pixels: color.repeat(16 * 16),
        },
        Duration::from_millis(milliseconds),
    )
}

fn encode(path: &Path, frames: &[(Frame, Duration)], rate: f64) {
    if path.extension().unwrap() == "mp4" {
        write_mp4(path, frames, rate).unwrap();
    } else {
        write_webm(path, frames, rate).unwrap();
    }
}

fn probe(path: &Path) -> Value {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .expect("ffprobe must be installed for video timing tests");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn duration(path: &Path) -> f64 {
    probe(path)["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

fn decode(path: &Path) -> Vec<u8> {
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-fps_mode",
            "passthrough",
            "pipe:1",
        ])
        .output()
        .expect("ffmpeg must be installed for video timing tests");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "betamax-video-{}-{}-{suffix}",
            std::process::id(),
            TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
