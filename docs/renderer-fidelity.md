# Renderer fidelity fixtures

The `betamax-core` integration tests `renderer_fidelity` and `sprite_rendering` form an acceptance
suite for the published libghostty backend and its software renderer. It drives the public capture API
with controlled VT bytes, then checks terminal state and raster pixels. No shell, PTY, network,
external TUI, clock, or sleep participates in the fixtures.

## Run and inspect

Install the repository tools with `mise install`, then run:

```sh
mise run renderer-fidelity
```

The suite also runs with `mise run test`. It requires **Menlo on macOS** (included with macOS) or
**DejaVu Sans Mono on other platforms**, plus a CJK fallback covering `界`. On Debian/Ubuntu,
install `fonts-dejavu-core fonts-noto-cjk`. macOS can use its system CJK fonts (for example PingFang
or Arial Unicode MS). The suite checks the actual fallback selected by cosmic-text: its shaped
glyph must be nonzero and its character map must cover `界`. A missing family or CJK glyph fails
with a setup message; tests do not silently skip or accept a missing-glyph box. Run with
`-- --nocapture` to print the selected CJK family and glyph ID. The Linux/macOS CI matrix runs the
suite and installs both Linux font packages explicitly.

To save the checkpoints before assertions:

```sh
BETAMAX_FIDELITY_OUTPUT="$PWD/target/renderer-fidelity" mise run renderer-fidelity
```

In a GitHub Actions run summary, select **View Linux fixtures** or **View macOS fixtures** for a
single-file HTML gallery with embedded PNGs and expandable state JSON. These unzipped artifacts
require GitHub sign-in and expire with repository retention. The gallery includes wide-glyph,
erasure/replacement, adjacent-style, and cursor checkpoints alongside the test-step outcome;
a partial gallery is not a passing result. Reporting runs after test failures too. If no checkpoints
exist, the summary explains that
no gallery is available. Generation/upload errors do not turn a failed test run into success.

When PR CI finishes, the **Renderer fixtures** bot comment links directly to both galleries.
Later runs update the same comment; missing galleries and failed runs are labeled explicitly.
The links have the same sign-in and retention requirements as the run-summary links.

The comment reporter runs separately after CI and loads its script from the trusted default-branch
commit. It never executes PR code or downloads artifacts with its comment-writing token. Run its
API-mocked tests locally with:

```sh
node --test scripts/renderer-comment.test.cjs
```

To verify the live integration after changing the reporter, merge the workflow/script change first,
then open a PR and let CI finish. Check the gallery links in its bot comment, rerun the completed
**Renderer gallery comment** workflow, and confirm that it updates the same comment instead of
creating another. A workflow introduced only on a PR branch cannot receive `workflow_run` events.

Build the same self-contained gallery locally with Python 3 (standard library only):

```sh
python3 scripts/renderer-gallery.py target/renderer-fidelity \
  target/renderer-gallery/local.html --platform local
```

Open the HTML file in a browser, or open the PNGs alongside their JSON files. CI uploads these as
`renderer-fidelity-ubuntu-latest` and `renderer-fidelity-macos-14` artifacts, including on test
failure. Filenames identify each transition; reruns overwrite matching files. Use a separate output
directory for concurrent suite runs. These diagnostic outputs belong under ignored `target/` and
must not be committed. There are no tracked image baselines or new binary assets.

The `cell-graphics-demo` checkpoint reproduces the terminal content from
`examples/cell-graphics.tape`, using the tracked VT input in
`crates/betamax-core/tests/fixtures/cell-graphics.vt`. CI regenerates its PNG and state JSON on both
platforms and includes them in the gallery. It checks the entire green fill and connected border
spans at exact pixel coordinates. It also encodes full and half fills into
`cell-graphics-demo.gif`, decodes both frames, and checks dimensions, frame delays, and every pixel
around the fill and border (allowing three channel levels for GIF palette quantization). The gallery
embeds this animation next to the PNG. Window decoration is omitted because this fixture exercises the
terminal renderer directly. The smaller `cell-graphics` checkpoint checks individual filled cells
and a complete rectangular border. Both use geometric assertions rather than font-dependent image
baselines.

## Procedural sprite integration

Betamax uses the published `qwertty-term-sprite` 0.4.0 rasterizer for its supported single-codepoint
characters. Coverage includes the complete box-drawing and block-element ranges, Braille, and
selected geometric shapes, Powerline separators, branch symbols, and legacy-computing symbols.
The dependency's dispatch table defines coverage, including gaps; Betamax does not duplicate it.
Ordinary text, unsupported symbols, and multi-codepoint graphemes use cosmic-text. Terminal parsing
and state remain with libghostty-vt; this is not a terminal-engine or rendering-stack migration.

The adapter supplies the same integer cell width and height used for terminal backgrounds and
positions, including letter and line spacing. `Metrics::simple` supplies the crate's default stroke
thickness (`max(cell_height / 12, 1)`). Geometry, fractional rounding, antialiasing and junction rules
belong to the dependency. This does not change how Betamax measures ordinary text or draw its
cursors/decorations through the sprite crate.

Glyphs are cached by character for the lifetime of a capture session, whose metrics are fixed.
Their alpha masks are independent of foreground color. A supported blank glyph is handled without
font fallback. The compositor uses signed origins and clips at the canvas, preserving overhang
between cells. All cell backgrounds are painted before any foreground glyphs, including across
rows, so backgrounds cannot erase diagonals extending into neighboring cells.

