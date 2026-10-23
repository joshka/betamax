# Module design

Choose modules around coherent features and domain concepts. A reader should be able to understand
and change one behavior without reconstructing unrelated configuration, state, and helpers.

## Start with the behavior

Before deciding where new code belongs, name the behavior it implements, the state or invariant it
owns, and the callers that need it. Inspect the existing types and methods across the relevant
crate; the current file is a starting point, not necessarily the right owner.

A declaration outline is a useful first pass:

```sh
rg -n \
  -e '^\s*(pub(\([^)]*\))?\s+)?((async|unsafe|const)\s+)?(struct|enum|trait|type|fn)\b' \
  -e '^\s*impl\b' \
  crates/betamax-core/src
```

Then read the implementations and call sites. Names alone do not show mutation, ordering, cache
lifetimes, or which calculations must agree. Describe a likely follow-up change and identify the
files and concepts a maintainer would need to understand to make it.

## Extend an existing module

Add code to an existing module when it strengthens that module's recognizable responsibility:

- It operates on the same feature state, enforces the same invariant, or completes an existing
  operation.
- Its helpers and configuration change for the same reasons as the behavior already there.
- Keeping it nearby makes the operation easier to read, and callers can continue to ignore its
  internal details.

Prefer a method on the owning type when that avoids repeatedly passing its fields to helpers. Keep
small private helpers near their caller. A helper used once does not need its own module.

Do not add code merely because a module already has convenient access to every required field.
`Settings` collecting startup values does not make it the owner of rendering or input-label policy.
A growing helper list with several unrelated vocabularies is a reason to reconsider ownership.

## Create a sibling or child

Create a module when it owns a behavior a reader can name and understand independently. An
independently useful feature can justify a new module from the start, even when an existing module
has room for it. Move the related types, configuration, state transitions, helpers, and tests
together where they support that behavior. Prefer vertical feature cuts: caption metrics,
truncation, and drawing belong together; a separate module for every feature's configuration,
layout, and rendering spreads the same story across technical layers.

Prefer wide module structures with a few meaningful levels:

- Add a sibling for an independently useful feature or concept. Captions and window chrome can
  evolve separately, so they are siblings under `runner`.
- Add a child for a substantial part of one feature whose details its other callers can ignore.
  Keyboard HUD label selection and key-chip drawing are children of `keyboard_overlay`.
- Repeated prefixes can reveal a missing parent namespace. Check that the proposed parent owns a
  coherent concept; sharing a word such as `frame` does not by itself make unrelated behaviors one
  feature.
- Keep a shared mechanism separate when several features genuinely depend on it. The canvas owns
  RGBA storage, clipping, and blending; it does not decide window-button colors or caption wording.

A module should hide enough detail to repay the extra navigation. Avoid catch-all `helpers` or
`utils` modules, chains of tiny forwarding files, and speculative extension frameworks. If a new
module needs most of its parent's private state, reconsider the boundary rather than exposing every
field or introducing another all-access context object.

### Compose independent features

An aggregate can hold another feature's configuration without owning its implementation. For
example, a window-shadow feature can have a `Shadow` type in a sibling `shadow.rs`, owning defaults,
numeric interpretation, eligibility, coverage calculation, painting, and tests. `WindowStyle` can
hold that value and delegate preparation to it. The window module does not need to absorb those
methods merely because a shadow appears behind a window or because its own file is small.

Presentation supplies the shared window bounds and schedules preparation on the cached backdrop.
The shadow feature decides how those bounds, its offset, blur, and opacity produce coverage; canvas
provides generic pixel blending. These calls connect the feature to its surroundings while keeping
its policy together. They also preserve the existing once-per-run preparation lifetime.

## Keep coordination explicit

Some decisions belong to the relationship between features. `presentation` resolves shared output
bounds, caption/keyboard spacing, drawing order, and renderer-cache lifetime. Caption shaping stays
with captions; keyboard-label expiration stays with the HUD; capture decides when recorded time
advances.

Keep one authoritative calculation for geometry used by both PTY sizing and image composition.
Feature modules can report their own space requirements; the coordinator resolves the combined
space. Keep that coordinator focused on these interactions instead of absorbing each feature's
implementation.

## Current runner ownership

Use these owners when extending runner features. They describe the current implementation, not a
requirement to preserve these exact files when a better boundary becomes clear.

- [`settings.rs`](../crates/betamax-core/src/runner/settings.rs): startup assembly, tape-order `Set`
  dispatch, common diagnostics, execution defaults and timing, and PTY-grid derivation.
- [`preset.rs`](../crates/betamax-core/src/runner/preset.rs): named appearance baselines, preset
  selection, and final-theme surface defaults. Explicit startup settings override the baseline.
- [`presentation.rs`](../crates/betamax-core/src/runner/presentation.rs): shared bounds and
  placement, composition order, and the prepared backdrop cache. Both PTY sizing and decoration use
  its geometry.
