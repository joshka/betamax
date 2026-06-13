# Betamax

Betamax is a Rust-first terminal capture tool in the spirit of
[VHS](https://github.com/charmbracelet/vhs). It reads tape files, runs commands in a PTY, feeds
terminal output through `libghostty-vt`, rasterizes frames in process with `cosmic-text` and
`swash`, and writes screenshots or animations.

The goal is VHS-style output without the browser/server/xterm.js stack. The current implementation
intentionally does not include `serve`, `record`, `source`, or `publish`.

## Quick Start

Betamax currently supports macOS and Linux. Windows is not supported because the upstream
`libghostty-vt-sys` native build does not support Windows.

Install the CLI from crates.io:

```sh
cargo install betamax --locked
```

For local development or source checkouts, run from the workspace:

```sh
cargo run -- run examples/basic.tape
```

Render all local examples:

```sh
scripts/render-examples.sh
```

MP4 and WebM output require `ffmpeg` on `PATH`:

```sh
# macOS
brew install ffmpeg

# Debian/Ubuntu
sudo apt-get update
sudo apt-get install ffmpeg
```

The script writes files under `examples/output` and copies them to `target/betamax-examples` for
local inspection. The GIF previews below are hosted as GitHub Release assets so they can render on
GitHub and crates.io without storing generated media in the source tree.

## Tape Example

```text
Output examples/output/basic.gif

Set Shell "bash"
Set Theme "Aardvark Blue"
Set FontSize 28
Set Width 900
Set Height 480
Set Margin 12
Set WindowBar Colorful
Set BorderRadius 8

Hide
Type "cargo build --quiet"
Enter
Wait
Show

Type "printf 'hello from betamax\n'"
Enter
Wait+Screen "hello from betamax"
Hide
Type "exit"
Enter
```

`Hide` and `Show` are useful for VHS-style tapes that hide setup or compile work and only reveal the
interesting terminal state. See the [Tape Reference][tape-reference] for the full command
and settings behavior.

## Examples

The checked-in examples are small smoke-test tapes that demonstrate core behavior:

| Tape                             | Demonstrates                                   | Main Output         |
| -------------------------------- | ---------------------------------------------- | ------------------- |
| `examples/basic.tape`            | typing, wait, theme, window bar, border radius | `basic.gif`         |
| `examples/hide-show.tape`        | hidden setup and hidden trailing cleanup       | `hide-show.gif`     |
| `examples/waits.tape`            | line, screen, regex, and default prompt waits  | `waits.gif`         |
| `examples/keys.tape`             | key commands, repeats, editing, and interrupt  | `keys.gif`          |
| `examples/clipboard-env.tape`    | `Env`, `Copy`, and `Paste`                     | `clipboard-env.gif` |
| `examples/outputs.tape`          | GIF, PNG, JSON, screenshot, state, frame dir   | `outputs.*`         |
| `examples/scrollback.tape`       | scrollback-inclusive state JSON                | `scrollback.*`      |
| `examples/text-styles.tape`      | ANSI styles, truecolor, and styled state spans | `text-styles.*`     |
| `examples/layout.tape`           | padding, margin, fill, window bar, radius      | `layout.gif`        |
| `examples/themes.tape`           | copied Ghostty themes and palette mapping      | `themes.gif`        |
| `examples/screenshot.tape`       | screenshots and terminal state JSON            | `screenshot.png`    |
| `examples/video.tape`            | GIF, MP4, and WebM from one capture            | `video.*`           |

### Basic

![Basic Betamax GIF][basic-gif]

### Hide And Show

![Hide and Show Betamax GIF][hide-show-gif]

### Themes

![Ghostty theme Betamax GIF][themes-gif]

### Video

The video tape writes GIF, MP4, and WebM from the same captured frames. The GIF preview is shown
here; the video files are generated next to it.

![Video output Betamax GIF][video-gif]

List available themes:

```sh
cargo run -- themes
cargo run -- themes --json
```

Theme lookup searches user Ghostty theme directories first, then the copied themes in
`crates/betamax-core/resources/ghostty/themes`.

## Differences From VHS

Betamax aims for the common VHS authoring flow, but the architecture is intentionally smaller and
Ghostty-first. See [Differences From VHS][vhs-differences] for the full comparison and
remaining parity notes.

| Area              | Betamax status                                       |
| ----------------- | ---------------------------------------------------- |
| Architecture      | PTY plus `libghostty-vt`, no browser/server/xterm.js |
| GIF/PNG/state     | In process                                           |
| MP4/WebM          | Supported through `ffmpeg`                           |
| Themes            | Copied Ghostty themes plus inline JSON themes        |
| Window styling    | Rust composition                                     |
| Testing snapshots | JSON state with viewport, scrollback, and styles     |
| `Source`          | Parsed but intentionally not executed                |
| `record`/`serve`  | Intentionally not implemented                        |
| `publish`         | Intentionally not implemented                        |

The main tradeoff today is video output: MP4 and WebM use `ffmpeg` as a focused encoder bridge.
Terminal execution and rasterization stay in process; video container encoding does not.

## Terminal Testing

Betamax can also be used as a terminal testing harness. A tape can run an interactive terminal
program, wait for text on the screen, capture PNG screenshots at checkpoints, write structured
terminal state, and fail when expected terminal output does not appear before a timeout.

See [Terminal Testing][terminal-testing] and [State JSON][state-json] for the test
workflow and state snapshot format.

## Documentation

- [Documentation Site][docs-site]
- [Tape Reference][tape-reference]
- [Terminal Testing][terminal-testing]
- [State JSON][state-json]
- [Differences From VHS][vhs-differences]
- [Contributing](CONTRIBUTING.md)
- [Development](docs/development.md)
- [Security Policy](SECURITY.md)
- [Support](SUPPORT.md)

The Starlight docs site lives under `site/` and can be run locally with:

```sh
pnpm install
just docs-site-dev
```

[basic-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/basic.gif
[docs-site]: https://www.joshka.net/betamax/
[hide-show-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/hide-show.gif
[state-json]: https://www.joshka.net/betamax/testing/state-json/
[tape-reference]: https://www.joshka.net/betamax/reference/tape-reference/
[terminal-testing]: https://www.joshka.net/betamax/testing/terminal-testing/
[themes-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/themes.gif
[video-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/video.gif
[vhs-differences]: https://www.joshka.net/betamax/reference/vhs-differences/
