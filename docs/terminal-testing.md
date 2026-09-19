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
- `Wait+Screen@<duration> "<text>"` for explicit screen assertions.
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
