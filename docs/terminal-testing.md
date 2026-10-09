# Terminal testing

Test a CLI or TUI with a tape that runs the program, waits for expected text, and captures
screenshots or state JSON at checkpoints. The tape fails if the text does not appear before the
timeout.

After each action, use `Wait`, `Wait+Line`, or `Wait+Screen` to check the result before capturing it.
Add `Sleep` when viewers need time to read the recording, or when there is no text to wait for.
For recordings, try 300-700 ms after simple transitions and 1.5-2.5 seconds for a screen of short
output. Reserve 4-5 second pauses for the final screen.

For test-oriented tapes, prefer:

- `Require` for external programs the test depends on.
- `Wait+Screen@<duration> "<text>"` to synchronize with application output.
- `AssertText`, `AssertCells`, `AssertStyle`, `AssertAbsent`, and `AssertState` for settled
  checkpoint assertions.
- `MouseMove`, `MouseDown`, `MouseUp`, and `MouseScroll` for application mouse input in terminal
  cells. Use `Wait+Screen` after the action to synchronize with redraw.
- `Screenshot <path>.png` for debugging failed or changed terminal states.
- `State <path>.json` for checkpoint snapshots that can be compared with snapshot-testing tools
  such as `insta`.
- `Output <path>.json` for final terminal state.
- `Hide` around setup and cleanup commands to keep captured artifacts focused.

State JSON includes terminal dimensions, cursor metadata, `viewport_text`, `scrollback_text`, a
single `default_style`, deduplicated non-default `styles`, and compact styled spans for the viewport
and scrollback. Plain string spans use the default style, while `[text, style_index]` spans
reference the `styles` array. See [State JSON](state-json.md) for the full format and the
JSON/YAML/TOML/JSONC tradeoffs.

The same runner is available as a Rust library for tests:

```rust
use betamax_core::{RunOptions, Runner, Tape};

let tape = Tape::parse(r#"
Output /tmp/state.json
Set Shell "bash"
Type "printf 'hello\n'"
Enter
Wait+Screen "hello"
Hide
Type "exit"
Enter
"#)?;
let artifacts = Runner::new(RunOptions::default()).run_artifacts(&tape)?;
assert!(artifacts.final_state.unwrap().viewport_text.contains("hello"));
# Ok::<(), miette::Report>(())
```

For Betamax's own deterministic renderer checks, see [Renderer Fidelity](renderer-fidelity.md).

## Automated terminal UX

[Mouse tape](../examples/ratatui-diff-mouse.tape) and
[resize tape](../examples/ratatui-diff-resize.tape) exercise the merged `ratatui-diff` viewer.
Build its `viewer` example and set `BETAMAX_DIFF_VIEWER` to the executable path. CI pins merged
revision `e05b74b4236e4306e93feb242b6e005e467c1077` for reproducible acceptance behavior.
Run tapes from the Betamax checkout root. They pin Aardvark Ink, JetBrains Mono, and media geometry;
mouse cells describe terminal coordinates rather than screenshot pixels. Copy preview belongs to
that example and does not exercise an OS clipboard.

The [combined UX tape](../examples/ratatui-diff-ux.tape) asserts search counts and navigation,
word-highlight styles, correct-side source selection, keyboard selection, mode changes, and tiny
resize behavior. The [Unicode tape](../examples/ratatui-diff-unicode.tape) selects across wrapped
source lines; [the checker](../scripts/check-terminal-ux.py) compares the complete preview with an
original text fixture, proving display wraps do not add source newlines. It temporarily grows the
grid to expose the full preview in state JSON; the media canvas remains fixed. The
[multi-file tape](../examples/ratatui-diff-multi-file.tape) checks nine search matches across three
files, mode/resize continuity, and start/end navigation.

Betamax's own [event fixture](../crates/betamax-core/tests/fixtures/terminal_events.py) runs in a real
PTY and independently decodes input with Python's standard library. It covers named keys, mouse
press/drag/release, wheel direction, and normal child resize notification. The `terminal_input` and
`terminal_assertions` integration tests run in macOS and Linux CI. Failure PNG/JSON/diff artifacts
are uploaded even when tests fail.

Checkpoint text and cell/style assertions provide the first baseline surface. Pixel baselines need
pinned fonts, theme, and geometry plus meaningful tolerances and an explicit approval workflow;
they are deferred. Real-terminal subjective feel and OS clipboard integration remain manual checks.
