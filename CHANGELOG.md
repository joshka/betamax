# Changelog

<!-- markdownlint-disable no-duplicate-heading -->

## [0.1.9](https://github.com/joshka/betamax/compare/betamax-v0.1.8...betamax-v0.1.9) - 2026-06-16

### Other

- Use static libghostty-vt builds ([#66](https://github.com/joshka/betamax/pull/66))
- Add homepage quick-start demo ([#60](https://github.com/joshka/betamax/pull/60))

## [0.1.5](https://github.com/joshka/betamax/compare/betamax-core-v0.1.4...betamax-core-v0.1.5) - 2026-06-16

### Other

- Use static libghostty-vt builds ([#66](https://github.com/joshka/betamax/pull/66))

## [0.1.8](https://github.com/joshka/betamax/compare/betamax-v0.1.7...betamax-v0.1.8) - 2026-06-15

### Other

- Document Homebrew installation ([#58](https://github.com/joshka/betamax/pull/58))
- Add quick-start Betamax tape ([#56](https://github.com/joshka/betamax/pull/56))

## [0.1.4](https://github.com/joshka/betamax/compare/betamax-core-v0.1.3...betamax-core-v0.1.4) - 2026-06-15

### Other

- Add quick-start Betamax tape ([#56](https://github.com/joshka/betamax/pull/56))

## [0.1.7](https://github.com/joshka/betamax/compare/betamax-v0.1.6...betamax-v0.1.7) - 2026-06-15

### Fixed

- Make cargo-binstall launchers report `betamax` in help and usage output.
- Publish corrected cargo-binstall archives in a patch release.

## [0.1.6](https://github.com/joshka/betamax/compare/betamax-v0.1.5...betamax-v0.1.6) - 2026-06-15

### Other

- Add binary release install path ([#46](https://github.com/joshka/betamax/pull/46))

## [0.1.5](https://github.com/joshka/betamax/compare/betamax-v0.1.4...betamax-v0.1.5) - 2026-06-12

### Other

- release ([#14](https://github.com/joshka/betamax/pull/14))
- updated the following local packages: betamax-core

## [0.1.3](https://github.com/joshka/betamax/compare/betamax-core-v0.1.2...betamax-core-v0.1.3) - 2026-06-12

### Added

- forward terminal-to-host PTY replies back to child

## [0.1.4](https://github.com/joshka/betamax/compare/betamax-v0.1.3...betamax-v0.1.4) - 2026-05-07

### Other

- updated the following local packages: betamax-core

## [0.1.3](https://github.com/joshka/betamax/compare/betamax-v0.1.2...betamax-v0.1.3) - 2026-05-07

### Other

- Deepen documentation site

## [0.1.1](https://github.com/joshka/betamax/compare/betamax-core-v0.1.0...betamax-core-v0.1.1) - 2026-05-07

### Other

- Deepen documentation site

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
