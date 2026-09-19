# Feature field guide

The field guide shows seven example scenes that CI renders and checks on every pull request. Open
**Betamax field guide** in the Linux package job summary for a self-contained gallery with
animations, named checkpoints, expected results and the exact tapes. Raw PNG, GIF and state JSON are
in `feature-checkpoints`.

## Scenes and visual language

Every scene uses a 900×540 canvas, 20-pixel DejaVu Sans Mono, a mint accent (deep teal on light),
generous padding and a
rounded window on a slate background. Short titles explain the feature; terminal content stays
stable across runs. The light theme is an intentional comparison, not a different sample. The
fixture application is a tiny, local Python program: no network, packages, clock, randomness or
host filesystem contents enter a scene.

| Scene                   | Purpose                                            | Automated evidence                      |
| ----------------------- | -------------------------------------------------- | --------------------------------------- |
| From intent to input    | Type, paste, edit, inject environment, send keys   | Exact text/key bytes, cursor state      |
| Text with character     | Emphasis, colors, Unicode and cell graphics        | Style spans, Unicode cells, mint ink    |
| Room to breathe · dark  | Padding, margins, chrome and rounded corners       | Region colors and frame geometry        |
| Room to breathe · light | Same content in another theme and decoration       | Matched text/grid, light regions        |
| Present the action      | Captions and keyboard chips annotate a stable page | Equal state; bottom-row image diff      |
| Reveal the result       | Hidden setup executes but stays out of animation   | Hidden checkpoint; every GIF frame      |
| Keep the history        | Scrollback, alternate screen and restoration       | Complete history; restored state/pixels |

Tapes live in `examples/features/`. Their first two comments give the title and purpose.
`scripts/feature-gallery.json` supplies gallery descriptions and expectations. The supporting
`scripts/feature-scene.py` is a controlled terminal subject, not an alternative implementation of
Betamax. In particular, the input scene records the exact bytes sent by the key encoder; it does not
pretend to test a full editor. Tape-local clipboard operations never access the OS clipboard.

## Coverage map

This inventory follows the public tape commands and settings in [Tape reference](tape-reference.md),
checked against `Command` and `Settings::apply_set`. A setting being configured is not proof of all
its values. The table distinguishes representative behavior from exhaustive parameter testing.

### Commands

| Command                            | Scenario or existing focused coverage                                        |
| ---------------------------------- | ---------------------------------------------------------------------------- |
| `Output`                           | Each scene: GIF/PNG; format and timing cases below                           |
| `Require`                          | All scenes require Python; missing-tool failure is outside this visual suite |
| `Set`                              | Settings table below; parser tests reject unknown/wrong-type settings        |
| `Env`                              | Input: exact `field-guide` value reaches the child                           |
| `Type`, `Type@duration`            | Input: paced text, exact edited result; launch uses zero delay               |
| `Copy`, `Paste`                    | Input: `candidate-X` becomes `candidate-1` after Backspace                   |
| `Wait`, `Wait+Line`, `Wait+Screen` | Scenes use real app markers; smoke uses prompt/regex waits                   |
| `Sleep`                            | Readable holds only after waits; playback smoke measures unequal holds       |
| Keys, modifiers, repeats, delays   | Input: Backspace, Enter, Down ×2, Tab, Ctrl+P, key delays                    |
| `Hide`, `Show`                     | Magenta hidden checkpoint is absent from every decoded GIF frame             |
| `Caption`                          | Clean/caption/key-chip checkpoints: equal state, distinct bottom pixels      |
| `Screenshot`                       | Named PNG checkpoints, including while hidden                                |
| `State`                            | Exact text, styles, cursor, scrollback and alternate-screen pairs            |
| `Source`                           | Explicitly unsupported; documented runtime error, no misleading demo         |

### Settings

| Setting                      | Evidence or justified gap                                                  |
| ---------------------------- | -------------------------------------------------------------------------- |
| `Shell`                      | Bash launches local Python; shell normalization has unit tests             |
| `Theme`                      | Named pair/lookup tests; inline themes and user lookup precedence are gaps |
| `FontFamily`                 | DejaVu Sans Mono; OS fallback/CJK fidelity tests remain separate           |
| `FontSize`                   | 20px throughout; geometry/grid assertions; extreme sizes are a gap         |
| `LetterSpacing`              | Explicit zero; odd cell widths covered by sprite diagnostics               |
| `LineHeight`                 | 1.2 throughout; paired grid and image regions; no parameter sweep          |
| `Width`, `Height`            | Every decoded checkpoint and animation is 900×540                          |
| `Padding`                    | Stable content inset; frame-region checks and manual comparison            |
| `Margin`, `MarginFill`       | Exact slate outer-region pixels                                            |
| `WindowBar`, `WindowBarSize` | Left colorful/right colorful, 32px chrome regions                          |
| `BorderRadius`               | Corner is margin-colored while panel top is theme-colored                  |
| `Framerate`                  | 10fps feature capture; 20fps smoke duration/transition assertions          |
| `TypingSpeed`                | Zero setup delay plus paced `Type@60ms`; exact final input                 |
| `PlaybackSpeed`              | Normal-speed smoke; non-default capture tracked in [issue 149][speed]      |
| `LoopOffset`                 | Gap: no rotated-loop showcase; rotation obscures these causal narratives   |
| `CursorBlink`                | Disabled for stable checkpoints; blink cadence is an explicit visual gap   |
| `KeyboardOverlay`            | Input, Keys, Off represented; All/expiry use focused runner tests          |
| `KeyboardOverlayLocation`    | BottomRight and CaptionRow; other corners use runner tests                 |
| `WaitTimeout`                | 5s bounded scene waits; timeout failures are outside this success gallery  |
| `WaitPattern`                | Custom default and bare Wait in text scene; explicit regex in smoke        |

