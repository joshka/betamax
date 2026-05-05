# Development

Common development commands are available through the root [justfile](../justfile):

```sh
mise install
just check
```

## Formatting

Rust formatting uses [rustfmt.toml](../rustfmt.toml). The config intentionally uses unstable
rustfmt options such as grouped imports, comment wrapping, and doc comment formatting, so formatting
requires nightly rustfmt:

```sh
just fmt-check
```

Markdown linting uses [.markdownlint-cli2.yaml](../.markdownlint-cli2.yaml). It enforces aligned
table columns via `MD060/table-column-style` and ignores table rows for line-length checks.

## Checks

Useful targeted checks:

```sh
just test
just doc-test
just doc
just lint-md
just validate
```

Release-oriented checks are kept in `just` so local and CI behavior stay aligned:

```sh
just release-check
just package
just package-cli-verify
just install-smoke
```

`just package-cli-verify` and `just publish-dry-run-cli` require the matching `betamax-core`
version to already be available from crates.io, because Cargo rewrites the workspace path
dependency into a registry dependency when packaging the CLI crate.

The CLI directly depends on `libghostty-vt-sys` in addition to `betamax-core` so its build script
can add an rpath for the vendored native `libghostty-vt` library. That keeps local
`cargo install --path crates/betamax --locked` installs runnable with the current upstream sys
crate.

## Platform And Tooling Notes

`libghostty-vt-sys` is a native dependency and currently determines Betamax's platform support.
Betamax supports macOS and Linux. Windows is not supported because the upstream vendored
`libghostty-vt` build does not support Windows.

GIF, PNG, screenshot, and state JSON outputs are written in process. MP4 and WebM intentionally use
`ffmpeg` on `PATH`; this keeps first-cut video support small and debuggable, but it means video
output can fail at runtime on machines without ffmpeg installed.

Install ffmpeg with the platform package manager before rendering `.mp4` or `.webm` outputs:

```sh
# macOS
brew install ffmpeg

# Debian/Ubuntu
sudo apt-get update
sudo apt-get install ffmpeg
```

## README Assets

README GIFs are generated artifacts, not tracked files. They are hosted on the GitHub Release tag
`readme-assets` and can be refreshed after rendering examples:

```sh
scripts/render-examples.sh
scripts/upload-readme-assets.sh
```
