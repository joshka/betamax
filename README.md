# Betamax

Betamax records terminal sessions from [VHS](https://github.com/charmbracelet/vhs)-style tape files
as GIFs, screenshots, videos, and JSON snapshots for tests. The [docs site][docs-site] has the
details; this page points to them.

![Installing Betamax, creating a tape, and rendering it, recorded with Betamax][hero-gif]

## Where to go

| To                                | See                                                                  |
| --------------------------------- | -------------------------------------------------------------------- |
| See what Betamax does             | The [docs site][docs-site] or the [crates.io page][crate]            |
| Install and render a first tape   | [Quick start][quick-start]                                           |
| Write tapes                       | [Writing tapes][tape-files] and the [Tape reference][tape-reference] |
| Test a terminal app               | [Terminal testing][terminal-testing] and [State JSON][state-json]    |
| Render tapes on pull requests     | [GitHub Action][github-action]                                       |
| Move tapes over from VHS          | [Differences from VHS][vhs-differences]                              |
| Embed Betamax in Rust             | [`betamax-core` on docs.rs][core-docs]                               |
| Browse rendered examples          | [Examples][examples]                                                 |
| Ask a question or share a project | [Discussions][discussions] and [Show and tell][show-and-tell]        |
| Get help or report a bug          | [SUPPORT.md](SUPPORT.md) and [Issues][issues]                        |
| Report a vulnerability            | [SECURITY.md](SECURITY.md)                                           |
| Build from source or contribute   | [CONTRIBUTING.md](CONTRIBUTING.md) and [Development][development]    |

## Repository layout

| Path                   | Contents                                                                      |
| ---------------------- | ----------------------------------------------------------------------------- |
| `crates/betamax`       | The `betamax` CLI                                                             |
| `crates/betamax-core`  | Tape parsing, the runner, rendering, media, and state JSON                    |
| `crates/betamax-fonts` | Font data embedded for the default font                                       |
| `examples/`            | Small tapes used as smoke tests and documentation examples                    |
| `site/`                | Source for the [docs site][docs-site]                                         |
| `docs/`                | Contributor and maintainer docs, starting at [docs/README.md](docs/README.md) |
| `scripts/`             | Helpers behind the `mise run` tasks                                           |

For local development, `mise install` sets up the pinned toolchain, including Zig 0.15.2, and
`mise run check` runs the CI checks. Before changing this file or the crate READMEs, see
[READMEs and media](docs/readmes-and-media.md).

Betamax is licensed under MIT or Apache-2.0. Third-party notices are in
[THIRD_PARTY.md](THIRD_PARTY.md).

[core-docs]: https://docs.rs/betamax-core
[crate]: https://crates.io/crates/betamax
[development]: https://www.joshka.net/betamax/reference/development/
[discussions]: https://github.com/joshka/betamax/discussions
[docs-site]: https://www.joshka.net/betamax/
[examples]: https://www.joshka.net/betamax/examples/
[github-action]: https://www.joshka.net/betamax/testing/github-action/
[hero-gif]: https://github.com/joshka/betamax/releases/download/readme-assets/quick-start.gif
[issues]: https://github.com/joshka/betamax/issues
[quick-start]: https://www.joshka.net/betamax/quick-start/
[show-and-tell]: https://github.com/joshka/betamax/discussions/categories/show-and-tell
[state-json]: https://www.joshka.net/betamax/testing/state-json/
[tape-files]: https://www.joshka.net/betamax/authoring/tape-files/
[tape-reference]: https://www.joshka.net/betamax/reference/tape-reference/
[terminal-testing]: https://www.joshka.net/betamax/testing/terminal-testing/
[vhs-differences]: https://www.joshka.net/betamax/reference/vhs-differences/
