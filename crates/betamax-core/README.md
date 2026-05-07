# betamax-core

`betamax-core` is the reusable library behind Betamax.

It parses tape files, runs terminal sessions through `portable-pty` and `libghostty-vt`, renders
terminal frames, writes media outputs, and returns structured terminal state for snapshot-style
tests.

```rust
use betamax_core::{RunOptions, Runner, Tape};

# fn main() -> betamax_core::Result<()> {
let tape = Tape::parse(r#"
Output /tmp/betamax-state.json
Set Shell "bash"
Type "printf 'hello\n'"
Enter
Wait+Screen "hello"
"#)?;
let artifacts = Runner::new(RunOptions::default()).run_artifacts(&tape)?;
assert!(artifacts.final_state.is_some());
# Ok(())
# }
```

Most users should install the `betamax` CLI crate. Use this crate directly when embedding Betamax in
Rust tests or tools.

Betamax currently supports macOS and Linux. Windows is not supported because the upstream
`libghostty-vt-sys` native build does not support Windows. MP4 and WebM output require `ffmpeg` on
`PATH`.

See the [Betamax documentation site](https://www.joshka.net/betamax/) for command behavior,
settings, defaults, testing workflows, and examples. See
[`docs.rs/betamax-core`](https://docs.rs/betamax-core) for the Rust API reference.
