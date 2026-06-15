# Betamax

Betamax is a Rust-first terminal capture CLI in the spirit of
[VHS](https://github.com/charmbracelet/vhs). It runs tape files in a real PTY, feeds terminal
output through `libghostty-vt`, rasterizes frames in process with `cosmic-text` and `swash`, and
writes GIFs, screenshots, videos, or structured terminal state.

The goal is VHS-style authoring without a browser, server, xterm.js, or shelling out to a terminal
web stack. Betamax is useful for project demos, CLI documentation, release notes, and snapshot-style
tests for terminal applications.

## Install

Betamax currently supports macOS and Linux. Windows is not supported because the upstream
`libghostty-vt-sys` native build does not support Windows.

```sh
cargo binstall betamax
```

If [cargo-binstall][cargo-binstall] is not installed, install it first:

```sh
cargo install cargo-binstall
```

Source installs are mainly for maintainers. They build Betamax's vendored `libghostty-vt`
dependency, and Ghostty currently requires Zig 0.15.2 rather than newer Zig releases such as 0.16.
In a checkout, use [mise][mise] to get the pinned toolchain and keep Cargo's target directory in the
checkout so the installed binary can find the native library build output:

```sh
mise install
mise run install-local
```

Other toolchain managers can work too, including [Nix][nix] or a manually installed [Zig][zig]
0.15.2 on `PATH`. mise is the documented path because it works for this repository today; PRs that
add tested docs for other approaches are welcome.

MP4 and WebM output require [ffmpeg][ffmpeg] on `PATH`; GIF, PNG, screenshots, and state JSON are
written in process.

```sh
# macOS
brew install ffmpeg

# Debian/Ubuntu
sudo apt-get update
sudo apt-get install ffmpeg
```

## A Tape

Tape files describe the terminal session to run and the artifacts to write:

```text
Output examples/output/basic.gif
Output examples/output/basic.png
Output examples/output/basic.state.json

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
Screenshot examples/output/basic-checkpoint.png
Hide
Type "exit"
Enter
```

Run it with:

```sh
betamax run demo.tape
```

`Hide` and `Show` let setup and teardown happen without appearing in the final animation. Wait
commands can block on time, a line, the whole screen, or a regular expression, so tapes can be
stable even when terminal programs do real work.

## What It Can Write

| Output               | Use case                                                   |
| -------------------- | ---------------------------------------------------------- |
| GIF                  | README demos, release notes, docs pages                    |
| PNG                  | Final-frame screenshots                                    |
| Screenshot command   | Checkpoint screenshots from the middle of a run            |
| MP4 and WebM         | Video assets encoded through `ffmpeg`                      |
| Frame directory      | Individual rendered frames for debugging or custom tooling |
| State JSON           | Viewport, scrollback, text, and style spans for tests      |

## Examples

The repository includes tapes that exercise the core behavior:

| Tape                          | Demonstrates                                   |
| ----------------------------- | ---------------------------------------------- |
| `examples/basic.tape`         | typing, wait, theme, window bar, border radius |
| `examples/hide-show.tape`     | hidden setup and hidden trailing cleanup       |
| `examples/waits.tape`         | line, screen, regex, and default prompt waits  |
| `examples/keys.tape`          | key commands, repeats, editing, and interrupt  |
| `examples/clipboard-env.tape` | `Env`, `Copy`, and `Paste`                     |
| `examples/outputs.tape`       | GIF, PNG, JSON, screenshot, state, frame dir   |
| `examples/scrollback.tape`    | scrollback-inclusive state JSON                |
| `examples/text-styles.tape`   | ANSI styles, truecolor, and styled state spans |
| `examples/layout.tape`        | padding, margin, fill, window bar, radius      |
| `examples/themes.tape`        | copied Ghostty themes and palette mapping      |
| `examples/video.tape`         | GIF, MP4, and WebM from one capture            |

### Basic

![Basic Betamax GIF][basic-gif]

### Hide And Show

![Hide and Show Betamax GIF][hide-show-gif]

### Themes

![Ghostty theme Betamax GIF][themes-gif]

## Themes And Styling

Betamax ships copied Ghostty themes and can also read user Ghostty theme directories. List theme
names with:

```sh
betamax themes
betamax themes --json
```

Tapes can control width, height, font size, padding, margin, border radius, window bar style, fill
color, typing speed, playback speed, and prompt text. The defaults are chosen to feel close to VHS:
a readable terminal size, a visible frame, and a simple `>` prompt unless a tape asks for a real
shell prompt.

## Terminal Testing

Betamax can be used as a terminal test harness, not only as a GIF generator. A tape can run an
interactive program, wait until expected text appears, capture screenshots at important states, and
write structured state JSON that snapshot tests can compare with tools such as `insta`.

State JSON includes:

- viewport text
- scrollback text
- a compact style table
- styled text spans that avoid cell-by-cell verbosity

That makes it possible to test terminal UI behavior as text and style data, while still keeping PNG
or GIF output available for visual review.

## Differences From VHS

Betamax intentionally keeps a smaller architecture than VHS:

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

See [Differences From VHS][vhs-differences] for the full comparison.

## Documentation

- [Documentation Site][docs-site]
- [Tape Reference][tape-reference]
- [Terminal Testing][terminal-testing]
- [State JSON][state-json]
- [Differences From VHS][vhs-differences]
- [Repository README][repo-readme]

[basic-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/basic.gif
[cargo-binstall]: https://github.com/cargo-bins/cargo-binstall
[docs-site]: https://www.joshka.net/betamax/
[ffmpeg]: https://ffmpeg.org/
[hide-show-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/hide-show.gif
[mise]: https://mise.jdx.dev/
[nix]: https://nixos.org/
[repo-readme]: https://github.com/joshka/betamax
[state-json]: https://www.joshka.net/betamax/testing/state-json/
[tape-reference]: https://www.joshka.net/betamax/reference/tape-reference/
[terminal-testing]: https://www.joshka.net/betamax/testing/terminal-testing/
[themes-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/themes.gif
[vhs-differences]: https://www.joshka.net/betamax/reference/vhs-differences/
[zig]: https://ziglang.org/