### Formats and specialized coverage

The feature selection renders only GIF and PNG. One independent `ci-playback.tape` renders GIF,
PNG, MP4 and WebM: dimensions, codecs, video pixel format, a red/blue transition, unequal one/two
second holds and the final frame are checked by `scripts/check-action-playback.py`. Input and
styling no longer multiply across every encoder. `ci-interaction.tape` remains a compact example.

Animated/static WebP, transparency, frame delays and decoding have focused tests in
`crates/betamax-core/tests/webp.rs`; PNG sequences and output routing have media/runner tests.
We deliberately do not add another full feature × format matrix. Library API contracts remain in
`public_api.rs`; CLI validation runs over all example tapes in CI; syntax has parser unit tests.

Linux and macOS [renderer fidelity diagnostics](renderer-fidelity.md) remain unchanged. They cover
CJK fallback, wide-cell occupancy, erasure, cursor overlays, odd cell geometry, block graphics and
partial redraws more precisely than these documentation scenes. This is a public tape feature
inventory, not a claim that every Rust method or every escape sequence has a visual scenario.

### CLI and library boundaries

`run` (file input, quiet mode and action-provided outputs) is exercised end to end here. Existing CI
also runs `validate` and `--version`. The `new` template writer, human/JSON `themes` listing, stdin
input, progress reporting and error-message presentation are not additional visual scenes; they
need separate CLI contract tests when changed. `run --publish` is a placeholder, not a publishing
feature. Library runner artifacts and custom capture backends are exercised by `public_api.rs`.

## Comparisons and known gaps

Use exact comparisons for state and for two checkpoints in the **same render run**. Caption changes
must leave the terminal canvas unchanged; returning from the alternate screen must restore the exact
primary image. Background-region colors and broad mint-ink counts are independent of glyph
antialiasing. Hidden visibility is checked against every decoded GIF frame, not one representative
sample.

We intentionally do not commit cross-platform PNG goldens. Font discovery, CJK fallback and
rasterization can differ between Linux and macOS. A perceptual whole-image threshold can hide a
small missing glyph while flagging a harmless font change. For a future stored baseline, first pin
the OS image and font files, then compare named regions and retain before/after/difference images;
review baseline changes as behavior changes. The current guide offers same-run checkpoint pairs
and per-PR evidence; it does **not** automatically compare against an older PR or main image.

The Unicode state assertion includes continuation-cell spaces after CJK characters because state
text follows terminal cell occupancy; it does not strip whitespace to conceal differences. Rendered
CJK correctness still depends on the specialized wide-glyph assertions and human inspection.

**Known visual defect:** [underline and strikethrough do not render][styles], despite correct style
spans. A minimal plain/underline/strike reproduction produced byte-identical PNGs. The specimen
keeps
those labels and the gallery calls out the gap. It asserts their state only; it does not bless their
missing pixels as a baseline. The independent fix must add pixel assertions when it lands. Faint,
overline, text blink, every cursor shape, all modified/function keys, every theme, shell and
keyboard
layout are also outside this scene set; existing focused tests are not a claim of complete fidelity.

Native PR attachment names currently depend on the pinned action reporter. Meaningful basename
captions are being added in [betamax-action PR 8][names]; until that separately reviewed reporter
rollout, native attachments retain numbered captions. The field guide and full-path action gallery
already identify every feature. No reporting credential or trust policy changes are needed here.

## Reproduce and inspect

Install the repository's mise tools, Python 3 and ffmpeg. Linux should also install
`fonts-dejavu-core` and `fonts-noto-cjk`, as the action does. From the checkout root:

```sh
mise run feature-gallery
open target/features/feature-gallery.html
```

On Linux, open the HTML file with your browser instead of `open`. To render just one tape:

```sh
BETAMAX_FEATURE_ROOT="$PWD" target/debug/betamax run examples/features/input-and-keys.tape
```

The explicit root is needed because the spawned PTY shell may start in the user's home directory.
The full runner supplies it automatically. Checkpoints are always relative to the CLI working
directory. To recheck existing output and rebuild the HTML without rerendering:

```sh
python3 scripts/check-feature-gallery.py
```

Inspect all seven animations, expand the clean/caption/chip and primary/alternate/restored pairs,
compare light and dark framing, and read the known-gap notice. A passed export count is
insufficient.
On failure the checker still writes a partial gallery with diagnostics. CI uploads the evidence with
`always()`; a failure before rendering may leave no checkpoints. Artifacts require GitHub sign-in
and expire with retention. Generated media stays untracked.

The feature render/check steps run on the same secret-free `contents: read` PR runner as the format
smoke. The protected reporter, PAT and environment configuration stay unchanged; see
[PR-built CLI previews](action-previews.md#trust-boundary).

[speed]: https://github.com/joshka/betamax/issues/149
[styles]: https://github.com/joshka/betamax/issues/151
[names]: https://github.com/joshka/betamax-action/pull/8
