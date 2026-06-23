# Tape Reference

This page summarizes the tape commands and settings supported by Betamax. It is intentionally a
compact reference rather than a tutorial; start with the examples in the main README when learning
the tape format.

## Outputs

- `Output <path>.gif`
- `Output <path>.png`
- `Output <path>.mp4`
- `Output <path>.webm`
- `Output <path>.json`
- `Output <dir>`
- `Screenshot <path>.png`
- `State <path>.json`

GIF, PNG, screenshot, state JSON, and PNG sequence output are produced in process. MP4 and WebM
output require `ffmpeg` on `PATH`; Betamax reports that as a runtime error only when a video output
is requested. Install it with `brew install ffmpeg` on macOS or
`sudo apt-get update && sudo apt-get install ffmpeg` on Debian/Ubuntu.

An extensionless `Output` path is treated as a directory for numbered PNG frames.
`Caption <text>` commands render presentation text onto visual outputs, but do not create files.

## Settings

Common settings:

| Setting         | Value                                      | Default          | Notes                                       |
| --------------- | ------------------------------------------ | ---------------- | ------------------------------------------- |
| `Shell`         | string                                     | `$SHELL` or `sh` | Shell argv is split with shell-like rules.  |
| `Theme`         | theme name or inline JSON string           | `Aardvark Blue`  | Theme names use Ghostty-style lookup.       |
| `FontFamily`    | string                                     | `JetBrains Mono` | Falls back through the font system.         |
| `FontSize`      | number                                     | `22`             | Pixels.                                     |
| `LetterSpacing` | number                                     | `1`              | Additional pixels between cells.            |
| `LineHeight`    | number                                     | `1`              | Multiplier applied to font size.            |
| `Width`         | number                                     | `1200`           | Final output width in pixels.               |
| `Height`        | number                                     | `600`            | Final output height in pixels.              |
| `Padding`       | number                                     | `60`             | Inner padding before terminal cells.        |
| `Framerate`     | number                                     | `50`             | Capture cadence in frames per second.       |
| `TypingSpeed`   | duration                                   | `50ms`           | Default delay between typed characters.     |
| `PlaybackSpeed` | number                                     | `1.0`            | Changes output playback, not PTY timing.    |
| `LoopOffset`    | number                                     | `0.0`            | `0..=1` is a fraction; otherwise seconds.   |
| `CursorBlink`   | bool                                       | `true`           | Simulates a half-second blink cadence.      |
| KeyboardOverlay | `Off`, `Keys`, `Input`, or `All`           | `Off`            | Shows recent input chips in media.          |
| `WaitTimeout`   | duration or number                         | `15s`            | Number values are seconds.                  |
| `WaitPattern`   | regex string                               | `>$`             | Used by `Wait` without explicit pattern.    |
| `Margin`        | number                                     | `0`              | Outer decoration margin in pixels.          |
| `MarginFill`    | `#rrggbb` string                           | `#102040`        | Invalid colors fall back to theme bg.       |
| `WindowBar`     | `Rings`, `RingsRight`, `Colorful`, or etc. | empty / disabled | Any non-empty value enables the bar.        |
| `WindowBarSize` | number                                     | `30`             | Bar height in pixels when enabled.          |
| `BorderRadius`  | number                                     | `0`              | Rounded-corner mask radius in pixels.       |

Unknown settings and type mismatches are errors. For example, `Set Wdith 900` and
`Set Width "wide"` fail before the shell starts instead of silently falling back to defaults.

The terminal grid is derived after all settings are applied. Margin, window-bar decoration, and any
presentation row needed for captions or keyboard overlay are subtracted from width and height first,
then padding, font size, letter spacing, and line height determine the PTY columns and rows.
Extremely small dimensions are clamped to at least one row and one column.

Treat larger `FontSize`, larger `Margin`, and decorative frame settings as presentation zoom. When
the tape is proving modal placement, centered content, wrapping, or split-pane layout, prefer the
default `FontSize`, default `Margin`, and a wider `Width` or `Height` so the proof matches the
application layout rather than the demo framing.

