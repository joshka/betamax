use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use betamax_core::ghostty::{
    CaptureRequest, PixelSize, StateCursor, StateSpan, StateStyle, TerminalGrid, TerminalState,
};
use betamax_core::media::{Frame, PixelFormat};
use betamax_core::tape::{Command, Key, KeyCode, KeyModifiers, Value, WaitPattern, WaitTarget};
use betamax_core::{
    Error, FrameCapture, Result, RunArtifacts, RunOptions, Runner, Tape, TerminalSession,
};

#[test]
fn public_api_types_are_send_where_expected() {
    assert_send::<CaptureRequest>();
    assert_send::<Command>();
    assert_send::<Error>();
    assert_send::<Frame>();
    assert_send::<Key>();
    assert_send::<KeyCode>();
    assert_send::<KeyModifiers>();
    assert_send::<PixelFormat>();
    assert_send::<PixelSize>();
    assert_send::<RunArtifacts>();
    assert_send::<RunOptions>();
    assert_send::<Runner>();
    assert_send::<StateCursor>();
    assert_send::<StateSpan>();
    assert_send::<StateStyle>();
    assert_send::<Tape>();
    assert_send::<TerminalState>();
    assert_send::<TerminalGrid>();
    assert_send::<Value>();
    assert_send::<WaitPattern>();
    assert_send::<WaitTarget>();
}

#[test]
fn public_runner_returns_final_state_and_output_paths() {
    let output = temp_output("betamax-public-api-state.json");
    let tape = Tape::parse(&format!(
        r#"
        Output {}
        Set Shell "bash"
        Copy "printf 'api ok\n'"
        Paste
        Enter
        Wait+Screen "api ok"
        Hide
        Type "exit"
        Enter
        "#,
        output.display()
    ))
    .unwrap();

    let artifacts = Runner::new(RunOptions {
        publish: false,
        quiet: true,
    })
    .run_artifacts(&tape)
    .unwrap();

    let state = artifacts.final_state.expect("final state");
    assert!(state.viewport_text.contains("api ok"));
    assert_eq!(artifacts.output_paths, vec![output.clone()]);
    assert!(output.is_file());
    let _ = fs::remove_file(output);
}

#[test]
fn runner_accepts_custom_capture_backend() {
    let output = temp_output("betamax-public-api-fake-capture.json");
    let tape = Tape::parse(&format!(
        r#"
        Output {}
        Set Shell "bash"
        Type "printf 'fake capture ok\n'"
        Enter
        Wait+Screen "fake capture ok"
        Hide
        Type "exit"
        Enter
        "#,
        output.display()
    ))
    .unwrap();

    let artifacts = Runner::with_capture(
        RunOptions {
            publish: false,
            quiet: true,
        },
        FakeCapture,
    )
    .run_artifacts(&tape)
    .unwrap();

    let state = artifacts.final_state.expect("final state");
    assert!(state.viewport_text.contains("fake capture ok"));
    assert_eq!(artifacts.output_paths, vec![output.clone()]);
    assert!(output.is_file());
    let _ = fs::remove_file(output);
}

#[test]
fn captions_affect_screenshots_not_state_json() {
    let final_state = temp_output("betamax-public-api-caption-final.json");
    let checkpoint_state = temp_output("betamax-public-api-caption-checkpoint.json");
    let plain_screenshot = temp_output("betamax-public-api-caption-plain.png");
    let captioned_screenshot = temp_output("betamax-public-api-caption-captioned.png");
    let caption = "Review-only caption";
    let tape = Tape::parse(&format!(
        r#"
        Output {final_state}
        Set Shell "bash"
        Set Width 180
        Set Height 120
        Set Padding 0
        Screenshot {plain_screenshot}
        Caption "{caption}"
        Screenshot {captioned_screenshot}
        Type "printf 'terminal only\n'"
        Enter
        Wait+Screen "terminal only"
        State {checkpoint_state}
        "#,
        final_state = final_state.display(),
        plain_screenshot = plain_screenshot.display(),
        captioned_screenshot = captioned_screenshot.display(),
        checkpoint_state = checkpoint_state.display(),
    ))
    .unwrap();

    let artifacts = Runner::with_capture(
        RunOptions {
            publish: false,
            quiet: true,
        },
        FakeCapture,
    )
    .run_artifacts(&tape)
    .unwrap();

    let final_json = fs::read_to_string(&final_state).unwrap();
    let checkpoint_json = fs::read_to_string(&checkpoint_state).unwrap();
    let plain_png = fs::read(&plain_screenshot).unwrap();
    let captioned_png = fs::read(&captioned_screenshot).unwrap();

    assert_eq!(artifacts.output_paths, vec![final_state.clone()]);
    assert!(final_json.contains("terminal only"));
    assert!(!final_json.contains(caption));
    assert!(checkpoint_json.contains("terminal only"));
    assert!(!checkpoint_json.contains(caption));
    assert_ne!(plain_png, captioned_png);

    for path in [
        final_state,
        checkpoint_state,
        plain_screenshot,
        captioned_screenshot,
    ] {
        let _ = fs::remove_file(path);
    }
}

fn temp_output(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{suffix}-{name}"))
}

fn assert_send<T: Send>() {}

#[derive(Debug, Default)]
struct FakeCapture;

impl FrameCapture for FakeCapture {
    type Session = FakeSession;

    fn open(&mut self, request: CaptureRequest) -> Result<Self::Session> {
        assert!(request.canvas.width > 0);
        assert!(request.grid.columns > 0);
        Ok(FakeSession::default())
    }
}

#[derive(Debug, Default)]
struct FakeSession {
    text: String,
}

impl TerminalSession for FakeSession {
    fn write_vt(&mut self, bytes: &[u8]) {
        self.text.push_str(&String::from_utf8_lossy(bytes));
    }

    fn capture_frame_with_cursor(&mut self, _cursor_visible: bool) -> Result<Frame> {
        Ok(Frame {
            width: 1,
            height: 1,
            stride: 4,
            format: PixelFormat::Rgba8,
            pixels: vec![0, 0, 0, 255],
        })
    }

    fn screen_text(&mut self) -> Result<String> {
        Ok(self.text.clone())
    }

    fn terminal_state(&mut self) -> Result<TerminalState> {
        Ok(TerminalState {
            size: [1, 1],
            total_rows: 1,
            scrollback_rows: 0,
            title: String::new(),
            working_directory: String::new(),
            cursor: StateCursor {
                x: 0,
                y: 0,
                visible: false,
            },
            default_style: StateStyle::default(),
            styles: Vec::new(),
            viewport_text: self.text.clone(),
            scrollback_text: String::new(),
            viewport: vec![vec![StateSpan::Text(self.text.clone())]],
            scrollback: Vec::new(),
        })
    }
}
