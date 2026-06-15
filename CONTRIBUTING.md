# Contributing

Thanks for working on Betamax. This project is still before `0.1.0`, so contributions should favor
small, reviewable changes that make the CLI more reliable, easier to maintain, or closer to the
documented tape behavior.

## Project Shape

Betamax is a Rust workspace with two crates:

- `crates/betamax`: the CLI crate published as `betamax`.
- `crates/betamax-core`: the reusable terminal runner, parser, renderer, media, and state logic.

The CLI is the primary product. Keep command-line behavior, release installability with
`cargo binstall betamax`, and generated terminal output in mind when changing shared code.

The core crate should avoid CLI-only dependencies such as `clap`. Put argument parsing, terminal
logging setup, and command dispatch in the CLI crate; put tape parsing, running, rendering, and
artifact generation in the core crate.

## Development Setup

Install the repository tools with [mise][mise]:

```sh
mise install
```

The checked-in `.mise.toml` installs [markdownlint-cli2][markdownlint-cli2], Node, [pnpm][pnpm],
and [Zig][zig] 0.15.2. Zig is required because Betamax builds against the vendored
`libghostty-vt-sys` native dependency, and that Ghostty build currently fails with newer Zig
releases such as 0.16. Run repository commands through `mise run ...` so Cargo sees the pinned Zig
version.

Other toolchain managers can work too, including [Nix][nix] or a manually installed Zig 0.15.2 on
`PATH`. mise is the documented path because it works for this repository today. PRs that add tested
setup docs for other approaches are welcome.

Rust itself is managed with [rustup][rustup].

The workspace `rust-version` is a compatibility floor, not a separately tested MSRV lane. It
should move only when Betamax code or dependency requirements need a newer compiler.

Rust formatting uses nightly rustfmt because `rustfmt.toml` enables unstable formatting options:

```sh
rustup toolchain install nightly --component rustfmt
```

List available tasks with:

```sh
mise tasks ls
```

## Before Opening A Pull Request

Run the same broad check used by CI:

```sh
mise run check
```

This runs nightly formatting, stable and beta clippy, tests, doctests, documentation generation,
Markdown linting, and tape validation. For release-oriented changes, run the larger check:

```sh
mise run release-check
```

`mise run release-check` also checks dependency policy, direct dependency freshness, direct minimal
versions, packaging, install smoke testing, and rendering the basic smoke tape.

## Useful Checks

Use narrower commands while iterating:

```sh
mise run fmt
mise run fmt-check
mise run clippy
mise run clippy-beta
mise run test
mise run doc-test
mise run doc
mise run lint-md
mise run validate
mise run smoke
```

Dependency maintenance has dedicated recipes:

```sh
mise run dependency-policy
mise run outdated
mise run minimal-versions
```

If those checks require missing Cargo subcommands, install them with Cargo:

```sh
cargo install cargo-deny cargo-outdated cargo-minimal-versions cargo-hack --locked
```

## Tape Behavior

Tape compatibility is documented in [docs/tape-reference.md](docs/tape-reference.md). When changing
parser or runner behavior, update that reference next to the code change so users can answer these
questions from one place:

- which command or setting is supported;
- what the default value is;
- when the command is allowed;
- what output or side effects it produces;
- which edge cases intentionally error.

VHS differences belong in [docs/vhs-differences.md](docs/vhs-differences.md). Roadmap and future
fidelity work belongs in [docs/roadmap.md](docs/roadmap.md), not in the VHS differences list.

## Documentation Style

Markdown is linted with `markdownlint-cli2` through `mise run lint-md`.

- Wrap prose at 100 columns.
- Use fenced code blocks with a language when the language is known.
- Use `1.` for every numbered-list item.
- Keep Markdown tables aligned.
- Do not check generated GIF, PNG, MP4, or WebM output into the repository.

Rust documentation should explain non-obvious behavior, return values, edge cases, and rationale.
The goal is useful long-term maintenance context, including for private items where the behavior is
not obvious from the name alone.

## Examples And README Assets

Checked-in tapes under `examples/` are small smoke tests and documentation examples. Render them
locally with:

```sh
mise run render-examples
```

Rendered files are written under `examples/output` and copied to `target/betamax-examples`.
Generated media is intentionally not tracked in source control.

README GIFs are hosted as assets on the GitHub Release tag `readme-assets`. Maintainers can refresh
them with:

```sh
mise run upload-readme-assets
```

That command requires the [GitHub CLI][github-cli] and permission to upload release assets.

## Pull Requests

Keep unrelated fixes in separate pull requests. Prefer clear, imperative commit or pull-request
titles such as `Document contributor workflow` or `Fix state JSON style spans`.

Pull requests should explain user-visible behavior, mention relevant docs or examples, and list the
validation commands run. If `mise run check` was not run, note which narrower check was sufficient and
why.

Questions, early ideas, and examples of Betamax in real projects should go to
[Discussions](https://github.com/joshka/betamax/discussions) instead of issues. The
[Show and tell](https://github.com/joshka/betamax/discussions/categories/show-and-tell) category is
for screenshots, rendered tapes, repo links, and short notes about how Betamax helps another
project.

## Release Notes

User-visible changes should update [CHANGELOG.md](CHANGELOG.md). Keep entries short and focused on
behavior: new commands, changed defaults, supported output formats, public API changes, important
bug fixes, and known limitations.

[github-cli]: https://cli.github.com/
[markdownlint-cli2]: https://github.com/DavidAnson/markdownlint-cli2
[mise]: https://mise.jdx.dev/
[nix]: https://nixos.org/
[pnpm]: https://pnpm.io/
[rustup]: https://rustup.rs/
[zig]: https://ziglang.org/
