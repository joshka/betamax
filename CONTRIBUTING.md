# Contributing

Thanks for working on Betamax. This project is still before `0.1.0`, so contributions should favor
small, reviewable changes that make the CLI more reliable, easier to maintain, or closer to the
documented tape behavior.

## Project Shape

Betamax is a Rust workspace with two crates:

- `crates/betamax`: the CLI crate published as `betamax`.
- `crates/betamax-core`: the reusable terminal runner, parser, renderer, media, and state logic.

The CLI is the primary product. Keep command-line behavior, installability with
`cargo install betamax --locked`, and generated terminal output in mind when changing shared code.

The core crate should avoid CLI-only dependencies such as `clap`. Put argument parsing, terminal
logging setup, and command dispatch in the CLI crate; put tape parsing, running, rendering, and
artifact generation in the core crate.

## Development Setup

Install the repository tools with `mise`:

```sh
mise install
```

The checked-in `.mise.toml` installs `just`, `markdownlint-cli2`, Node, and Zig. Zig is present for
native dependency work around Ghostty tooling. Rust itself is managed with `rustup`.

The workspace `rust-version` is a compatibility floor, not a separately tested MSRV lane. It
should move only when Betamax code or dependency requirements need a newer compiler.

Rust formatting uses nightly rustfmt because `rustfmt.toml` enables unstable formatting options:

```sh
rustup toolchain install nightly --component rustfmt
```

The local task runner is `just`:

```sh
just --list
```

## Before Opening A Pull Request

Run the same broad check used by CI:

```sh
just check
```

This runs nightly formatting, stable and beta clippy, tests, doctests, documentation generation,
Markdown linting, and tape validation. For release-oriented changes, run the larger check:

```sh
just release-check
```

`just release-check` also checks dependency policy, direct dependency freshness, direct minimal
versions, packaging, install smoke testing, and rendering the basic smoke tape.

## Useful Checks

Use narrower commands while iterating:

```sh
just fmt
just fmt-check
just clippy
just clippy-beta
just test
just doc-test
just doc
just lint-md
just validate
just smoke
```

Dependency maintenance has dedicated recipes:

```sh
just dependency-policy
just outdated
just minimal-versions
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

Markdown is linted with `markdownlint-cli2` through `just lint-md`.

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
just render-examples
```

Rendered files are written under `examples/output` and copied to `target/betamax-examples`.
Generated media is intentionally not tracked in source control.

README GIFs are hosted as assets on the GitHub Release tag `readme-assets`. Maintainers can refresh
them with:

```sh
just upload-readme-assets
```

That command requires the GitHub CLI and permission to upload release assets.

## Pull Requests

Keep unrelated fixes in separate pull requests. Prefer clear, imperative commit or pull-request
titles such as `Document contributor workflow` or `Fix state JSON style spans`.

Pull requests should explain user-visible behavior, mention relevant docs or examples, and list the
validation commands run. If `just check` was not run, note which narrower check was sufficient and
why.

## Release Notes

User-visible changes should update [CHANGELOG.md](CHANGELOG.md). Keep entries short and focused on
behavior: new commands, changed defaults, supported output formats, public API changes, important
bug fixes, and known limitations.
