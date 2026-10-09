//! Exercise input through a real PTY and an independent event decoder.

use std::path::PathBuf;

use betamax_core::ghostty::{
    CaptureRequest, GhosttyFrameCapture, PixelSize, TerminalGrid, TerminalTheme, TextSettings,
};
use betamax_core::tape::{KeyModifiers, MouseButton, MouseEvent, ScrollDirection};
use betamax_core::{RunOptions, Runner, Tape, TerminalSession};

#[test]
fn mouse_and_keys_reach_real_pty_with_cell_coordinates() {
    run_fixture(
        "mouse",
        r#"
        Up
        Wait+Screen@2s "key up"
        MouseMove 4 2
        Shift+MouseDown Left
        Wait+Screen@2s "mouse down left 4 2 shift"
        Ctrl+MouseMove 8 3
        Wait+Screen@2s "mouse move left 8 3 ctrl"
        MouseUp Left
        Wait+Screen@2s "mouse up left 8 3 none"
        Alt+MouseScroll Down 2
        Wait+Screen@2s "mouse down wheel 8 3 alt"
        MouseScroll Up 1
        Wait+Screen@2s "mouse up wheel 8 3 none"
        MouseScroll Left 1
        Wait+Screen@2s "mouse left wheel 8 3 none"
        MouseScroll Right 1
        Wait+Screen@2s "mouse right wheel 8 3 none"
        Type "x"
        Wait+Screen@2s "format x10"
        MouseMove 9 4
        MouseDown Right
        Wait+Screen@2s "mouse down right 9 4 none"
        MouseUp Right
        Wait+Screen@2s "mouse up none 9 4 none"
    "#,
    );
}

#[test]
fn resize_reaches_child_and_updates_live_grid() {
    let state = run_fixture(
        "resize",
        r#"
        Resize 50 16
        Wait+Screen@2s "resize 50 16"
        MouseMove 49 15
        MouseDown Left
        Wait+Screen@2s "mouse down left 49 15 none"
        MouseUp Left
        Resize 30 8
        Wait+Screen@2s "resize 30 8"
        Resize 100 30
        Wait+Screen@2s "resize 100 30"
    "#,
    );
    assert_eq!(state.size, [100, 30]);
}

#[test]
fn resize_keeps_canvas_and_clamps_pointer_with_held_button() {
    let mut terminal = GhosttyFrameCapture
        .open(CaptureRequest {
            canvas: PixelSize::new(200, 100),
            grid: TerminalGrid::new(10, 5),
            text: TextSettings::default(),
            theme: TerminalTheme::default(),
        })
        .unwrap();
    terminal.write_vt(b"\x1b[?1006h\x1b[?1002h");
    let modifiers = KeyModifiers::default();
    terminal
        .mouse_input(MouseEvent::Move { column: 9, row: 4 }, modifiers)
        .unwrap();
    terminal
        .mouse_input(MouseEvent::Down(MouseButton::Left), modifiers)
        .unwrap();
    terminal.resize(TerminalGrid::new(3, 2)).unwrap();
    assert_eq!(terminal.terminal_state().unwrap().size, [3, 2]);
    assert_eq!(
        terminal
            .mouse_input(MouseEvent::Up(MouseButton::Left), modifiers)
            .unwrap(),
        b"\x1b[<0;3;2m"
    );
    let frame = terminal.capture_frame_with_cursor(false).unwrap();
    assert_eq!((frame.width, frame.height), (200, 100));
    terminal.resize(TerminalGrid::new(40, 20)).unwrap();
    assert_eq!(terminal.terminal_state().unwrap().size, [40, 20]);
    let frame = terminal.capture_frame_with_cursor(false).unwrap();
    assert_eq!((frame.width, frame.height), (200, 100));
    assert!(terminal.resize(TerminalGrid::new(0, 2)).is_err());
    assert_eq!(terminal.terminal_state().unwrap().size, [40, 20]);
    terminal.resize(TerminalGrid::new(1, 1)).unwrap();
    assert_eq!(terminal.terminal_state().unwrap().size, [1, 1]);
}