Vertical placement is verified against 0.4.0's `Canvas::into_glyph` implementation:
`bitmap_top = cell_top + cell_height - glyph.offset_y`, and
`bitmap_left = cell_left + glyph.offset_x`. Despite the dependency's mixed baseline/cell-bottom
wording, the text baseline must not be subtracted. Trimmed masks must not be stretched or centered.
Tests check full/upper/lower blocks at odd/even sizes, alpha blending, recoloring, fallback, blank
Braille, negative-origin clipping, and diagonal overhang/redraw.

The tracked `sprite-coverage.vt` fixture generates PNG, JSON and two-frame inverse-color GIF
checkpoints at 12×24, 13×25, 12×25 and 13×24 cell sizes. Each specimen includes rounded, double,
dashed and mixed borders, fractional blocks, shades/quadrants, Braille, Powerline, branch and
geometric symbols, sextants/octants and other legacy-computing symbols. It checks visible coverage,
blank cells, solid fills, shade values and connected edges. Both PNG and decoded GIF pixels must
match the captured frames exactly: monochrome coverage fits in GIF's 256-color palette. Dimensions,
frame count and delays are checked too. The gallery embeds both formats for each size.

The `sprite-alignment.vt` checkpoints make relationships between sprites explicit. Four border
networks check 48 joins per cell size: light, heavy, double and rounded corners, tees and crosses.
The complete edge profiles must match, including blank pixels and both double-border rails.
Rounded joins compare the half-coverage contour because curved endpoints are antialiased;
straight joins compare exact coverage. Every join must contain ink, so blank output cannot pass.

Repeated dashes, fractional fills, shades, Braille and Powerline symbols must preserve their entire
cell-local pattern. Complementary top/bottom blocks and quadrants must combine into the expected
fill; Braille dot columns must combine into full Braille; the first two sextants must combine into
their shared top strip. These union checks allow the intentional one-pixel overlap on odd cell
sizes but reject gaps. Braille dots and dashed gaps are intentionally separate, not continuous
strokes. Each alignment scene is checked through PNG and two-frame inverse-color GIF decoding at
all four sizes and appears beside the coverage specimens in the gallery.

The published crate contains its own golden-parity tests and 36 reference PNGs; its README's
statement that parity testing is deferred is stale. Betamax tests the integration and output path
rather than copying those baselines or adopting the rest of qwertty-term.

## Acceptance criteria

| Fixture                 | State assertions                                                                               | Image assertions                                                                                                                              |
| ----------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Full-screen file picker | Exact 32×10 layout, selection colors, hidden cursor, no scrollback                             | Border/title/selection ink, exact blank-cell colors, padding, partial redraw erases old text and background                                   |
| Wide and combining text | Wide continuation cell, combining sequence, cursor columns, fragmented UTF-8/CSI equivalence   | Real CJK glyph ink spans both cells; following columns and accents agree; edits erase both halves; adjacent styles and cursor overlay survive |
| Alternate screen        | Primary viewport and scrollback, isolated alternate screen, restored cursor and complete state | Primary frame restored byte-for-byte; state inspection leaves the viewport unchanged                                                          |
| Inverse and cursor      | Hidden/visible cursor and exact location                                                       | Inverse agrees with swapped truecolor; block cursor occupies only its cell; blink suppression restores hidden frame                           |

The file picker is a hand-authored **Ratatui-style** border, tabs, selected row, details, and footer.
It is not captured output from Ratatui and does not claim compatibility with a specific release.
Keeping the stream controlled makes each expected cell and transition reviewable without app,
terminal-size detection, prompt, locale, or dependency-version noise.

The original text/layout fixtures use a 392×248 canvas with 32×10 cells, four pixels of padding,
20-pixel text and 12×24-pixel cells. The demo uses a 780×350 canvas; the sprite specimens use
48×20 cells with eight pixels of padding. The built-in theme avoids local theme lookup.
Cursor visibility is controlled by
VT sequences or the capture API, rather than elapsed blink time.

Image checks compare exact solid colors and equivalent regions **within the same run**, not whole
images across machines. Menlo and DejaVu render differently; font versions and rasterization can
also change. Glyph-presence checks and the accent comparison prevent an entirely blank renderer
from passing equality checks. This suite intentionally does not pin every glyph outline or detect
every typographic regression. Its small relational checks need no platform-specific golden PNGs,
font downloads, or baseline regeneration procedure.

## Font coverage and remaining coverage

Wide-glyph rendering is an active acceptance test. All backgrounds are painted before text, so a
continuation cell cannot erase the right half of a glyph. Erasure and replacement at either half
are compared with a fresh terminal, including styled backgrounds. Adjacent colors and a cursor
over the continuation cell are checked separately.

Betamax relies on installed fonts and cosmic-text fallback selection. Missing font coverage is a
rendering limitation: production captures may show a missing-glyph box instead of reporting an
error. Install a suitable fallback for the scripts in your content. The fidelity suite treats
missing coverage as an actionable setup error rather than accepting that box as a complete glyph.

Emoji presentation/ZWJ sequences, Nerd Font symbols outside sprite coverage, broader font fallback,
ligatures, and
text decorations, bar/hollow/underline cursors, resizing, graphics protocols, and actual app
integration remain outside this first suite. Unsupported graphics behavior is not specified here.
A future backend should pass the state/geometry and wide-glyph tests. The current
continuation-space representation in state JSON is recorded as a baseline,
not a requirement for every future terminal-state API.
