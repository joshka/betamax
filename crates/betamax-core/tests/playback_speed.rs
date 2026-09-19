//! Exercise real PTY capture and decode animation holds at faster and slower playback speeds.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use betamax_core::{RunOptions, Runner, Tape};
use image_webp::WebPDecoder;

#[test]
fn captured_holds_scale_with_playback_speed() {
    let directory = TestDirectory::new();
    for speed in [0.5, 1.0, 2.0] {
        let stem = directory.0.join(format!("speed-{speed}").replace('.', "_"));
        capture(&stem, speed, &["gif", "webp"]);
        for (extension, holds) in [
            ("gif", gif_holds(&stem.with_extension("gif"))),
            ("webp", webp_holds(&stem.with_extension("webp"))),
        ] {
            // Show contributes one capture tick. Allow capture/render scheduling overhead on CI.
            let red = holds
                .iter()
                .take_while(|(channel, _)| *channel == 0)
                .map(|(_, delay)| delay)
                .sum::<f64>();
            let total = holds.iter().map(|(_, delay)| delay).sum::<f64>();
            assert!(
                (red * speed - 1.05).abs() < 0.3,
                "{extension} speed {speed}: red {red}, {holds:?}"
            );
            assert!(
                (total * speed - 3.05).abs() < 0.5,
                "{extension} speed {speed}: total {total}, {holds:?}"
            );
            assert!(holds.iter().any(|(channel, _)| *channel == 2));
            assert_eq!(holds.last().unwrap().0, 2);
        }
    }
}

#[test]
#[ignore = "requires ffmpeg with H.264/VP9 encoders and ffprobe"]
fn captured_videos_scale_transitions_and_final_holds() {
    let directory = TestDirectory::new();
    for speed in [0.5, 1.0, 2.0] {
        let stem = directory.0.join(format!("speed-{speed}").replace('.', "_"));
        capture(&stem, speed, &["mp4", "webm"]);
        for extension in ["mp4", "webm"] {
            let path = stem.with_extension(extension);
            let probe = std::process::Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-show_entries",
                    "format=duration",
                    "-of",
                    "json",
                ])
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                probe.status.success(),
                "{}",
                String::from_utf8_lossy(&probe.stderr)
            );
            let metadata: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
            let duration: f64 = metadata["format"]["duration"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                (duration * speed - 3.05).abs() < 0.5,
                "{path:?}: duration {duration}"
            );
            // Sample the center background throughout playback, including the last blue hold.
            let decoded = std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(&path)
                .args([
                    "-vf",
                    "crop=2:2:240:120,fps=40,format=rgb24",
                    "-f",
                    "rawvideo",
                    "-",
                ])
                .output()
                .unwrap();
            assert!(
                decoded.status.success(),
                "{}",
                String::from_utf8_lossy(&decoded.stderr)
            );
            let channels: Vec<_> = decoded
                .stdout
                .as_chunks::<12>()
                .0
                .iter()
                .map(|pixel| channel(pixel))
                .collect();
            let red_frames = channels.iter().take_while(|&&value| value == 0).count();
            let transition = red_frames as f64 / 40.0;
            assert!(
                (transition * speed - 1.05).abs() < 0.3,
                "{path:?}: transition {transition}"
            );
            assert!(channels[red_frames..].iter().all(|&value| value == 2));
            assert_eq!(channels.last(), Some(&2));
            assert!((channels.len() as f64 / 40.0 - duration).abs() < 0.06);
        }
    }
}

fn capture(stem: &Path, speed: f64, extensions: &[&str]) {
    let outputs = extensions
        .iter()
        .map(|extension| format!("Output {}.{extension}\n", stem.display()))
        .collect::<String>();
    let tape = format!(
        r#"
{outputs}
Set Shell "bash"
Set Width 480
Set Height 240
Set FontSize 20
Set Framerate 20
Set PlaybackSpeed {speed}
Set TypingSpeed 0ms
Set CursorBlink false
Hide
Type "printf '\033[?25l\033[41m\033[2J\033[HRED_READY\n'"
Enter
Wait+Screen /^RED_READY/
Show
Sleep 1s
Hide
Type "printf '\033[44m\033[2J\033[HBLUE_READY\n'; sleep 2; printf 'DONE\n'"
Enter
Wait+Screen /^BLUE_READY/
Show
Wait+Screen /DONE/
Hide
Type "exit"
Enter
"#,
    );
    let mut runner = Runner::new(RunOptions {
        publish: false,
        quiet: true,
    });
    runner.run(&Tape::parse(&tape).unwrap()).unwrap();
}

fn channel(pixel: &[u8]) -> usize {
    if i16::from(pixel[0]) > i16::from(pixel[2]) + 50 {
        0
    } else {
        assert!(
            i16::from(pixel[2]) > i16::from(pixel[0]) + 50,
            "unexpected background {pixel:?}"
        );
        2
    }
}

fn gif_holds(path: &Path) -> Vec<(usize, f64)> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(fs::File::open(path).unwrap()).unwrap();
    let mut holds = Vec::new();
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        let offset = (120 * usize::from(frame.width) + 240) * 4;
        holds.push((
            channel(&frame.buffer[offset..offset + 3]),
            f64::from(frame.delay) / 100.0,
        ));
    }
    holds
}

fn webp_holds(path: &Path) -> Vec<(usize, f64)> {
    let mut decoder = WebPDecoder::new(Cursor::new(fs::read(path).unwrap())).unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let channels = if decoder.has_alpha() { 4 } else { 3 };
    let offset = (120 * 480 + 240) * channels;
    let mut holds = Vec::new();
    for _ in 0..decoder.num_frames() {
        let delay = decoder.read_frame(&mut pixels).unwrap();
        holds.push((
            channel(&pixels[offset..offset + 3]),
            f64::from(delay) / 1000.0,
        ));
    }
    holds
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("betamax-speed-{}-{suffix}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