fn run_fixture(name: &str, commands: &str) -> betamax_core::ghostty::TerminalState {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terminal_events.py");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/terminal-input")
        .join(name);
    std::fs::create_dir_all(&root).unwrap();
    // Retain point-in-time diagnostics even when a later Wait fails in CI.
    let commands = commands
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("Wait") {
                format!(
                    "State {}/actual.json\nScreenshot {}/actual.png\n{line}",
                    root.display(),
                    root.display()
                )
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let tape = Tape::parse(&format!(
        r#"
        Set Shell "python3 {}"
        Set Width 1000
        Set Height 640
        Set Padding 0
        Set FontSize 18
        Set TypingSpeed 0ms
        Hide
        Wait+Screen@2s "ready"
        {commands}
        Type "q"
        Wait+Screen@2s "bye"
        State {}/final.json
    "#,
        fixture.display(),
        root.display()
    ))
    .unwrap();
    Runner::new(RunOptions {
        quiet: true,
        publish: false,
    })
    .run_artifacts(&tape)
    .unwrap()
    .final_state
    .unwrap()
}

#[test]
fn tracking_modes_filter_motion_and_disabled_reporting() {
    let mut terminal = GhosttyFrameCapture
        .open(CaptureRequest {
            canvas: PixelSize::new(100, 100),
            grid: TerminalGrid::new(10, 5),
            text: TextSettings::default(),
            theme: TerminalTheme::default(),
        })
        .unwrap();
    let modifiers = KeyModifiers::default();
    assert!(terminal
        .mouse_input(MouseEvent::Down(MouseButton::Left), modifiers)
        .unwrap()
        .is_empty());
    terminal
        .mouse_input(MouseEvent::Up(MouseButton::Left), modifiers)
        .unwrap();
    terminal.write_vt(b"\x1b[?1006h\x1b[?1000h");
    assert!(terminal
        .mouse_input(MouseEvent::Move { column: 4, row: 2 }, modifiers)
        .unwrap()
        .is_empty());
    assert_eq!(
        terminal
            .mouse_input(MouseEvent::Down(MouseButton::Middle), modifiers)
            .unwrap(),
        b"\x1b[<1;5;3M"
    );
    assert!(terminal
        .mouse_input(MouseEvent::Move { column: 5, row: 2 }, modifiers)
        .unwrap()
        .is_empty());
    terminal.write_vt(b"\x1b[?1002h");
    assert_eq!(
        terminal
            .mouse_input(MouseEvent::Move { column: 6, row: 2 }, modifiers)
            .unwrap(),
        b"\x1b[<33;7;3M"
    );
    assert_eq!(
        terminal
            .mouse_input(MouseEvent::Up(MouseButton::Middle), modifiers)
            .unwrap(),
        b"\x1b[<1;7;3m"
    );
    assert!(terminal
        .mouse_input(MouseEvent::Move { column: 7, row: 2 }, modifiers)
        .unwrap()
        .is_empty());
    terminal.write_vt(b"\x1b[?1003h");
    assert_eq!(
        terminal
            .mouse_input(MouseEvent::Move { column: 8, row: 2 }, modifiers)
            .unwrap(),
        b"\x1b[<35;9;3M"
    );

    terminal.write_vt(b"\x1b[?1003l\x1b[?1002l\x1b[?1000l");
    assert!(terminal
        .mouse_input(
            MouseEvent::Scroll {
                direction: ScrollDirection::Down,
                count: 3
            },
            modifiers
        )
        .unwrap()
        .is_empty());
}

