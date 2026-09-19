# Renderer fidelity fixtures

The `betamax-core` integration test `renderer_fidelity` is a small acceptance suite for the current
published libghostty backend and future terminal-core migrations. It drives the public capture API
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
require GitHub sign-in and expire with repository retention. The gallery labels the known CJK
clipping/missing-font limitation and shows the test-step outcome; a partial gallery is not a passing
result. Reporting runs after test failures too. If no checkpoints exist, the summary explains that
no gallery is available. Generation/upload errors do not turn a failed test run into success.

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

The fixed canvas is 392×248 pixels with 32×10 cells, four pixels of padding, 20-pixel text, and
12×24-pixel cells. The built-in theme avoids local theme lookup. Cursor visibility is controlled by
VT sequences or the capture API, rather than elapsed blink time.

Image checks compare exact solid colors and equivalent regions **within the same run**, not whole
images across machines. Menlo and DejaVu render differently; font versions and rasterization can
also change. Glyph-presence checks and the accent comparison prevent an entirely blank renderer
from passing equality checks. This suite intentionally does not pin every glyph outline or detect
every typographic regression. Its small relational checks need no platform-specific golden PNGs,
font downloads, or baseline regeneration procedure.

## Font coverage and remaining coverage

Wide-glyph rendering is an active acceptance test. Row backgrounds are painted before text, so a
continuation cell cannot erase the right half of a glyph. Erasure and replacement at either half
are compared with a fresh terminal, including styled backgrounds. Adjacent colors and a cursor
over the continuation cell are checked separately.

Betamax relies on installed fonts and cosmic-text fallback selection. Missing font coverage is a
rendering limitation: production captures may show a missing-glyph box instead of reporting an
error. Install a suitable fallback for the scripts in your content. The fidelity suite treats
missing coverage as an actionable setup error rather than accepting that box as a complete glyph.

Emoji presentation/ZWJ sequences, Nerd Font symbols, broader fallback selection, ligatures, and
text decorations, bar/hollow/underline cursors, resizing, graphics protocols, and actual app
integration remain outside this first suite. Unsupported graphics behavior is not specified here.
A future backend should pass the state/geometry and wide-glyph tests. The current
continuation-space representation in state JSON is recorded as a baseline,
not a requirement for every future terminal-state API.
