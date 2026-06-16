# Development

Common development commands are available as [mise][mise] tasks. Install the repository tools and
the documentation site dependencies with mise and [pnpm][pnpm]:

```sh
mise install
pnpm install
mise run check
```

## Toolchain Requirements

Betamax is a Rust workspace, but the terminal engine depends on Ghostty's native VT library through
two Rust crates:

- `libghostty-vt` is the safe Rust wrapper that `betamax-core` uses directly.
- `libghostty-vt-sys` is the low-level FFI crate. Its default build fetches a pinned Ghostty source
  commit and runs Zig to build `libghostty-vt`.

The CLI crate does not depend on `libghostty-vt-sys` directly. Older Betamax releases needed that
direct dependency only so the CLI build script could add an rpath for the dynamically linked native
library. `libghostty-vt-sys` now links the native VT library statically by default, so the normal
dependency chain is `betamax` -> `betamax-core` -> `libghostty-vt` -> `libghostty-vt-sys`.

The repository pins Zig 0.15.2 in [.mise.toml](../.mise.toml), so run development, packaging, and
source-install commands through mise:

```sh
mise install
mise run test
mise run build-release -- aarch64-apple-darwin
```

mise is not the only possible way to provide that Zig version. [Nix][nix], a manually managed
[Zig][zig] 0.15.2, or another reproducible toolchain wrapper can also work if Cargo finds Zig
0.15.2 when `libghostty-vt-sys` builds. That version is an upstream Ghostty build requirement until
Ghostty supports newer Zig releases such as 0.16. mise is the documented path because it works for
this checkout and CI.

## Formatting

Rust formatting uses [rustfmt.toml](../rustfmt.toml). The config intentionally uses unstable
rustfmt options such as grouped imports, comment wrapping, and doc comment formatting, so formatting
requires nightly rustfmt installed with [rustup][rustup]:

```sh
mise run fmt-check
```

Clippy runs on stable and beta. Stable is the baseline, and beta catches new lints before they
reach the next stable release:

```sh
mise run clippy
mise run clippy-beta
```

Markdown linting uses [.markdownlint-cli2.yaml](../.markdownlint-cli2.yaml). It enforces aligned
table columns via `MD060/table-column-style` and ignores table rows for line-length checks.

## Rust Version Policy

The workspace `rust-version` is a compatibility floor, not a separately tested MSRV lane. It should
move only when Betamax code or dependency requirements need a newer compiler. Routine CI follows
the current stable compiler, with beta clippy used as an early warning for upcoming lint changes.

## Checks

Useful targeted checks:

```sh
mise run test
mise run doc-test
mise run doc
mise run lint-md
mise run validate
mise run docs-site-check
mise run docs-site-build
```

Dependency policy is enforced with [cargo-deny][cargo-deny]:

```sh
mise run dependency-policy
```

Docs dependencies are intentionally limited to [Astro][astro], [Starlight][starlight], Astro's
checker, and [TypeScript][typescript]. Prefer normal grouped pnpm updates first:

```sh
pnpm update
mise run docs-site-check
mise run docs-site-build
mise run pnpm-audit
```

Use `pnpm.overrides` only when upstream packages still resolve vulnerable transitive versions after
a normal update. Remove overrides once the direct docs dependencies naturally resolve patched
versions.

Release-oriented checks are kept as mise tasks so local and CI behavior stay aligned:

```sh
mise run release-check
mise run package
mise run package-cli-verify
mise run install-smoke
```

`mise run package-cli-verify` and `mise run publish-dry-run-cli` require the matching `betamax-core`
version to already be available from crates.io, because Cargo rewrites the workspace path
dependency into a registry dependency when packaging the CLI crate.

Run source installs through `mise` so Cargo finds the Zig version pinned in
[.mise.toml](../.mise.toml):

```sh
mise run install-local
```

## Installation Modes

Users can install Betamax with Homebrew from [joshka/homebrew-tap][homebrew-tap]:

```sh
brew install joshka/tap/betamax
```

If Homebrew requires trusted taps, trust the formula first:

```sh
brew trust --formula joshka/tap/betamax
```

They can also install from crates.io with [cargo-binstall][cargo-binstall]:

```sh
cargo binstall betamax
```

