# Changelog

## [0.1.2](https://github.com/joshka/betamax/compare/betamax-v0.1.1...betamax-v0.1.2) - 2026-05-07

### Other

- Fix CLI documentation link
- Expand CLI crate README

## [0.1.1](https://github.com/joshka/betamax/compare/betamax-v0.1.0...betamax-v0.1.1) - 2026-05-07

### Other

- Bump CLI crate version
- Implement Ghostty-first tape runner

## 0.1.0 - Unreleased

Initial Betamax release.

- Adds a Rust-first VHS-style CLI for running tape files.
- Runs terminal sessions through `portable-pty` and `libghostty-vt`.
- Renders GIF and PNG outputs in process.
- Writes MP4 and WebM through `ffmpeg` when those output formats are requested.
- Supports structured JSON terminal state for snapshot-style terminal tests.
- Includes copied Ghostty themes and a `themes` command for listing theme names.
- Validates unknown `Set` keys and mismatched setting types before starting the shell.
- Provides MIT OR Apache-2.0 package metadata, release check recipes, and CI.

Known limitations:

- `Source`, `record`, `serve`, and `publish` are intentionally not implemented.
- MP4 and WebM require `ffmpeg` on `PATH`.
- Rendering is based on libghostty-vt state plus Betamax's Rust raster path, not Ghostty's full
  native renderer.
- Theme fidelity is limited to the fields Betamax currently reads from Ghostty theme files.
