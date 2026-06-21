# Terminal Testing

Betamax can be used as a terminal testing harness. A tape can run an interactive terminal program,
wait for text on the screen, capture PNG screenshots at checkpoints, write structured terminal
state, and fail when expected terminal output does not appear before a timeout. That makes it useful
for CLI and TUI smoke tests in the same broad role that Playwright fills for browser flows.

Validation tapes and review-media tapes optimize for different readers. Validation tapes exercise a
user journey, wait for semantic screen state, and write focused `State` or `Screenshot` checkpoints
for assertions. Review-media tapes make the same kind of journey readable for humans in a GIF,
video, screenshot, or pull-request artifact.

Waits are assertions and synchronization points. In validation tapes, put a semantic `Wait`,
`Wait+Line`, or `Wait+Screen` after each meaningful app transition, then add `Sleep` only when the
generated media needs human pacing or no semantic wait is possible. Useful review-media pauses are
300-700 ms after simple transitions, 1.5-2.5 seconds for stable simple screens, and 4-5 seconds
only for final review endpoints.

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
