# Changelog

Betamax has one product changelog covering the CLI and core library. Historical entries use CLI
versions and identify the core version shipped in the release lockfile.

## [Unreleased]

## [0.1.21](https://github.com/joshka/betamax/compare/betamax-v0.1.20...betamax-v0.1.21) - 2026-09-19

### Other

- Unify Betamax release history ([#157](https://github.com/joshka/betamax/pull/157))

- Use one shared version for the CLI and core library, starting with 0.1.21.
- Consolidate release notes into one product changelog, including core improvements in CLI releases.

## [0.1.20](https://github.com/joshka/betamax/compare/betamax-v0.1.19...betamax-v0.1.20) - 2026-09-19

Includes [betamax-core 0.1.16](https://crates.io/crates/betamax-core/0.1.16).

### Added

- Bundle Aardvark Ink so the theme works on fresh machines and CI runners
  ([#158](https://github.com/joshka/betamax/pull/158)).

## [0.1.19](https://github.com/joshka/betamax/compare/betamax-v0.1.18...betamax-v0.1.19) - 2026-09-19

Includes [betamax-core 0.1.15](https://crates.io/crates/betamax-core/0.1.15).

### Fixed

- Render faint terminal text ([#156](https://github.com/joshka/betamax/pull/156)).
- Render underline and strikethrough ([#152](https://github.com/joshka/betamax/pull/152)).
- Fix capture playback speed scaling ([#150](https://github.com/joshka/betamax/pull/150)).

## [0.1.18](https://github.com/joshka/betamax/compare/betamax-v0.1.17...betamax-v0.1.18) - 2026-09-19

Includes [betamax-core 0.1.14](https://crates.io/crates/betamax-core/0.1.14).

### Other

- Add lossless animated and static WebP output ([#145](https://github.com/joshka/betamax/pull/145))
- Render terminal graphics with cached sprites ([#144](https://github.com/joshka/betamax/pull/144))

## [0.1.17](https://github.com/joshka/betamax/compare/e1e6c94f1a874e07595acf824ca0c5d53ca140e5...betamax-v0.1.17) - 2026-09-19

Includes [betamax-core 0.1.13](https://crates.io/crates/betamax-core/0.1.13).

### Other

- Use libghostty tracing logger ([#74](https://github.com/joshka/betamax/pull/74))
- Clarify Zig build requirements and troubleshooting ([#134](https://github.com/joshka/betamax/pull/134))
- Fix video playback timing ([#143](https://github.com/joshka/betamax/pull/143))
- Fix wide-glyph clipping and CJK fixture coverage ([#139](https://github.com/joshka/betamax/pull/139))
- Add controlled renderer fidelity fixtures ([#136](https://github.com/joshka/betamax/pull/136))

## [0.1.16](https://crates.io/crates/betamax/0.1.16) - 2026-09-19

Includes [betamax-core 0.1.12](https://crates.io/crates/betamax-core/0.1.12).

Published on crates.io on 2026-09-19; no GitHub tag or release was created.

### Other

- Add corner keyboard overlay locations
- Bump the rust group across 1 directory with 3 updates ([#107](https://github.com/joshka/betamax/pull/107))
- Anti-alias rounded overlay edges ([#98](https://github.com/joshka/betamax/pull/98))
- Polish keyboard overlay layout ([#96](https://github.com/joshka/betamax/pull/96))

## [0.1.15](https://github.com/joshka/betamax/compare/betamax-v0.1.14...betamax-v0.1.15) - 2026-06-23

Includes [betamax-core 0.1.11](https://crates.io/crates/betamax-core/0.1.11).

### Other

- Place presentation overlays outside terminal ([#94](https://github.com/joshka/betamax/pull/94))

## [0.1.14](https://github.com/joshka/betamax/compare/betamax-v0.1.13...betamax-v0.1.14) - 2026-06-22

Includes [betamax-core 0.1.10](https://crates.io/crates/betamax-core/0.1.10).

### Other

- Add keyboard overlay rendering ([#89](https://github.com/joshka/betamax/pull/89))
- Add caption behavior tests ([#90](https://github.com/joshka/betamax/pull/90))
- Add media captions ([#86](https://github.com/joshka/betamax/pull/86))

## [0.1.13](https://github.com/joshka/betamax/compare/betamax-v0.1.12...betamax-v0.1.13) - 2026-06-21

Includes [betamax-core 0.1.9](https://crates.io/crates/betamax-core/0.1.9).

### Other

- Align frame sizing with VHS ([#82](https://github.com/joshka/betamax/pull/82))

## [0.1.12](https://github.com/joshka/betamax/compare/betamax-v0.1.11...betamax-v0.1.12) - 2026-06-19

Includes [betamax-core 0.1.8](https://crates.io/crates/betamax-core/0.1.8).

### Added

- Embed bundled Ghostty themes ([#80](https://github.com/joshka/betamax/pull/80)).

## [0.1.11](https://github.com/joshka/betamax/compare/betamax-v0.1.10...betamax-v0.1.11) - 2026-06-16

Includes [betamax-core 0.1.7](https://crates.io/crates/betamax-core/0.1.7).

### Other

- Add media encoding progress ([#78](https://github.com/joshka/betamax/pull/78))

## [0.1.10](https://github.com/joshka/betamax/compare/betamax-v0.1.9...betamax-v0.1.10) - 2026-06-16

Includes [betamax-core 0.1.6](https://crates.io/crates/betamax-core/0.1.6).

### Other

- Trace Ghostty capture boundaries ([#72](https://github.com/joshka/betamax/pull/72))

## [0.1.9](https://github.com/joshka/betamax/compare/betamax-v0.1.8...betamax-v0.1.9) - 2026-06-16

Includes [betamax-core 0.1.5](https://crates.io/crates/betamax-core/0.1.5).

### Other

- Use static libghostty-vt builds ([#66](https://github.com/joshka/betamax/pull/66))
- Add homepage quick-start demo ([#60](https://github.com/joshka/betamax/pull/60))

## [0.1.8](https://github.com/joshka/betamax/compare/betamax-v0.1.7...betamax-v0.1.8) - 2026-06-15

Includes [betamax-core 0.1.4](https://crates.io/crates/betamax-core/0.1.4).

### Other

- Document Homebrew installation ([#58](https://github.com/joshka/betamax/pull/58))
- Add quick-start Betamax tape ([#56](https://github.com/joshka/betamax/pull/56))

## [0.1.7](https://github.com/joshka/betamax/compare/betamax-v0.1.6...betamax-v0.1.7) - 2026-06-15

Includes [betamax-core 0.1.3](https://crates.io/crates/betamax-core/0.1.3).

### Fixed

- Make cargo-binstall launchers report `betamax` in help and usage output.
- Publish corrected cargo-binstall archives in a patch release.

## [0.1.6](https://github.com/joshka/betamax/compare/betamax-v0.1.5...betamax-v0.1.6) - 2026-06-15

Includes [betamax-core 0.1.3](https://crates.io/crates/betamax-core/0.1.3).

### Other

- Add binary release install path ([#46](https://github.com/joshka/betamax/pull/46))

## [0.1.5](https://github.com/joshka/betamax/compare/betamax-v0.1.4...betamax-v0.1.5) - 2026-06-12

Includes [betamax-core 0.1.3](https://crates.io/crates/betamax-core/0.1.3).

### Other

- release ([#14](https://github.com/joshka/betamax/pull/14))

### Added

- forward terminal-to-host PTY replies back to child

## [0.1.4](https://github.com/joshka/betamax/compare/betamax-v0.1.3...betamax-v0.1.4) - 2026-05-07

Includes [betamax-core 0.1.2](https://crates.io/crates/betamax-core/0.1.2).

### Fixed

- Preserve elapsed capture time and coalesce identical frames so static GIF holds retain their duration.

## [0.1.3](https://github.com/joshka/betamax/compare/betamax-v0.1.2...betamax-v0.1.3) - 2026-05-07

Includes [betamax-core 0.1.1](https://crates.io/crates/betamax-core/0.1.1).

### Other

- Deepen documentation site

## [0.1.2](https://github.com/joshka/betamax/compare/betamax-v0.1.1...betamax-v0.1.2) - 2026-05-07

Includes [betamax-core 0.1.0](https://crates.io/crates/betamax-core/0.1.0).

### Other

- Fix CLI documentation link
- Expand CLI crate README

## [0.1.1](https://github.com/joshka/betamax/compare/betamax-v0.1.0...betamax-v0.1.1) - 2026-05-07

Includes [betamax-core 0.1.0](https://crates.io/crates/betamax-core/0.1.0).

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
