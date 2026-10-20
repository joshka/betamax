# Tape reference

This reference lists supported tape commands, settings, and defaults. Start with the
[README examples](../README.md#tape-example) to write your first tape. The
[feature field guide](feature-gallery.md) shows rendered examples and test coverage.

## Outputs

- `Output <path>.gif`
- `Output <path>.webp`
- `Output <path>.png`
- `Output <path>.mp4`
- `Output <path>.webm`
- `Output <path>.json`
- `Output <dir>`
- `Screenshot <path>.png`
- `Screenshot <path>.webp`
- `State <path>.json`

GIF, WebP, PNG, screenshot, state JSON, and PNG sequence output are produced in process. MP4 and
WebM output require `ffmpeg` on `PATH`; Betamax reports that as a runtime error only when a video
output is requested. Install it with `brew install ffmpeg` on macOS or `sudo apt-get update && sudo
apt-get install ffmpeg` on Debian/Ubuntu.

An extensionless `Output` path is treated as a directory for numbered PNG frames.
`Caption <text>` commands render presentation text onto visual outputs, but do not create files.

## Settings

Defaults below describe `Default`, which currently selects `Framed`. See [Presets](#presets) for alternatives.

| Setting                   | Value                                                  | Default          | Notes                                       |
| ------------------------- | ------------------------------------------------------ | ---------------- | ------------------------------------------- |
| `Shell`                   | string                                                 | `$SHELL` or `sh` | Shell argv is split with shell-like rules.  |
| `Theme`                   | theme name or inline JSON string                       | `Aardvark Ink`   | Ghostty or VHS name, any case.              |
| `FontFamily`              | string                                                 | `JetBrains Mono` | Falls back through the font system.         |
| `FontSize`                | number                                                 | `22`             | Pixels.                                     |
| `LetterSpacing`           | number                                                 | `0`              | Additional pixels between cells.            |
| `LineHeight`              | number                                                 | `1`              | Multiplier applied to font size.            |
| `Width`                   | number                                                 | `1200`           | Final output width in pixels.               |
| `Height`                  | number                                                 | `720`            | Final output height in pixels.              |
| `Columns`                 | number                                                 | from `Width`     | Terminal columns; sizes the output width.   |
| `Rows`                    | number                                                 | from `Height`    | Terminal rows; sizes the output height.     |
| `Padding`                 | number                                                 | `0`              | Inner padding before terminal cells.        |
| `Framerate`               | number                                                 | `50`             | Capture cadence in frames per second.       |
| `TypingSpeed`             | duration                                               | `50ms`           | Default delay between typed characters.     |
| `PlaybackSpeed`           | number                                                 | `1.0`            | Changes output playback, not PTY timing.    |
| `LoopOffset`              | number                                                 | `0.0`            | `0..=1` is a fraction; otherwise seconds.   |
| `CursorBlink`             | bool                                                   | `true`           | Simulates a half-second blink cadence.      |
| KeyboardOverlay           | `Off`, `Keys`, `Input`, or `All`                       | `Off`            | Shows recent input chips in media.          |
| KeyboardOverlayLocation   | `CaptionRow`, `TopLeft`, `TopRight`, etc.              | `CaptionRow`     | Places keyboard overlay chips.              |
| `WaitTimeout`             | duration or number                                     | `15s`            | Number values are seconds.                  |
| `WaitPattern`             | regex string                                           | `>$`             | Used by `Wait` without explicit pattern.    |
| `Margin`                  | number                                                 | `32`             | Outer decoration margin in pixels.          |
| `MarginFill`              | string                                                 | theme gradient   | Color, gradient, image, or `transparent`.   |
| `WindowBar`               | `Rings`, `RingsRight`, `Colorful`, or etc.             | `Colorful`       | Left dots; any non-empty value enables bar. |
| `WindowBarSize`           | number                                                 | `30`             | Bar height in pixels when enabled.          |
| `WindowBarColor`          | `#rrggbb` string                                       | theme surface    | Invalid colors fall back to theme bg.       |
| `WindowTitle`             | string                                                 | empty / disabled | Initial centered window bar title.          |
| `WindowTitleFromTerminal` | bool                                                   | `true`           | Let OSC 0/2 update the window title.        |
| `WindowTitleColor`        | `auto` or `#rrggbb` string                             | auto             | Invalid colors use automatic contrast.      |
| `BorderRadius`            | number                                                 | `11`             | Rounded-corner mask radius in pixels.       |
| `WindowBorder`            | number                                                 | `1`              | Shell outline width; zero disables.         |
| `WindowBorderColor`       | string                                                 | `auto`           | Auto contrast or explicit `#rrggbb`.        |
| `WindowBorderOpacity`     | number                                                 | `0.15`           | Outline opacity, clamped to 0–1.            |
| `Preset`                  | `Default`, `Framed`, `Bare`, `Inset`, or `Default-0.1` | `Default`        | Baseline for appearance settings.           |
| `Shadow`                  | bool                                                   | `true`           | Outer window shadow.                        |
| `ShadowBlur`              | number                                                 | `20`             | CSS blur radius in pixels.                  |
| `ShadowOffset`            | number                                                 | `6`              | Vertical shadow offset in pixels.           |
| `ShadowOpacity`           | number                                                 | `0.55`           | Shadow opacity, clamped to 0–1.             |

Unknown settings and type mismatches are errors. For example, `Set Wdith 900` and
`Set Width "wide"` fail before the shell starts instead of silently falling back to defaults.

The terminal grid is derived after all settings are applied. Margin, window-bar decoration, and any
presentation row needed for captions or `KeyboardOverlayLocation CaptionRow` are subtracted from
width and height first, then padding, font size, letter spacing, and line height determine the PTY
columns and rows. Positive terminal dimensions fit at least one row and column. Settings that
leave no terminal canvas are errors; increase dimensions or select `Bare` for a small image.

Font size, margins, and frame decoration affect the space available to the application. When testing
modal placement, centering, wrapping, or split panes, start with the default `FontSize` and
`Margin`.
Increase `Width` or `Height` if the application needs more space.

### Sizing in cells

`Set Columns` and `Set Rows` ask for an exact terminal grid, like VHS. Betamax grows the output to
fit it: the cells, then padding, margin, window bar, and any caption row, rounded up to an even
pixel count for video encoders. Font size, letter spacing, and line height change the output size,
not the grid.

```tape
Set Columns 80
Set Rows 24
```

Each axis is sized one way. Set only `Rows` and the width still comes from `Width`, the preset, or
the default. Setting `Width` and `Columns`, or `Height` and `Rows`, is an error. A preset's size is
a default, so cells replace it without an error. Both values are whole numbers in `1..=65535` and,
like other settings, must appear before runtime commands. Each captured run prints the resolved
grid and output size, for example `terminal 80x24 cells, output 1104x622 px`.

`Set Columns` and `Set Rows` choose the starting grid and size the output around it.
[`Resize`](#resize-columns-rows) changes the grid mid-tape and keeps the output size fixed: a
smaller grid leaves empty canvas, and a larger one is clipped. Use `Set` to size the recording,
and `Resize` to test how an application reacts when its terminal changes size.

`Set KeyboardOverlay Input` draws input labels on generated PNG, GIF, video,
screenshot, and frame-sequence media. Labels appear when input is queued and linger briefly after
the input is typed, so review GIFs show the action near the terminal change it caused. The overlay
is presentation-only: it does not change PTY input bytes, waits, state JSON, or final output
dimensions. `KeyboardOverlayLocation CaptionRow` reserves a bottom presentation row before deriving
the terminal grid so chips do not cover terminal content. Corner locations such as `BottomRight`
draw chips inside the terminal canvas with a small inset from the terminal edge. Caption-row
overlays are inset by `Padding` when the tape has no margin, or by half of `BorderRadius` from the
terminal frame edge when it has one; see [`Caption`](#caption-text) for details and workarounds.

Keyboard overlay modes are:

- `Off`: no overlay.
- `Keys`: explicit key commands only, such as `Ctrl+P`, `Down`, `Enter`, and `Escape`.
- `Input`: key commands plus short typed input that reads like user intent.
- `All`: every visible input event, including long `Type` commands summarized by character count.

For built-in shell names such as `bash`, `zsh`, and `fish`, Betamax starts a clean recording shell
with startup files and history disabled where possible. Those shells use the VHS-style colored `>`
prompt by default, so captures do not inherit prompts such as `bash-5.3$` from the host
environment.

### Presets

An unconfigured tape uses `Default`, which currently selects `Framed`. `Set Preset Default`
selects the current default explicitly; `Set Preset Framed` selects that named look directly.
Future default selection can change without redefining the named Framed preset.
Preset names are case-insensitive. When repeated, the last `Set Preset` selects the baseline.
Ordinary `Set` values override it regardless of their position relative to `Set Preset`; repeated
ordinary settings retain their existing last-value-wins behavior.

```tape
Set FontSize 26
Set Preset Framed
Set Theme "GitHub Light Default"
Set Shadow false
```

This uses the framed baseline, 26px text, a light theme, and no shadow. Explicit colors survive
preset and theme ordering. `Set WindowBar ""`, `Set WindowBorder 0`, and `Set Shadow false` disable
those effects. To remove all decoration together, select `Bare`.

| Setting                               | Framed               | Bare                  | Inset                 | Default-0.1           |
| ------------------------------------- | -------------------- | --------------------- | --------------------- | --------------------- |
| Width × Height                        | 1200×720             | 1200×720              | 1200×720              | 1200×600              |
| Theme                                 | Aardvark Ink         | Aardvark Ink          | Aardvark Ink          | Aardvark Blue         |
| FontSize / LetterSpacing / LineHeight | 22 / 0 / 1           | 22 / 0 / 1            | 22 / 0 / 1            | 22 / 1 / 1            |
| FontFamily                            | JetBrains Mono       | JetBrains Mono        | JetBrains Mono        | JetBrains Mono        |
| Padding                               | 0                    | 0                     | 0                     | 60                    |
| Margin                                | 32                   | 0                     | 16                    | 0                     |
| MarginFill                            | theme gradient       | theme background      | theme surface         | theme background      |
| WindowBar / WindowBarSize             | Colorful / 30        | disabled / 30         | disabled / 30         | disabled / 30         |
| WindowBarColor                        | theme surface        | theme background      | theme surface         | theme background      |
| BorderRadius / WindowBorder           | 11 / 1               | 0 / 0                 | 11 / 1                | 0 / 0                 |
| WindowBorderColor / Opacity           | auto / 0.15          | auto / 0.15           | auto / 0.15           | auto / 0.15           |
| Shadow / Blur / Offset / Opacity      | true / 20 / 6 / 0.55 | false / 28 / 9 / 0.55 | false / 28 / 9 / 0.55 | false / 28 / 9 / 0.55 |
| WindowTitleFromTerminal               | true                 | true                  | true                  | false                 |

All presets start with an empty static title and automatic title contrast. Timing, shell, cursor,
keyboard overlays, captions, and output format are independent of preset.

`Framed` provides a window bar, rounded shell, subtle outline, gradient, and shadow for README
media. `Bare` provides undecorated terminal content. `Inset` provides a compact rounded panel on a
solid backdrop, without a bar or shadow. Run
`examples/presets-{default,framed,bare,inset,default-0.1}.tape` to render the same terminal scene
with each baseline.

The framed backdrop is a gradient in the theme background's own hue, darker than the window. Its
top is slightly darker than the background with a little more chroma; its bottom is darker still,
at 70% of the background's OKLCH lightness for dark themes and 0.10 below it for light themes.
Neutral backgrounds stay neutral. The bar and inset backdrop use the
background mixed with 25% black for dark themes and 4% black for light themes. These defaults
follow the final selected theme. An explicit `MarginFill` or `WindowBarColor` overrides the
derived surface.

At a displayed width of 800px, the 1200px source's 22px text appears approximately 16px tall.
Choose a smaller font or larger canvas for denser applications; output scaling is independent.

For tapes migrating from Betamax 0.1 to the planned 0.2 defaults, add `Set Preset Default-0.1` in
the preamble. This restores the pre-preset 0.1 sizing, typography, theme, and disabled effects. It
does not promise pixel-identical VHS rendering. `Bare` retains the new typography and sizing
instead. `Framed`, `Bare`, and `Inset` are visual baselines whose values may evolve. `Default-0.1`
retains the pre-preset 0.1 default settings; renderer fixes may still change pixels. Set individual
values when exact settings matter.

### Window border

`Set WindowBorder 1` enables a subtle inset outline around the terminal and window bar together.
It follows the continuous corners of `BorderRadius`, without a divider between the bar and terminal.
The Framed and Inset width is one pixel; Bare and Default-0.1 disable the outline. Fractional widths
truncate to integer pixels. Negative or non-finite widths and non-finite opacity values are errors.
Opacity is clamped to 0–1; zero leaves rendering unchanged.

`WindowBorderColor "auto"` (the default) selects white or black for contrast with the final theme
background, using linear-sRGB relative luminance with a 0.179 threshold. Explicit `#rrggbb` colors
survive subsequent theme changes. Invalid color strings are errors; `auto` resets an override.

The shell expands outward into the margin until its border and rounded corners surround the complete
terminal cell rectangle. They never paint over or clip cells, including with `Padding 0`. If the
margin cannot hold the shell, startup reports an error: increase `Margin` or reduce `BorderRadius`
or `WindowBorder`. Output dimensions, cell placement, PTY rows/columns, raw frames, and state JSON
do not change when border width or radius changes. Captions and keyboard overlays draw afterward.
Rounded edges blend against the backdrop at the same coordinates, including a shadow when available.
Output remains opaque.

### Margin fill

`MarginFill` accepts a single color, Betamax's `linear(...)` gradient syntax, or an
[image](#image-margin). A color is
`#rrggbb`, `#rrggbbaa` with alpha last as in CSS, or `transparent`:

```tape
Set Margin 44
Set MarginFill "linear(#232640, #0f101a)"
Set MarginFill "linear(to: bottom-right, #232640, #0f101a)"
Set MarginFill "linear(to: left, #232640, #0f101a)"
Set MarginFill "linear(angle: 135deg, #232640, #0f101a)"
Set MarginFill "linear(angle: 135deg, #4d5b9e 0%, #232640 42%, #0f101a 100%)"
```

`linear(...)` takes an optional direction followed by at least two comma-separated color stops.
Without a direction, it runs from top to bottom. A `to:` direction points from the first stop to
the last, and accepts `top`, `top-right`, `right`, `bottom-right`, `bottom`, `bottom-left`, `left`,
or `top-left`. `angle: <number>deg` is measured clockwise from upward: `90deg` is left to right and
`135deg` is top-left to bottom-right. A stop is a color, optionally followed by `<position>%`.
When positions are omitted, they are distributed evenly; when one stop has a position, every stop
must have one, in strictly ascending order.

This is Betamax syntax, not a CSS parser: it accepts hex colors, `transparent`, `to:` directions, angles,
and linear stops only. Colors interpolate in sRGB, rounding each channel with a small, fixed ordered
dither to soften banding in GIFs. The gradient spans the entire output, including rounded window
corners and the caption row. It does not change the terminal's background or grid.

With no explicit `MarginFill`, the backdrop follows the selected preset and final theme. An explicit
color or gradient is preserved regardless of `Set Theme` ordering. `Set MarginFill ""` disables the
margin. Invalid colors and malformed gradients fall back to the active theme background. `betamax
validate` checks tape syntax and command order, so it does not reject malformed fill strings. Radial
gradients are not supported. See `examples/margin-gradient.tape` for a complete
example.

#### Transparent margin

`Set MarginFill transparent` leaves the backdrop clear so the output sits on whatever page shows it.
Alpha colors tint the page or fade a gradient to clear:

```tape
Set Preset Framed
Set MarginFill transparent
Set MarginFill "#0f101a80"
Set MarginFill "linear(to: bottom, #4d5b9e, transparent)"
```

Stops with alpha blend premultiplied, so a fade to `transparent` keeps its hue as it clears rather
than darkening. With `transparent`, the margin, caption row, and the area outside rounded corners
are fully transparent. The shadow
becomes translucent black, so it darkens any page. The window, bar, border, and terminal stay opaque.
Captions and keyboard labels keep their theme colors, so check them against the page they sit on.

| Output             | Transparency                                                          |
| ------------------ | --------------------------------------------------------------------- |
| PNG, `Screenshot`  | Full alpha                                                            |
| PNG sequence, WebP | Full alpha                                                            |
| WebM               | Full alpha (VP9 `yuva420p`)                                           |
| GIF                | One bit: partly transparent pixels, including the shadow, are dropped |
| MP4                | Rejected before the shell starts; H.264 has no alpha                  |

Use PNG or WebP to keep soft shadows and antialiased corners. See
`examples/margin-transparent.tape`.

#### Image margin

`MarginFill` also takes a picture. A bare path works as in VHS and stretches the image to the
output size. `image(<path>)` keeps the aspect ratio and takes an optional fit:

```tape
Set MarginFill "backdrop.png"
Set MarginFill "image(backdrop.png)"
Set MarginFill "image(backdrop.png, contain)"
```

| Fit       | Behavior                                                                      |
| --------- | ----------------------------------------------------------------------------- |
| `cover`   | Default. Scale to fill the output; crop the overflow equally from both sides. |
| `contain` | Scale to fit inside the output, centered; the theme background fills the gap. |
| `tile`    | Repeat at native size from the top-left corner.                               |
| `stretch` | Scale each axis to the output size, as a bare path does.                      |

Any value that is not empty, a `#` color, `linear(...)`, or `transparent` is an image path.
Relative paths resolve against the working directory first, as in VHS, then against the tape's
directory, so a tape and its picture can move together. `~/` expands to the home directory. PNG,
JPEG, and WebP files are supported, detected from their contents; each side may be up to 16384
pixels. The image is decoded and resampled with Lanczos3 once per run.

Unlike invalid colors, image problems stop the run before the shell starts: a missing file names
every place it looked, an undecodable file reports the decoder error, and a malformed `image(...)`
lists the accepted forms. `betamax validate` does not open images.

The image covers the same area as a gradient, including the caption row and the area outside
rounded corners. The shadow, captions, and keyboard chips draw over it unchanged, so prefer calm,
low-detail pictures behind text. A picture with alpha keeps it and follows the transparency rules
above. GIF output shares one 256-color palette between the picture and the terminal, so detailed
photos band; use MP4, WebM, or WebP for those. See `examples/margin-image.tape`.

### Window shadow

`Shadow` draws a black outer shadow behind the rounded terminal window. `ShadowBlur` is the CSS blur
radius, so its standard deviation is half the value; Framed uses `20` (sigma `10`); the other styles
retain the inactive value `28` (sigma `14`). `ShadowOffset` is a signed vertical displacement, and
`ShadowOpacity` is clamped to `0..1`.

```tape
Set Margin 44
Set Shadow true
Set ShadowBlur 28
Set ShadowOffset 9
Set ShadowOpacity 0.55
```

The shadow is clipped to the output frame. `Margin 0` or `MarginFill ""` makes it a no-op, as does
zero opacity. Allow a margin of at least about `ShadowBlur + abs(ShadowOffset)` to avoid obvious
clipping. Also allow room for shell expansion around cells; use `Margin 44` for extra room. Increase
the margin further if the faint tail is still visible at the edge. The shadow shows best on a
gradient or on a backdrop lighter than near-black.

### Window corners

`BorderRadius` uses continuous corners: the curve eases into the straight window edges instead of
joining them with a circular arc. The same outline shapes the window mask, inset border, and shadow.
The radius clamps to fit small windows; zero keeps square corners. Corner coverage and antialiased
circular window buttons are prepared once per run, without resizing terminal pixels or the output.

### Window bar color

`Set WindowBarColor "#0f101a"` fills the window bar with a `#rrggbb` color. The default is the
preset-derived theme surface. An explicit color takes precedence regardless of whether `Set Theme`
appears before or after it. Invalid colors, including an empty string, fall back to the active theme
background. The setting has no effect when `WindowBar` is empty.

Only the bar changes; terminal colors stay in the theme. With `BorderRadius`, the window's top
corners are rounded and the boundary between the bar and terminal remains straight.

```tape
Set WindowBar RingsRight
Set WindowBarColor "#0f101a"
Set Theme '{"name":"Night","background":"#151723"}'
Set BorderRadius 11
```

### Window title

```tape
Set WindowBar Colorful
Set WindowTitle "jk — log"
Set WindowTitleColor auto
```

`WindowTitle` defaults to an empty string and is ignored when `WindowBar` is empty. The
single-line title is centered across the whole bar, with equal space reserved at both ends to avoid
buttons in left and `Right` modes, with a gap of two button radii. Long titles truncate with `…`;
extremely narrow bars clip the ellipsis or omit the title when no space remains. Line breaks become
spaces.

The title uses `FontFamily` at 40% of `WindowBarSize` (12 pixels for the default 30-pixel bar).
`WindowTitleColor` defaults to `auto`; omitted, empty, or invalid values also use automatic color.
Automatic color starts with 65% theme foreground and 35% effective window-bar color, mixing sRGB
channels. It increases the foreground share in one-percent steps until the text color reaches at
least 4.5:1 contrast against the bar. If the theme foreground cannot reach that contrast, it chooses
black or white, whichever contrasts more. This uses an explicit `WindowBarColor` when configured.
The ratio describes the text color before glyph antialiasing.

An explicit `#rrggbb` title color bypasses automatic contrast adjustment and survives `Set Theme`
in either order, even when its contrast is low. Titles affect decorated media only; they do not
change terminal state.

#### Following app titles

`Set WindowTitleFromTerminal true` lets the recorded program change the bar with OSC 0 or OSC 2. The
default is `true` (`false` with `Preset Default-0.1`). Set it to `false` to keep a static title. Custom
capture backends follow app titles by default only when they report title-event support; an explicit
`true` still requires that capability. For example, the app can write `printf '\033]2;title set by
app\007'`. BEL and ST terminators are supported by Ghostty.

```tape
Set WindowBar Colorful
Set WindowTitle "title set by tape"
Set WindowTitleFromTerminal true

Type "printf '\033]2;title set by app\007'"
Enter
Sleep 1s
WindowTitle "title reset by tape"
Sleep 1s
```

The latest assignment wins: app titles replace the initial tape title, `WindowTitle "text"`
replaces an app title, and a later OSC can replace it again. Shell startup and prompt OSC titles
count too. An empty title explicitly clears the bar; it does not restore the initial value.
Repeated identical assignments do not create a transition. The title remains hidden with no bar.

Every visible title transition retains its corresponding terminal frame, including several titles
in one output burst. Titles participate in frame coalescing. Before encoding, a transition gets at
least one output video frame and 10ms of hold time if its natural hold is shorter. Very fast bursts
can therefore extend playback. Hidden updates change the current title without replaying hidden
history on `Show`. Screenshots and final stills use the current title. Title patches are cached
with a bounded 32-entry cache for the run.

State JSON always reports the app's raw terminal title, even with following disabled. Tape display
assignments do not change that state. Custom capture backends must implement title observation to
support the opt-in; an unsupported backend returns an error instead of silently losing events.

## Commands

### `Output <path>`

Adds a primary output for the tape. The runner classifies the output by extension after parsing and
before starting the shell.

Supported output paths:

- `.gif`: animated GIF written in process.
- `.webp`: lossless animated WebP written in process, with millisecond frame holds.
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

### `WindowTitle <text>`

Change the displayed window title immediately in tape order. Unlike the startup-only
`Set WindowTitle`, this command can appear between runtime commands. It accepts one text argument,
including an empty string to clear the bar; use quotes for spaces. It does not accept `@duration`:
use `Sleep` for a deliberate hold. Pending PTY output is consumed before the assignment without
waiting for quiet. If that queue exceeds the bounded drain budget, synchronize with the app first.
See [Following app titles](#following-app-titles) for precedence and output timing.

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

In tests, use sleeps only when there is no prompt, status line, or other text to wait for. In demos,
add sleeps after waits to give viewers time to read the output.

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

### Mouse commands

Mouse coordinates are zero-based terminal cells: column `0`, row `0` is the top-left cell.
They exclude font padding, margins, window bars, and captions. The pointer starts at `(0, 0)`.

```tape
MouseMove 42 8
MouseDown Left
MouseMove 46 8
MouseUp Left
Shift+MouseScroll Down 3
```

`MouseMove <column> <row>` moves the pointer; moving with a held button produces a drag.
`MouseDown <button>` and `MouseUp <button>` accept `Left`, `Middle`, or `Right`.
`MouseScroll <direction> <count>` sends a positive number of wheel ticks at the pointer, with
`Up`, `Down`, `Left`, or `Right` describing wheel direction. These commands accept `Ctrl+`,
`Alt+`, and `Shift+` prefixes in any order. Modifiers apply to that action only.

The application controls mouse reporting. Betamax uses Ghostty's encoder to respect disabled,
press/release, button-motion, and all-motion reporting and the negotiated X10, UTF-8, SGR, urxvt,
or SGR-pixel format. Legacy protocols can suppress coordinates they cannot represent. SGR-pixel
reports use the center of the requested cell with the current cell metrics. Wheel commands emit
press reports without changing held-button state. Multiple held buttons are supported; motion
reports the most recently pressed button still held.

Out-of-bounds coordinates, pressing an already-held button, and releasing a button that is not held
fail the tape. With reporting disabled, input produces no PTY bytes while pointer/button state
still updates. Betamax does not emulate a terminal window's local selection; use `ScrollUp` and
`ScrollDown` to move the viewport through scrollback.

Use an output wait after each action to synchronize with application redraw. Mouse commands do not
accept `@duration`; use `Sleep` separately for recording pacing. Mouse input requires a terminal
backend even when the tape requests no media. Custom `TerminalSession` implementations can opt in
by implementing `mouse_input`; its default reports unsupported input when a mouse command runs.

### `ScrollUp[@duration] [count]` and `ScrollDown[@duration] [count]`

Move the terminal viewport through scrollback, one row at a time, without sending input to the
program. `count` defaults to `1`, and `@duration` sets the pause after each row; the frame interval
applies when it is absent. These match VHS's commands of the same name.

```tape
ScrollUp 10
ScrollDown@50ms 4
```

Scrolling stops at the oldest scrollback row and at the live screen. While the viewport is scrolled
back, new output does not pull it down; `Type`, key commands, and `Paste` return it to the bottom
before sending input, as Ghostty and VHS do. The alternate screen has no scrollback, so these
commands do nothing there. `Wait+Screen`, `AssertText`, screenshots, and the `viewport` fields in
state JSON all read the rows currently shown.

Use `MouseScroll` instead when the program should handle scrolling, such as a TUI with its own
scroll region.

### `Resize <columns> <rows>`

Resize the live terminal grid and PTY in cells. Both dimensions must be integers in `1..=65535`.
The child receives its normal resize notification; follow the command with an output wait to
verify redraw rather than assuming a timing delay is sufficient.

```tape
Resize 50 16
Wait+Screen@5s "narrow layout"
State target/narrow.json
```

The media canvas, font metrics, decorations, and padding stay fixed for the entire tape. To size
the recording in cells, use [`Set Columns` and `Set Rows`](#sizing-in-cells) instead. The live
grid starts at the same top-left origin as the original grid. Shrinking leaves unused canvas space
in the terminal background color; growing beyond the canvas clips the rendered grid. State JSON
and subsequent mouse bounds use the live grid even for clipped cells. The pointer clamps to the
last valid column and row after shrinking; held buttons remain held.

The terminal backend updates before the PTY notification, so child redraw goes into the new grid.
Either update failing aborts the tape; commands do not continue with inconsistent dimensions.
Optional kernel pixel dimensions use grid size times cell metrics, or zero (unspecified) if they
exceed the PTY's 16-bit pixel fields. The PTY control handle remains alive for the session and is
released with its reader, writer, and child handles. A custom `TerminalSession` can implement
`resize`; the default returns an unsupported-resize error when this command runs.

### Checkpoint assertions

Assertions check the visible terminal after at least 100 ms of PTY quiet. Settling is bounded by
`WaitTimeout` and fails if output stays busy. Use `Wait+Screen` to synchronize with an observable
application result first; assertions check the settled checkpoint, and do not wait until their
condition becomes true. A transient matching frame cannot satisfy a later assertion.

```tape
Wait+Screen@5s "ready"
AssertText "ready"
AssertCells 4 2 '["1", "5", "0", "0"]'
AssertStyle 4 2 4 '{"fg":"#7ecf91","bold":true}'
AssertAbsent@500ms "error"
AssertState baselines/ready.json
```

- `AssertText <text>` requires a literal visible-screen substring.
- `AssertCells <column> <row> <JSON>` matches an exact horizontal range. Supply one JSON string per
  physical cell, including spaces for blanks and wide-character continuations. For example,
  `'["界", " ", "é"]'` occupies three cells; the combining mark stays with `e`. Coordinates are
  zero-based and exclude all presentation decoration. Ranges outside the live grid fail.
- `AssertStyle <column> <row> <width> <JSON>` checks every cell in the range. `fg` and `bg`, when
  supplied, match resolved `#RRGGBB` colors. Omitted colors are unconstrained. All attributes match
  exactly: omitted booleans mean false and omitted `underline` means `none`. Accepted attributes
  are those in [State JSON](state-json.md); unknown fields, invalid colors, and underline names fail
  validation. The width must be positive.
- `AssertAbsent <text>` checks absence at the settled checkpoint. `AssertAbsent@<duration> <text>`
  additionally observes the screen throughout that positive duration, checking after each PTY
  chunk and at the end. It fails immediately if the substring appears. This samples terminal state;
  text emitted and erased within one PTY chunk is not observable. The observation begins after
  settling, and does not make a claim about later output.
- `AssertState <baseline.json>` compares the captured state with an existing explicit baseline.
  Dimensions, cursor, metadata, text, styles, and compact spans all participate. No fields are
  ignored, including title, working directory, scrollback, and style-table order. The JSON
  representation of omitted underline and explicit `none` is treated as equivalent.
  Capture baselines with `State`, inspect their text/styles and corresponding screenshot, and
  approve changes explicitly. Assertions never create or replace baselines automatically.

Failures identify the command number and expected/actual text, cells, or styles. The runner writes
`actual.json`, `actual.png`, `cells.json` (when the backend provides cells), and `diff.txt` under a
unique `target/betamax-failures/` directory. Set `BETAMAX_FAILURE_DIR` to choose another artifact
root. Wait, input, and other runtime failures retain the same diagnostics. Media stays local or in
CI artifacts; no pixel golden comparison is performed. Custom backends can implement the optional
`viewport_cells` method for cell/style assertions; text and baseline assertions use existing
methods.

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

Betamax renders captions left-aligned below the terminal canvas. Their horizontal inset depends on
whether the tape sets `Margin`:

- Without a margin, the presentation row spans the full image width. Caption text starts `Padding`
  pixels from the left edge, in line with the first terminal column, and caption-row keyboard chips
  keep the same inset from the right edge. With `Padding 0`, the inset is half the caption font
  size so text never touches the image edge.
- With a margin, the presentation row sits below the framed terminal. Captions and chips align with
  the terminal frame edge and ignore `Padding`. Both are inset by half of `BorderRadius`, so their
  edges feel aligned with rounded corners.

To place captions closer to the image edge than the terminal text, add a margin and reduce
`Padding` by the same amount. For example, replacing `Set Padding 20` with `Set Margin 8` and
`Set Padding 12` keeps terminal text and the grid where they were and moves the caption to 8 pixels
from the edge. The margin and presentation row share the `MarginFill` backdrop, so the
terminal gains a thin frame in that color. Lowering `Padding` alone also moves the caption, but
changes the terminal grid size.

Captions are single-line presentation text: if a caption does not fit beside right-aligned keyboard
chips, Betamax truncates it with `...` instead of wrapping. Caption glyphs are clipped to their
reserved width as a final guard for font fallback and unusually wide characters. If the caption or
keyboard overlay needs more room, increase the tape height or reduce presentation chrome such as
margin and window bar size. Keyboard chips reserve caption space using their visible width.

### `Screenshot <path>.png`

Accepts `.png` and `.webp` (case-insensitive). Writes a static screenshot at that point in the tape,
using the same theme and frame decoration as primary visual outputs. WebP screenshots are lossless
and preserve transparency. To write a static final WebP, put `Screenshot final.webp` at the end of
the tape; `Output final.webp` always writes an animation.

### `State <path>.json`

Writes an immediate structured terminal-state snapshot at that point in the tape. The JSON includes
viewport text, scrollback text, cursor metadata, the default style, a non-default style table, and
compact styled spans. See [State JSON](state-json.md) for the full format.

### `Source <path>.tape`

Parsed for language compatibility but intentionally not executed. Running a tape containing
`Source` currently returns an explicit not-implemented error.

### Key commands

Key commands send terminal key sequences to the PTY. They may include an optional `@duration`
suffix, which sleeps after each key press, and an optional repeat count token after the key.

Examples:

```tape
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
`crates/betamax-core/resources/ghostty/themes` and the VHS-only themes in
`crates/betamax-core/resources/vhs/themes`. A name with no exact match falls back to the VHS alias
table (`resources/vhs/aliases.json`, so `tokyonight` loads `TokyoNight`), then to a match that
ignores case, spaces, and punctuation. Unknown names are reported with close suggestions.
