# Betamax

Record terminal demos and test terminal apps from a script. Betamax runs
[VHS](https://github.com/charmbracelet/vhs)-style tape files and saves GIFs, PNGs, videos, and
JSON snapshots of the screen.

![Installing Betamax, creating a tape, and rendering it, recorded with Betamax][hero-gif]

- **One binary, no browser.** Betamax drives a real PTY, parses output with Ghostty's terminal
  engine (`libghostty-vt`), and writes GIF, WebP, and PNG itself. No Chromium, ttyd, or xterm.js,
  and no `ffmpeg` unless you want MP4 or WebM.
- **Correct terminal output.** Wide characters, emoji, truecolor, and kitty, sixel, and iTerm2
  inline images.
- **Tests, not just demos.** Wait for text on screen, then snapshot the viewport, scrollback, and
  styles as JSON for CI.
- **Previews on pull requests.** The [GitHub Action][github-action] renders your tapes in CI and
  posts a gallery comment, so reviewers see the change running.

## How it works

A tape lists what to type and what to wait for. Betamax runs it in a real terminal and writes every
`Output` when the tape ends:

```tape
Output demo.gif
Output demo.png

Hide
Type "cargo build --quiet"
Enter
Wait
Show

Type "my-app --demo"
Enter
Wait+Screen "Ready"
Screenshot ready.png
State ready.json
```

- `Hide` and `Show` keep setup out of the recording.
- `Wait` blocks until the prompt returns. `Wait+Screen` blocks until text appears on screen, and the
  tape fails if it doesn't appear in time, so tapes don't depend on guessed sleeps.
- `Screenshot` and `State` capture a checkpoint mid-run: a PNG for people, and JSON with the
  viewport, scrollback, cursor, and styles for snapshot tests.

`betamax new demo.tape` writes a starter tape with every command documented, and
`betamax run demo.tape` renders it. Most VHS tapes run unchanged; see
[Differences from VHS][vhs-differences].

## What it writes

| Output                     | Use it for                                                    |
| -------------------------- | ------------------------------------------------------------- |
| GIF, WebP                  | README demos, release notes, and docs                         |
| PNG, `Screenshot`          | The final frame, or a checkpoint mid-run                      |
| JSON, `State`              | Viewport, scrollback, cursor, and styles for snapshot tests   |
| MP4, WebM                  | Video, encoded with `ffmpeg`                                  |
| Frame directory            | Every rendered frame as a PNG, for custom tooling             |
| `.txt`, `.ascii`           | Plain screen text, for VHS-style golden files                 |

![ANSI colors in the Dracula theme, one of the bundled Ghostty themes][themes-gif]

![Typing a command and waiting for its output][basic-gif]

## Install

```sh
cargo binstall betamax
# or
brew install joshka/tap/betamax
```

`cargo install betamax` also works, but it builds Ghostty's terminal library from source and needs
Zig 0.15.2 (which [mise](https://mise.jdx.dev/) can install), so prefer the prebuilt binaries
above. Betamax runs on macOS and Linux. See [Install][install] for details, including `ffmpeg` for
video output.

## Learn more

- [Quick start][quick-start]: install, write a first tape, and render it
- [Writing tapes][tape-files]: typing, keys, waits, themes, window styling, and captions
- [Terminal testing][terminal-testing] and [State JSON][state-json]: assert on terminal output in CI
- [GitHub Action][github-action]: render tapes on pull requests and post previews
- [Tape reference][tape-reference]: every command and setting
- [Examples][examples]: rendered tapes with their source

To embed Betamax in Rust tests, use [`betamax-core`][betamax-core]. Source, issues, and discussions
are on [GitHub][github].

[basic-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/basic.gif
[betamax-core]: https://crates.io/crates/betamax-core
[examples]: https://www.joshka.net/betamax/examples/
[github]: https://github.com/joshka/betamax
[github-action]: https://www.joshka.net/betamax/testing/github-action/
[hero-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/quick-start.gif
[install]: https://www.joshka.net/betamax/quick-start/#install
[quick-start]: https://www.joshka.net/betamax/quick-start/
[state-json]: https://www.joshka.net/betamax/testing/state-json/
[tape-files]: https://www.joshka.net/betamax/authoring/tape-files/
[tape-reference]: https://www.joshka.net/betamax/reference/tape-reference/
[themes-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/themes.gif
[terminal-testing]: https://www.joshka.net/betamax/testing/terminal-testing/
[vhs-differences]: https://www.joshka.net/betamax/reference/vhs-differences/
