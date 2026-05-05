# Repository Guidelines

## Project Structure & Module Organization

Betamax is a Rust workspace with two crates. `crates/betamax` is the installable CLI and owns
`clap` argument parsing, command dispatch, logging, and CLI packaging behavior. `crates/betamax-core`
contains tape parsing, terminal running, Ghostty VT integration, rendering, media writing, themes,
and state snapshots. Keep CLI-only dependencies out of the core crate.

Documentation lives under `docs/`, with user-facing command behavior in
`docs/tape-reference.md`. Small smoke-test tapes live under `examples/`; rendered media goes to
`examples/output` and `target/betamax-examples` and is not tracked. Scripts live in `scripts/`.

## Build, Test, and Development Commands

Install repository tools with:

```sh
mise install
```

Use `just` for local and CI-aligned tasks:

```sh
just check          # formatting, tests, docs, markdown lint, tape validation
just test           # cargo test --workspace --all-targets
just validate       # validate examples/*.tape and docs.tape
just smoke          # render examples/basic.tape
just release-check  # full release-oriented validation
```

Run the CLI locally with `cargo run -- run examples/basic.tape`.

## Coding Style & Naming Conventions

Rust formatting uses `rustfmt.toml` and nightly rustfmt:

```sh
rustup toolchain install nightly --component rustfmt
just fmt
```

Prefer small modules with clear ownership and documented non-obvious behavior. Public API docs
should explain behavior, return values, edge cases, and examples where useful. Markdown is linted
with `markdownlint-cli2`; wrap prose at 100 columns and keep tables aligned.

## Testing Guidelines

Use Rust unit, integration, and doc tests for library behavior. Add or update tape examples when a
change affects tape syntax, settings, rendering, or output formats. Use `State <path>.json` and
`Screenshot <path>` examples for terminal-testing behavior. Do not commit generated media outputs.

## Commit & Pull Request Guidelines

Keep unrelated fixes in separate pull requests. Use imperative, concise commit or pull-request
titles such as `Document contributor workflow` or `Fix state JSON style spans`.

Pull requests should explain the user-visible behavior, mention relevant docs or examples, and
include validation commands run, especially `just check` or why a narrower check was sufficient.