#[test]
fn coordinates_follow_negotiated_formats_and_cell_metrics() {
    for font_size in [10.0, 30.0] {
        let text = TextSettings {
            font_size,
            padding: 99,
            ..TextSettings::default()
        };
        let width = text.cell_width();
        let height = text.cell_height();
        let mut terminal = GhosttyFrameCapture
            .open(CaptureRequest {
                canvas: PixelSize::new(2000, 1000),
                grid: TerminalGrid::new(300, 10),
                text,
                theme: TerminalTheme::default(),
            })
            .unwrap();
        let mods = KeyModifiers {
            ctrl: true,
            shift: true,
            alt: true,
        };
        terminal.write_vt(b"\x1b[?1000h\x1b[?1006h");
        terminal
            .mouse_input(MouseEvent::Move { column: 4, row: 2 }, mods)
            .unwrap();
        assert_eq!(
            terminal
                .mouse_input(MouseEvent::Down(MouseButton::Left), mods)
                .unwrap(),
            b"\x1b[<28;5;3M"
        );
        terminal
            .mouse_input(MouseEvent::Up(MouseButton::Left), mods)
            .unwrap();
        terminal.write_vt(b"\x1b[?1006l\x1b[?1015h");
        assert_eq!(
            terminal
                .mouse_input(MouseEvent::Down(MouseButton::Left), mods)
                .unwrap(),
            b"\x1b[60;5;3M"
        );
        terminal
            .mouse_input(MouseEvent::Up(MouseButton::Left), mods)
            .unwrap();
        terminal.write_vt(b"\x1b[?1015l\x1b[?1005h");
        terminal
            .mouse_input(
                MouseEvent::Move {
                    column: 250,
                    row: 2,
                },
                mods,
            )
            .unwrap();
        assert_eq!(
            terminal
                .mouse_input(MouseEvent::Down(MouseButton::Left), mods)
                .unwrap(),
            "\x1b[M<\u{11b}#".as_bytes()
        );
        terminal
            .mouse_input(MouseEvent::Up(MouseButton::Left), mods)
            .unwrap();
        terminal.write_vt(b"\x1b[?1005l");
        assert!(terminal
            .mouse_input(MouseEvent::Down(MouseButton::Left), mods)
            .unwrap()
            .is_empty());
        terminal
            .mouse_input(MouseEvent::Up(MouseButton::Left), mods)
            .unwrap();
        terminal.write_vt(b"\x1b[?1006h\x1b[?1016h");
        terminal
            .mouse_input(MouseEvent::Move { column: 4, row: 2 }, mods)
            .unwrap();
        let bytes = terminal
            .mouse_input(MouseEvent::Down(MouseButton::Left), mods)
            .unwrap();
        let expected = format!(
            "\x1b[<28;{};{}M",
            (4.5 * width as f32).round() as u32,
            (2.5 * height as f32).round() as u32
        );
        assert_eq!(bytes, expected.as_bytes());
    }
}

#[test]
fn bounds_and_button_transitions_fail_actionably() {
    let mut terminal = GhosttyFrameCapture
        .open(CaptureRequest {
            canvas: PixelSize::new(100, 100),
            grid: TerminalGrid::new(10, 5),
            text: TextSettings::default(),
            theme: TerminalTheme::default(),
        })
        .unwrap();
    let modifiers = KeyModifiers::default();
    let error = terminal
        .mouse_input(MouseEvent::Move { column: 10, row: 0 }, modifiers)
        .unwrap_err();
    assert!(error.to_string().contains("outside 10 columns by 5 rows"));
    assert!(terminal
        .mouse_input(MouseEvent::Up(MouseButton::Left), modifiers)
        .unwrap_err()
        .to_string()
        .contains("not pressed"));
    terminal
        .mouse_input(MouseEvent::Down(MouseButton::Left), modifiers)
        .unwrap();
    assert!(terminal
        .mouse_input(MouseEvent::Down(MouseButton::Left), modifiers)
        .unwrap_err()
        .to_string()
        .contains("already pressed"));
}