`Set KeyboardOverlay Input` draws compact time-aware input chips on generated PNG, GIF, video,
screenshot, and frame-sequence media. Labels appear when input is queued and linger briefly after
the input is typed, so review GIFs show the action near the terminal change it caused. The overlay
is presentation-only: it does not change PTY input bytes, waits, state JSON, or final output
dimensions. When enabled, Betamax reserves a bottom presentation row before deriving the terminal
grid so chips do not cover terminal content. Keyboard chips are right-aligned to the terminal frame
edge and share the row with captions when both are active.

Keyboard overlay modes are:

- `Off`: no overlay.
- `Keys`: explicit key commands only, such as `Ctrl+P`, `Down`, `Enter`, and `Escape`.
- `Input`: key commands plus short typed input that reads like user intent.
- `All`: every visible input event, including long `Type` commands summarized by character count.

For built-in shell names such as `bash`, `zsh`, and `fish`, Betamax starts a clean recording shell
with startup files and history disabled where possible. Those shells use the VHS-style colored `>`
prompt by default, so captures do not inherit prompts such as `bash-5.3$` from the host
environment.

## Commands

### `Output <path>`

Adds a primary output for the tape. The runner classifies the output by extension after parsing and
before starting the shell.

Supported output paths:

- `.gif`: animated GIF written in process.
- `.png`: final-frame PNG written in process.
- `.mp4`: MP4 written through `ffmpeg`.
- `.webm`: WebM written through `ffmpeg`.
- `.json`: final terminal-state JSON.
- no extension: directory of numbered PNG frames.

`Output` paths are returned from `Runner::run_artifacts` in deterministic output-kind order, not
source order. Inline `Screenshot` and `State` commands are checkpoint outputs and are not included
in that primary output list.

### `Require <program>`

Checks that `<program>` exists on `PATH` before any PTY or output side effects occur. The check
looks for a file with that name in each `PATH` directory; platform-specific executable-bit failures
are left to the eventual process that uses the program.

`Require` must appear before runtime commands.

### `Set <name> <value>`