- [`window.rs`](../crates/betamax-core/src/runner/window.rs): aggregate window style, effective
  margin and bar dimensions, buttons, window painting, and delegation to independent effects.
- [`window_shape.rs`](../crates/betamax-core/src/runner/window_shape.rs): continuous corner geometry,
  cached outer/inner coverage, and the matching shadow-distance profile.
- [`border.rs`](../crates/betamax-core/src/runner/border.rs): inset outline configuration, automatic
  contrast, numeric interpretation, coverage composition policy, and border tests. Window chrome
  delegates drawing; prepared window coverage supplies geometry and canvas supplies opaque pixel mixing.
- [`window_title.rs`](../crates/betamax-core/src/runner/window_title.rs): title configuration, typography,
  automatic contrast, single-line text policy, bounded raster cache, and title behavior tests in
  its `tests` child. Window chrome resolves button clearance before applying the rounded mask.
- [`margin.rs`](../crates/betamax-core/src/runner/margin.rs): fill parsing, gradient projection and
  sampling, dithering, and backdrop rendering. Its `image_fill` child owns image syntax, path
  resolution, decoding, and fitting.
- [`insets.rs`](../crates/betamax-core/src/runner/insets.rs): per-side `Margin` values and their
  CSS shorthand parsing. Geometry users pick the side they need from the window's effective margin.
- [`caption.rs`](../crates/betamax-core/src/runner/caption.rs): caption text semantics, typography,
  height requirements, and the single-line shaping, truncation, and drawing mechanism reused by
  window titles.
- [`keyboard_overlay.rs`](../crates/betamax-core/src/runner/keyboard_overlay.rs): HUD configuration,
  input eligibility, and recorded-time event lifetime. Its `labels` child owns readable input
  intent; its `chips` child owns chip measurement, wrapping, and drawing.
- [`canvas.rs`](../crates/betamax-core/src/runner/canvas.rs): opaque pixel storage and shared
  drawing mechanisms. Keep feature-specific appearance and eligibility rules with their features.
- [`capture.rs`](../crates/betamax-core/src/runner/capture.rs): visibility, frame collection and
  coalescing, recorded durations, title-transition holds, and snapshots of presentation metadata.
- `ghostty/title.rs`: ordered OSC callbacks, event-boundary raster capture, hidden observation,
  and deferred callback errors. The session owns callback installation and PTY integration.

A test that constructs `Settings` belongs with the behavior it verifies. Keep integration tests for
shared bounds, drawing order, or interactions between features beside the coordinator. The test's
setup type does not determine its owner.

## Use size to prompt a closer look

Roughly 200–400 physical lines is a useful range for reviewing a module, not a minimum or maximum.
Focused modules of 100–300 or 300–500 lines can be reasonable. As a file approaches 1,000 lines,
inspect its responsibilities before adding more code.

Count the proposed methods, types, documentation, and tests before extracting. Separate production
code from test volume when explaining the size. A coherent implementation with many behavior tests
is different from a file mixing several features. A substantial test suite can live in a nearby
`tests.rs` child while remaining with its owning feature.

Check the destination modules too: an extraction that moves an oversized bucket into another file
has not solved the ownership problem. Conversely, do not fragment one clear algorithm merely to meet
a line target. Merge or inline a weak module when it adds names and jumps without hiding useful
detail.

## Check the proposed addition before extending it

For an in-flight feature, first incorporate the current merged module layout using the normal jj
workflow while preserving in-progress work. Resolving a rebase conflict by placing all new code in
whichever file compiles can undo a recent ownership improvement.

Before growing the implementation, make a compact inventory:

1. Name the feature owner and list the proposed types, methods, helpers, and tests assigned to it.
1. Identify the shared operations it needs and what remains in the coordinator or startup dispatch.
1. Estimate the resulting size of each destination, including existing code. Explain any substantial
   child or sibling by the behavior it owns, not by the number of lines it absorbs.
1. Name a representative follow-up change and check that it stays near its owning behavior.

Choose between extending an existing owner and introducing a new feature owner based on cohesion
and caller needs. The current file layout is not a whitelist of permitted concepts. Check that the
feature's policy stays together while its integration with shared mechanisms remains explicit. This
is a focused ownership check, not a requirement to redesign adjacent features or stop for approval
on routine implementation decisions.

## Review the resulting boundary

Before accepting a new grouping, check:

- Can each important behavior point to one owner or an explicit coordinator?
- Do the destination's methods form one recognizable concept? Which proposed method fits least?
- Does a representative feature change require fewer unrelated concepts and file jumps?
- Does the boundary reduce field traffic and visibility rather than duplicate state or calculations?
- Are the related helpers, contracts, and tests still easy to find together?
- Does each new module justify its name and navigation cost independently of its line count?

Keep structural refactors separate from intentional behavior changes. Preserve tape-order semantics,
terminal bounds, drawing order, recorded-time behavior, and cache lifetime. Use focused behavior
tests and, for rendering changes, matched before/after pixel comparisons where practical. See
[renderer fidelity](renderer-fidelity.md) for the existing evidence and its limits.
