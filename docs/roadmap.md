# Roadmap

This document tracks follow-up work that is not part of the VHS differences list. The differences
document should stay focused on user-visible behavior and intentionally omitted VHS features.

## Suggested Priority

1. Improve renderer fidelity around wide glyphs, combining marks, emoji, and fallback fonts.

   Betamax currently rasterizes the terminal grid itself with `cosmic-text` and `swash`. That is
   enough for common demos, but terminal output can contain wide CJK characters, combining marks,
   emoji sequences, and fonts that require fallback or shaping decisions. This work should add
   focused fixtures and image/state assertions for those cases, then tighten renderer behavior where
   Betamax diverges from expected terminal output.

1. Feed live terminal mode state into `libghostty-vt::key::Encoder`.

   Key encoding can depend on the terminal's active modes. Betamax routes key commands through
   `libghostty-vt::key::Encoder`, but the current integration does not yet model every live terminal
   mode that can change escape sequences. This work should identify the mode state exposed by
   `libghostty-vt`, thread it into key encoding, and add fixtures for applications that switch
   cursor-key, application-keypad, or modifier behavior.

1. Decide whether the library API should expose structured artifacts without writing files.

   `Runner::run_artifacts` already returns final terminal state for Rust callers, while tape
   commands such as `Output` and `State` still describe file outputs. Library users may want to run
   a tape and receive images, frames, state snapshots, or diagnostics directly in memory for tests.
   This work should settle the public API shape before 0.1 exposes patterns that are hard to change.

1. Introduce mockable runtime boundaries if the library API starts serving test harnesses directly.

   The current runner owns PTY spawning, wall-clock sleeps, filesystem writes, and the ffmpeg process
   boundary. That is pragmatic for the CLI, but a library-oriented test harness would benefit from
   injectable process, clock, and filesystem seams so failure and timing behavior can be tested
   without spawning real shells or writing real media files.

1. Add targeted benchmarks after the renderer and state format settle.

   The likely benchmark targets are software raster rendering, terminal-state compaction, frame
   decoration, and GIF encoding. Benchmarking before those shapes settle would add maintenance cost
   without giving much release signal.

1. Keep MP4/WebM on optional `ffmpeg` unless native encoding becomes worth the packaging cost.

   GIF and PNG rendering are in process. MP4 and WebM currently use `ffmpeg`, which keeps Betamax
   smaller and avoids taking on video-container and codec maintenance. Native encoding would reduce
   external tooling requirements, but it would add dependencies, build complexity, licensing review,
   and cross-platform test coverage. This should stay external until there is a clear user-facing
   need.

1. Track upstream libghostty APIs for renderer frame capture.

   The ideal long-term rendering path may be for libghostty to render frames itself, then let Betamax
   capture those frames for GIF, PNG, and video output. Ghostty discussions and PR comments have
   mentioned renderer-level frame capture for OpenGL and Metal, but that is not currently available
   through the Rust APIs Betamax uses. This work is mainly watching upstream and keeping Betamax's
   renderer boundary narrow enough to swap when a supported libghostty capture API exists.
