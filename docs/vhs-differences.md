# Differences From VHS

This document lists meaningful differences between Betamax and
[VHS](https://github.com/charmbracelet/vhs). It intentionally omits features that work the same in
both tools; see the [Tape Reference](tape-reference.md) for Betamax's full supported tape surface.

Sources checked:

- <https://github.com/charmbracelet/vhs>
- <https://www.mankier.com/1/vhs>
- <https://man.archlinux.org/man/vhs.1.en>

## Summary

| Area              | VHS                                | Betamax                                      |
| ----------------- | ---------------------------------- | -------------------------------------------- |
| Terminal engine   | Browser terminal through xterm.js  | `libghostty-vt` in process                   |
| Rendering         | Browser and ffmpeg filter pipeline | Rust raster renderer                         |
| Process model     | Browser/server-oriented pipeline   | No `ttyd`, browser server, or xterm.js layer |
| State snapshots   | Not a primary feature              | JSON viewport, scrollback, and styled spans  |
| Theme source      | VHS theme set and JSON themes      | Ghostty themes and JSON themes               |
| `Source`          | Supported                          | Parsed but intentionally not executed        |
| `record`/`serve`  | Supported                          | Intentionally not implemented                |
| `publish`         | Supported                          | Intentionally not implemented                |
| Video encoding    | ffmpeg-backed                      | ffmpeg-backed                                |
| GIF/PNG rendering | Browser/server pipeline            | In-process Rust                              |

## Architecture

Betamax's core difference is architectural. VHS drives a browser terminal and composes output
through a browser/ffmpeg-oriented pipeline. Betamax runs a PTY directly, feeds terminal bytes into
`libghostty-vt`, rasterizes terminal frames with `cosmic-text` and `swash`, and writes GIF/PNG
outputs in Rust.

MP4 and WebM are still encoded through `ffmpeg`. The difference is that terminal execution and
rasterization stay in process; video container encoding is the only remaining external encoder
bridge.

## Testing State

Betamax adds structured terminal-state output:

- `Output <path>.json` writes final terminal state.
- `State <path>.json` writes checkpoint terminal state.
- State JSON includes viewport text, scrollback text, cursor metadata, a default style,
  deduplicated non-default styles, and compact styled spans.
- `Runner::run_artifacts` returns final terminal state for Rust callers.

This is not a direct VHS parity feature. It is aimed at CLI/TUI snapshot tests where text,
scrollback, cursor state, and styles are useful assertions.

## Themes

VHS ships its own theme set and supports inline JSON themes. Betamax uses copied Ghostty theme files
plus inline JSON themes, and the `themes` command lists effective theme names from user Ghostty
theme directories and bundled resources.

The visible result should be close for common named themes and base16-style JSON themes, but exact
theme metadata is not identical. Selection colors, cursor text colors, and other theme fields are
not fully represented in Betamax's current renderer.

## Intentional Omissions

Betamax intentionally omits these VHS features for the first cut:

- `Source <path>.tape`
- `record`
- `serve`
- `publish`

`Source` is parsed so migrated VHS tapes get a clear error, but execution returns an explicit
not-implemented error.

## Fidelity Risks

Betamax's renderer is good enough for the current examples, but it is not Ghostty's full renderer
and is not VHS's browser output. Known risks:

- no ligature shaping parity guarantee with Ghostty;
- no image/Kitty graphics rendering path;
- limited cursor/text composition compared with full terminal renderers;
- possible differences in wide glyphs, combining marks, emoji, and fallback fonts.

Key input has a similar caveat. Betamax routes key events through `libghostty-vt::key::Encoder` with
fallbacks for common control sequences, but broader named-key coverage and live terminal-mode input
state remain future fidelity work.

## Betamax-Only Features

These are useful, but not VHS parity features:

- strict JSON state snapshots for viewport, scrollback, and styles;
- scrollback-inclusive state capture;
- `Runner::run_artifacts` for Rust callers;
- local README demo preview workflow under `target/betamax-examples`.
