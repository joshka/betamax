# Roadmap

Planned work, in suggested priority order. See [Differences from VHS](vhs-differences.md) for
compatibility and unsupported VHS features.

## Renderer fidelity

Extend fixtures for wide glyphs, combining marks, emoji sequences, and fallback fonts. Compare
terminal state and rendered images, then fix differences from expected output. See
[Renderer fidelity](renderer-fidelity.md) for current coverage.

## Terminal modes and key encoding

Pass live terminal mode state to `libghostty-vt::key::Encoder`. Identify which modes the bindings
expose and test applications that switch cursor-key, application-keypad, or modifier behavior.

## In-memory outputs

Decide whether library callers should be able to receive images, frames, state snapshots, and
diagnostics without writing files. `Runner::run_artifacts` already returns final terminal state;
`Output` and `State` commands still write files.

## Testing the runner

If direct library use grows, consider injectable process, clock, and filesystem interfaces.
The runner currently controls PTY startup, sleeps, file writes, and ffmpeg execution. Substitutes
would let tests exercise timing and failures without real shells or media files.

## Benchmarks

Add benchmarks once renderer behavior and the state format settle. Candidate measurements include
raster rendering, terminal-state compaction, frame decoration, and GIF encoding.

## Video encoding

Keep MP4 and WebM encoding in optional `ffmpeg` unless users need native encoding. Native support
would remove an external tool requirement but add codec dependencies, build complexity, licensing
review, and platform tests.

## Upstream frame capture

Watch for a supported libghostty API that renders frames for capture. The Rust bindings Betamax
uses do not currently expose one. Keep rendering separate from the runner so Betamax can adopt
such an API if it becomes available.
