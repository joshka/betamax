# Differences from VHS

This page compares Betamax with [VHS](https://github.com/charmbracelet/vhs).
See the [Tape reference][tape-reference] for all supported commands and settings.

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
| `Source`          | Supported, one level, CWD paths    | Nested, tape-relative paths, CWD fallback    |
| `record`/`serve`  | Supported                          | Intentionally not implemented                |
| `publish`         | Supported                          | Intentionally not implemented                |
| Video encoding    | ffmpeg-backed                      | ffmpeg-backed                                |
| GIF/PNG rendering | Browser/server pipeline            | In-process Rust                              |

## Presentation defaults

Betamax defaults to `Framed`: Aardvark Ink, 1200×720, 22px text, zero terminal padding, and a
rounded window with left dots, a backdrop, and a shadow. Existing tapes can select `Set Preset Default-0.1`
to restore Betamax's previous defaults. `Set Preset Bare` removes decoration while keeping the new
sizing and typography. These preset commands are Betamax extensions; they do not make tapes or
rendered pixels identical to VHS. See [Presets][presets] for resolved values and override rules.

## Architecture

Betamax's core difference is architectural. VHS drives a browser terminal and composes output
through a browser/ffmpeg-oriented pipeline. Betamax runs a PTY directly, feeds terminal bytes into
`libghostty-vt`, rasterizes terminal frames with `cosmic-text` and `swash`, and writes GIF/PNG
outputs in Rust.

Betamax uses `ffmpeg` to encode MP4 and WebM. Terminal parsing and rendering run in process.

## Testing state

Betamax adds structured terminal-state output:

- `Output <path>.json` writes final terminal state.
- `State <path>.json` writes checkpoint terminal state.
- State JSON includes viewport text, scrollback text, cursor metadata, a default style,
  deduplicated non-default styles, and compact styled spans.
- `Runner::run_artifacts` returns final terminal state for Rust callers.

Use these snapshots to assert text, scrollback, cursor state, and styles in CLI/TUI tests.

## Themes

VHS ships its own theme set and supports inline JSON themes. Betamax uses copied Ghostty theme files
plus inline JSON themes, and the `themes` command lists effective theme names from user Ghostty
theme directories and bundled resources.

All 348 VHS theme names resolve. VHS's names come from a 2023 snapshot of iTerm2-Color-Schemes,
which has since renamed most themes, so Betamax maps each old VHS spelling to its current name
(`tokyonight` loads `TokyoNight`). `betamax themes` lists only current names. Some themes changed
colors upstream since that snapshot, usually bright black (color 8). The 29 VHS themes with no
Ghostty counterpart are bundled with VHS's colors under normalized names (`CyberPunk2077` loads
`Cyberpunk 2077`). Other names match ignoring case, spaces, and punctuation, and unknown names get
"did you mean" suggestions. `scripts/import-vhs-themes.py` regenerates the alias table and bundled
VHS themes.

The visible result should be close for common named themes and base16-style JSON themes, but exact
theme metadata is not identical. Selection colors, cursor text colors, and other theme fields are
not fully represented in Betamax's current renderer.

## Tape ordering and `Type`

Startup commands after runtime commands follow VHS, with warnings: `Set TypingSpeed` applies from
that point, `Set Shell` and `Env` apply to the whole tape, and other settings are ignored. Two
differences: a late `Require` is still checked, where VHS skips it, and an `Env` does not end the
header, so a `Set` after it still applies, where VHS ignores it. `Type "a" "b"` types `a b`, as in
VHS, and warns. See
[Startup commands after runtime commands][startup-order].

## Intentional omissions

These VHS features are not implemented:

- `record`
- `serve`
- `publish`

`Source <path>.tape` is supported. Betamax resolves the path against the sourcing tape's directory
before the current directory and allows nested `Source` with cycle detection. Sourced startup
commands follow the same ordering rule as the rest of the tape.

## Fidelity risks

Betamax uses its own renderer, so output can differ from Ghostty and VHS. Known limitations:

- no ligature shaping parity guarantee with Ghostty;
- inline images cover the kitty graphics protocol (including unicode placeholders), sixel, and
  iTerm2 `OSC 1337 File=inline=1`; kitty animation frames and iTerm2 multipart transfers are not
  rendered;
- limited cursor/text composition compared with full terminal renderers;
- possible differences in wide glyphs, combining marks, emoji, and fallback fonts.

Key input has a similar caveat. Betamax routes key events through `libghostty-vt::key::Encoder` with
fallbacks for common control sequences, but broader named-key coverage and live terminal-mode input
state remain future fidelity work.

## Betamax-only features

Betamax also provides:

- strict JSON state snapshots for viewport, scrollback, and styles;
- inline images drawn by programs with kitty graphics, sixel, or iTerm2 sequences;
- scrollback-inclusive state capture;
- `Runner::run_artifacts` for Rust callers;
- local README demo preview workflow under `target/betamax-examples`.

[presets]: https://www.joshka.net/betamax/reference/tape-reference/#presets
[startup-order]: https://www.joshka.net/betamax/reference/tape-reference/#startup-commands-after-runtime-commands
[tape-reference]: https://www.joshka.net/betamax/reference/tape-reference/