The CLI crate declares `[package.metadata.binstall]` in
[`crates/betamax/Cargo.toml`](../crates/betamax/Cargo.toml). The metadata points cargo-binstall at
GitHub Release assets named `betamax-<version>-<target>.tgz` on release tags named
`betamax-v<version>`. This follows cargo-binstall's [package support metadata][binstall-support]
model. Those archive names must stay aligned with
[`release-plz.yml`](../.github/workflows/release-plz.yml).

`libghostty-vt-sys` links the native VT library statically by default. The cargo-binstall archive
therefore only needs the `betamax` executable and does not need a companion shared library payload.

## Scripts And Tasks

Use mise tasks as the primary developer entrypoints. They load the repository toolchain before
running scripts or Cargo commands:

```sh
mise run render-examples
mise run upload-readme-assets
mise run build-release -- aarch64-apple-darwin
```

Scripts in [`scripts/`](../scripts/) remain small implementation details behind those tasks. Call
them directly only when you already have the mise environment active or are debugging the script
itself.

[`scripts/package-binstall-archive.sh`](../scripts/package-binstall-archive.sh) is release
workflow-only. Run `mise run build-release -- <target>` first, then pass the same target and
version to the packaging script. The script writes the cargo-binstall archive path to stdout so the
GitHub workflow can upload that exact file.

## Releases

Releases are managed by [release-plz][release-plz] in
[`.github/workflows/release-plz.yml`](../.github/workflows/release-plz.yml). The release job uses
crates.io [Trusted Publishing][trusted-publishing], so it does not read `CARGO_REGISTRY_TOKEN` and
must be configured on crates.io before it can publish a crate version.

The workflow uses [release-plz action outputs][release-plz-outputs] to build binary release assets
in the same workflow run. That avoids needing a personal access token or GitHub App token just to
trigger another workflow: release-plz's [GitHub token guidance][release-plz-token] documents that
events created with the default `GITHUB_TOKEN` do not start follow-up workflow runs.

Configure a trusted publisher for each published crate:

- crate: `betamax-core` and `betamax`
- repository: `joshka/betamax`
- workflow: `release-plz.yml`
- environment: `release`

The workflow has two jobs. `release-plz-release` publishes crate versions that exist on `main` but
are not yet on crates.io, then creates GitHub releases and tags. It installs the repo's mise tools
because package verification builds `libghostty-vt-sys`, which requires Zig 0.15.2.
`release-plz-pr` opens or updates the release PR that prepares the next version and changelog
entry.

Binary release assets are built by the `prepare-release-assets` and `release-assets` jobs in
[`release-plz.yml`](../.github/workflows/release-plz.yml) after release-plz reports that the
`betamax` package was released. Release-plz remains the owner of crates.io publishing, tags,
changelog content, and GitHub Release creation; the asset jobs upload cargo-binstall archives for
supported native targets.

## Platform And Tooling Notes

`libghostty-vt-sys` is a native dependency and currently determines Betamax's platform support.
Betamax supports macOS and Linux. Windows is not supported because the upstream `libghostty-vt`
build does not support Windows.

GIF, PNG, screenshot, and state JSON outputs are written in process. MP4 and WebM intentionally use
[ffmpeg][ffmpeg] on `PATH`; this keeps first-cut video support small and debuggable, but it means video
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
mise run render-examples
mise run upload-readme-assets
```

[astro]: https://astro.build/
[binstall-support]: https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md
[cargo-binstall]: https://github.com/cargo-bins/cargo-binstall
[cargo-deny]: https://github.com/EmbarkStudios/cargo-deny
[ffmpeg]: https://ffmpeg.org/
[homebrew-tap]: https://github.com/joshka/homebrew-tap
[mise]: https://mise.jdx.dev/
[nix]: https://nixos.org/
[pnpm]: https://pnpm.io/
[release-plz]: https://release-plz.dev/
[release-plz-outputs]: https://release-plz.dev/docs/github/output
[release-plz-token]: https://release-plz.dev/docs/github/token#how-to-trigger-further-workflow-runs
[rustup]: https://rustup.rs/
[starlight]: https://starlight.astro.build/
[trusted-publishing]: https://doc.rust-lang.org/cargo/reference/registry-authentication.html#trusted-publishing
[typescript]: https://www.typescriptlang.org/
[zig]: https://ziglang.org/
