# betamax-core

`betamax-core` is the reusable library behind Betamax.

It parses tape files, runs terminal sessions through `portable-pty` and `libghostty-vt`, renders
terminal frames, writes media outputs, and returns structured terminal state for snapshot-style
tests.

```rust
use betamax_core::{Runner, Tape};

# fn main() -> betamax_core::Result<()> {
let tape = Tape::parse(r#"
Output /tmp/betamax-state.json
Set Shell "bash"
Type "printf 'hello\n'"
Enter
Wait+Screen "hello"
"#)?;
let artifacts = Runner::new().run_artifacts(&tape)?;
assert!(artifacts.final_state.is_some());
# Ok(())
# }
```

Most users should install the `betamax` CLI crate. Use this crate directly when embedding Betamax in
Rust tests or tools.

For a snapshot test that drives a terminal program and checks its state with
[insta](https://insta.rs/), see
[Snapshot state with insta](https://www.joshka.net/betamax/testing/terminal-testing/#snapshot-state-with-insta)
and the [compiled example](https://github.com/joshka/betamax/blob/main/crates/betamax-core/tests/insta_snapshot.rs).

Betamax currently supports macOS and Linux. Windows is not supported because the upstream
`libghostty-vt-sys` native build does not support Windows. MP4 and WebM output require `ffmpeg` on
`PATH`.

See the [Betamax documentation site](https://www.joshka.net/betamax/) for command behavior,
settings, defaults, testing workflows, and examples. See
[`docs.rs/betamax-core`](https://docs.rs/betamax-core) for the Rust API reference.