Applies a runtime setting. See [Settings](#settings) for supported settings, value types, defaults,
and behavior notes.

Unknown settings and type mismatches are errors. `Set` commands must appear before runtime commands
so terminal dimensions, shell startup, theme, and timing are fixed before execution begins.

### `Env <key> <value>`

Adds or overrides an environment variable for the spawned shell process. Betamax applies terminal
defaults first, then tape-provided `Env` entries, so `Env` can intentionally override `TERM`,
`COLORTERM`, `NO_COLOR`, prompt variables, or any application-specific variables.

`Env` must appear before runtime commands because the shell is spawned before runtime execution.

### `Sleep <duration>`

Pauses tape execution for a duration. In capture runs, Betamax continues to drain PTY output and
capture frames during the sleep so animations show output that appears while waiting.

Durations accept compact forms such as `500ms` and `1s`, as well as VHS-style forms such as `0.5`,
`500 ms`, and `1 s`. Bare numeric durations are seconds.

In validation tapes, use sleeps as a last resort for behavior that cannot be matched with a prompt,
status line, or visible screen text. In example and review-media tapes, sleeps are presentation
pacing after a semantic wait has already proved the screen state.

### `Type[@duration] <text>`

Types Unicode text into the PTY one scalar value at a time. `Type@duration` overrides the default
typing delay for that command; otherwise `Set TypingSpeed` applies. In capture runs, Betamax drains
output and captures frames after each typed character so typed text appears progressively.

Use `Type` for printable text. Use key commands for terminal control keys and modified key
combinations.

### `Copy <text>`

Stores text in Betamax's tape-local clipboard. This does not read or write the host operating
system clipboard.

### `Paste`

Writes the tape-local clipboard into the PTY. If no `Copy` command has run, the clipboard is empty
and `Paste` writes nothing.

### `Wait[@duration] [/regex/|text]`

Waits until terminal text matches a pattern while continuing to drain output and capture frames.
Bare `Wait` inspects the current cursor line and uses `Set WaitPattern` as its pattern.

Waits are assertions and synchronization points. Put `Wait`, `Wait+Line`, or `Wait+Screen` before
any sleep that only exists to make a GIF, video, or review artifact readable.

Pattern forms:

- `/regex/`: regular expression. The parser validates the regex before execution.
- `text`: plain substring match.
- omitted: use `Set WaitPattern`, which defaults to `>$`.

Timeout behavior:

- `Wait@duration` sets the timeout for that command.
- Otherwise `Set WaitTimeout` applies, defaulting to `15s`.
- On timeout, the command fails the tape with the target and pattern in the error message.

### `Wait+Line[@duration] [/regex/|text]`

Same as `Wait`, but explicitly names the current cursor line as the target. This is useful when a
tape wants to make the target clear beside other wait commands.

### `Wait+Screen[@duration] [/regex/|text]`

Waits against the visible viewport text instead of only the cursor line. Use this for command output
that may appear away from the prompt or for TUI assertions where the cursor location is not the
important part.

### `Hide`

Stops appending frames to animated outputs. PTY output still feeds the terminal model, waits still
work, screenshots and state checkpoints still see the current terminal, and hidden commands can
affect final state. `Hide` is for recording visibility, not execution isolation.

### `Show`

Resumes appending frames to animated outputs and immediately captures the current terminal frame.
This avoids a visible gap after hidden setup work and is the usual way to reveal a prepared terminal
state.

### `Caption <text>`

Sets a caption rendered onto later visual media frames. The text is one token, so quote captions
that contain spaces. Use `Caption ""` to clear the active caption.

Captions are presentation metadata only. They do not write to the PTY, alter terminal state, affect
wait matching, or change final output dimensions. Active captions appear on GIF, PNG, MP4, WebM,
frame directory, and `Screenshot` outputs. `State` JSON does not include captions. When a tape
contains a caption, Betamax reserves a bottom presentation row before deriving the terminal grid so
captions do not cover terminal content.

`Caption` does not capture a frame or add time to animated output by itself. The new caption appears
on the next visual frame Betamax renders, such as a frame captured during a later `Sleep`, `Wait`,
typing, key press, `Show`, final-frame output, or `Screenshot`. Add `Sleep` after a caption-only
change when an animation should dwell on the new caption without changing terminal content.

Betamax renders captions below the terminal canvas, left-aligned with the terminal frame edge.
Captions are single-line presentation text: if a caption does not fit beside right-aligned keyboard
chips, Betamax truncates it with `...` instead of wrapping. Caption glyphs are clipped to their
reserved width as a final guard for font fallback and unusually wide characters. If the caption or
keyboard overlay needs more room, increase the tape height or reduce presentation chrome such as
margin and window bar size.

### `Screenshot <path>.png`

Writes an immediate PNG screenshot at that point in the tape. The screenshot uses the same theme and
frame decoration as primary PNG/GIF/video outputs. Only `.png` checkpoint screenshots are supported.

### `State <path>.json`

Writes an immediate structured terminal-state snapshot at that point in the tape. The JSON includes
viewport text, scrollback text, cursor metadata, the default style, a non-default style table, and
compact styled spans. See [State JSON](state-json.md) for the full format.

### `Source <path>.tape`

Parsed for language compatibility but intentionally not executed. Running a tape containing
`Source` currently returns an explicit not-implemented error.

### Key Commands

Key commands send terminal key sequences to the PTY. They may include an optional `@duration`
suffix, which sleeps after each key press, and an optional repeat count token after the key.

Examples:

```text
Enter
Enter@250ms
Down 3
Shift+Tab
Ctrl+C
Alt+F
Ctrl+Alt+Shift+X
F5
```

Supported named keys:

- `Escape`
- `Backspace`
- `Delete`
- `Insert`
- `Down`
- `Enter`
- `Space`
- `Tab`
- `Left`
- `Right`
- `Up`
- `PageUp`
- `PageDown`
- `Home`
- `End`
- `F1` through `F25`
- single-character keys, usually for modified combinations such as `Ctrl+C`

Supported modifiers are `Ctrl`, `Alt`, and `Shift`. Betamax routes key events through
`libghostty-vt`'s key encoder with fallbacks for common terminal control sequences.

## Themes

List available themes:

```sh
cargo run -- themes
cargo run -- themes --json
```

Theme lookup searches user Ghostty theme directories first, then the copied themes in
`crates/betamax-core/resources/ghostty/themes`.
