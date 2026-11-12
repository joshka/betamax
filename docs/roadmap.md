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

## Assertion timing

Assertions pass on the first matching frame within `WaitTimeout`; `AssertAbsent@<duration>` holds
for a whole period. That keeps animation from failing a tape and lets an assertion follow an action
without its own wait. Consider making this configurable, for example a per-tape setting or an
`@duration` on each assertion, so tapes can choose single-frame checks or shorter timeouts without
adding commands.

## libghostty 1.0

Betamax uses `libghostty-vt` 0.2, which builds a Ghostty revision from before libghostty 1.0. When
the Rust bindings publish a release based on 1.0 (it needs Zig 0.17), move to it and adopt:

- **Exact synchronized-update boundaries** (`on_render_hold`). Betamax checks mode 2026 after each
  PTY read, so when one read ends an update and starts the next, the frame in between is never
  recorded. The callback fires at the byte where the mode changes, so that frame can be kept.
- **Prompt detection** (`is_cursor_at_prompt`). For shells that emit OSC 133 prompt markers, a plain
  `Wait` and shell startup could wait for the prompt itself rather than match `WaitPattern` text or
  wait for output to stop.
- **Image placement** (`render::ImageIterator`). Ghostty resolves each inline image's position,
  layer, and placeholder runs, which could replace most of Betamax's own placement code if the
  rendered output matches.
- **Animated kitty images** (`kitty_graphics_animation_tick`). Recordings show only the first frame
  of an animated image today; the tick would advance animations on Betamax's capture clock.
- **Replies to more queries.** `on_color_scheme` answers the light/dark query (`CSI ? 996 n`) from
  the theme. `on_xtversion` and `set_terminfo_name` report a terminal name and version to programs
  that detect features. `on_clipboard_read` could serve OSC 52 reads from the tape's `Copy`
  clipboard.
- **Unknown sequences in failure reports** (`on_unknown_sequence`). Failure bundles could list the
  escape sequences Ghostty ignored, so a report can say a program sent something the terminal does
  not support.

A trial build against the 1.0 bindings matched current output on every repeatable example tape.

## Video encoding

Keep MP4 and WebM encoding in optional `ffmpeg` unless users need native encoding. Native support
would remove an external tool requirement but add codec dependencies, build complexity, licensing
review, and platform tests.

## Upstream frame capture

Watch for a supported libghostty API that renders frames for capture. The Rust bindings Betamax
uses do not currently expose one. Keep rendering separate from the runner so Betamax can adopt
such an API if it becomes available.
