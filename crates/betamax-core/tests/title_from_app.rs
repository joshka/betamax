//! App and tape title assignments share one display value; raw state remains app-owned.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use betamax_core::{Runner, Tape};

fn image(path: &Path) -> (usize, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    bytes.truncate(info.buffer_size());
    (info.width as usize, bytes)
}

fn bar(path: &Path) -> Vec<u8> {
    let (width, pixels) = image(path);
    pixels[12 * width * 4..42 * width * 4].to_vec()
}

fn state_title(path: &Path) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    value["title"].as_str().map(str::to_owned)
}

#[test]
fn title_changes_follow_last_writer_and_preserve_bursts_and_raw_json() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/title-from-app-tests")
        .join(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_string(),
        );
    std::fs::create_dir_all(&root).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/title_events.py");
    let source = format!(
        r#"
        Output {root}/frames
        Set Shell "python3 {fixture}"
        Set Preset Default-0.1
        Set Width 400
        Set Height 240
        Set Padding 0
        Set FontSize 16
        Set TypingSpeed 0ms
        Set CursorBlink false
        Set WindowBar Colorful
        Set Margin 12
        Set WindowTitle "title set by tape"
        Set WindowTitleFromTerminal true
        Hide
        Wait+Screen@5s "ready"
        Show
        Screenshot {root}/tape.png
        Type "a"
        Wait+Screen@5s "app-ready"
        Screenshot {root}/app.png
        Type "b"
        Wait+Screen@5s "update-ready"
        Screenshot {root}/updated.png
        WindowTitle "title reset by tape"
        Screenshot {root}/reset.png
        State {root}/reset.json
        Type "r"
        Wait+Screen@5s "restored-ready"
        Screenshot {root}/restored.png
        Type "e"
        Wait+Screen@5s "burst-ready"
        WindowTitle ""
        Screenshot {root}/blank.png
        State {root}/blank.json
        Type "c"
        Wait+Screen@5s "clear-ready"
        State {root}/clear.json
        Hide
        Type "q"
    "#,
        root = root.display(),
        fixture = fixture.display()
    );
    let tape = Tape::parse(&source).unwrap();
    let mut runner = Runner::new();
    runner.run(&tape).unwrap();
    let tape = bar(&root.join("tape.png"));
    let app = bar(&root.join("app.png"));
    let updated = bar(&root.join("updated.png"));
    let reset = bar(&root.join("reset.png"));
    assert_ne!(tape, app);
    assert_ne!(app, updated);
    assert_ne!(updated, reset);
    assert_eq!(updated, bar(&root.join("restored.png")));
    assert_ne!(reset, bar(&root.join("blank.png")));
    assert_eq!(
        state_title(&root.join("reset.json")).as_deref(),
        Some("title updated by app")
    );
    assert_eq!(
        state_title(&root.join("blank.json")).as_deref(),
        Some("title set by app")
    );
    assert_eq!(state_title(&root.join("clear.json")), None);
    let mut paths: Vec<_> = std::fs::read_dir(root.join("frames"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    let sequence: Vec<_> = paths.iter().map(|path| bar(path)).collect();
    let mut distinct = Vec::new();
    for title in sequence {
        if distinct.last() != Some(&title) {
            distinct.push(title);
        }
    }
    // The burst's A -> B -> A survives even though the terminal content is unchanged between OSCs.
    assert!(distinct
        .windows(3)
        .any(|titles| titles == [app.clone(), updated.clone(), app.clone()]));
}

#[test]
fn app_titles_do_not_change_the_bar_without_opt_in() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/title-from-app-tests")
        .join(format!(
            "off-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/title_events.py");
    let source = format!(
        r#"
        Set Shell "python3 {fixture}"
        Set Preset Default-0.1
        Set Width 400
        Set Height 240
        Set Padding 0
        Set FontSize 16
        Set TypingSpeed 0ms
        Set CursorBlink false
        Set WindowBar Colorful
        Set Margin 12
        Set WindowTitle "title set by tape"
        Hide
        Wait+Screen@5s "ready"
        Show
        Screenshot {root}/before.png
        Type "a"
        Wait+Screen@5s "app-ready"
        Screenshot {root}/after.png
        State {root}/raw.json
        WindowTitle "tape changed without app following"
        Screenshot {root}/reset.png
        Hide
        Type "q"
    "#,
        fixture = fixture.display(),
        root = root.display()
    );
    let tape = Tape::parse(&source).unwrap();
    let mut runner = Runner::new();
    runner.run(&tape).unwrap();
    assert_eq!(bar(&root.join("before.png")), bar(&root.join("after.png")));
    assert_ne!(bar(&root.join("after.png")), bar(&root.join("reset.png")));
    assert_eq!(
        state_title(&root.join("raw.json")).as_deref(),
        Some("title set by app")
    );
}

#[test]
fn hidden_title_bursts_only_show_the_latest_value() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/title-from-app-tests")
        .join(format!(
            "hidden-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&root).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/title_events.py");
    let source = format!(
        r#"
        Output {root}/frames
        Set Shell "python3 {fixture} startup"
        Set Preset Default-0.1
        Set Width 400
        Set Height 240
        Set Padding 0
        Set FontSize 16
        Set TypingSpeed 0ms
        Set CursorBlink false
        Set WindowBar Colorful
        Set Margin 12
        Set WindowTitle "title set by tape"
        Set WindowTitleFromTerminal true
        Hide
        Wait+Screen@5s "ready"
        Type "e"
        Wait+Screen@5s "burst-ready"
        Show
        Screenshot {root}/shown.png
        State {root}/shown.json
        Hide
        Type "q"
    "#,
        fixture = fixture.display(),
        root = root.display()
    );
    let tape = Tape::parse(&source).unwrap();
    let mut runner = Runner::new();
    runner.run(&tape).unwrap();
    assert_eq!(
        state_title(&root.join("shown.json")).as_deref(),
        Some("title set by app")
    );
    let frames: Vec<_> = std::fs::read_dir(root.join("frames"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(frames.len(), 1, "hidden title history must not be replayed");
    assert_eq!(bar(&frames[0]), bar(&root.join("shown.png")));
}
