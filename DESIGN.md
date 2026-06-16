---
version: alpha
name: Betamax Analog Documentation
description: >-
  A dark, warm, 80s analog technical-manual design system for the Betamax documentation site. It
  carries the homepage's VCR/Betamax identity into docs without using full hardware skeuomorphism.
colors:
  background: "#060403"
  background-noise-base: "#0a0705"
  margin-dark: "#030201"
  sidebar: "#0d0906"
  sidebar-raised: "#130d08"
  surface: "#120d09"
  surface-raised: "#1a110b"
  surface-inset: "#080604"
  surface-code: "#0b0806"
  border-subtle: "#2a1a10"
  border: "#4a2c18"
  border-strong: "#70401f"
  text: "#ead4b3"
  text-strong: "#f4dfbd"
  text-muted: "#b99066"
  text-dim: "#7f6044"
  link: "#f0782c"
  link-hover: "#ff9a4a"
  accent: "#d85b2a"
  accent-dark: "#7f2118"
  accent-orange: "#bd471d"
  accent-amber: "#d48614"
  stripe-cream: "#e6c997"
  stripe-amber: "#d48614"
  stripe-orange: "#bd471d"
  stripe-red: "#7f2118"
  success: "#9fbd73"
  warning: "#d48614"
  danger: "#c74227"
typography:
  display-xl:
    fontFamily:
      "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
    fontSize: 56px
    fontWeight: 800
    lineHeight: 1.0
    letterSpacing: 0
  display-lg:
    fontFamily:
      "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
    fontSize: 44px
    fontWeight: 800
    lineHeight: 1.05
    letterSpacing: 0
  heading-md:
    fontFamily:
      "ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
    fontSize: 26px
    fontWeight: 700
    lineHeight: 1.18
    letterSpacing: 0
  heading-sm:
    fontFamily:
      "ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
    fontSize: 20px
    fontWeight: 700
    lineHeight: 1.25
  body-md:
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace"
    fontSize: 15px
    fontWeight: 400
    lineHeight: 1.68
  body-sm:
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace"
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.6
  code-md:
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace"
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.55
  label-caps:
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace"
    fontSize: 12px
    fontWeight: 700
    lineHeight: 1.1
    letterSpacing: 0.11em
  nav-label:
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace"
    fontSize: 14px
    fontWeight: 500
    lineHeight: 1.35
rounded:
  none: 0px
  xs: 2px
  sm: 4px
  md: 8px
  lg: 12px
  xl: 16px
spacing:
  unit: 8px
  hairline: 1px
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 32px
  xxl: 48px
  xxxl: 64px
  sidebar-width: 300px
  content-max: 1220px
  content-readable: 780px
  topbar-height: 72px
  gutter-desktop: 64px
  gutter-wide: 96px
components:
  page-shell:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text}"
  page-margin:
    backgroundColor: "{colors.margin-dark}"
  topbar:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text-strong}"
    borderColor: "{colors.border-subtle}"
    height: "{spacing.topbar-height}"
  sidebar:
    backgroundColor: "{colors.sidebar}"
    textColor: "{colors.text-muted}"
    borderColor: "{colors.border-subtle}"
    width: "{spacing.sidebar-width}"
  sidebar-item-active:
    backgroundColor: "#2a160d"
    textColor: "{colors.text-strong}"
    borderColor: "{colors.accent}"
    rounded: "{rounded.sm}"
  content-panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    borderColor: "{colors.border-subtle}"
  command-block:
    backgroundColor: "{colors.surface-code}"
    textColor: "{colors.text-strong}"
    borderColor: "{colors.border}"
    rounded: "{rounded.sm}"
    padding: "12px 16px"
  manual-card:
    backgroundColor: "rgba(26, 17, 11, 0.72)"
    textColor: "{colors.text}"
    borderColor: "{colors.border}"
    rounded: "{rounded.md}"
    padding: "{spacing.lg}"
  note-card:
    backgroundColor: "rgba(42, 26, 16, 0.48)"
    textColor: "{colors.text}"
    borderColor: "{colors.border}"
    rounded: "{rounded.sm}"
    padding: "{spacing.md}"
  table:
    backgroundColor: "rgba(8, 6, 4, 0.44)"
    textColor: "{colors.text}"
    borderColor: "{colors.border}"
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.sm}"
    padding: "10px 18px"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.text-strong}"
    borderColor: "{colors.border-strong}"
    rounded: "{rounded.sm}"
    padding: "10px 18px"
---

# Betamax Analog Documentation Design System

## Overview

Betamax docs should feel like a dark 1980s analog technical manual for a VCR-era developer tool:
warm, practical, structured, and tactile without becoming a fake physical binder or a hardware
simulation.

The homepage is allowed to be cinematic and physical. Interior documentation should be calmer: an
operator manual that borrows the same Betamax colors, stripes, and analog warmth, while staying
readable and task-focused.

The target mood is:

- **80s analog, not digital neon.** Think VCR packaging, service manuals, tape labels, matte
  plastic, amber print, and warm paper ink.
- **Manual-like, not app-like.** Documentation should prioritize sequence, reference, examples, and
  scannability.
- **Dark by default.** Light surfaces may exist only as rare callout/content variants; the primary
  theme is dark.
- **Subtle texture.** Use noise/grain only. Avoid visible repeating patterns, leather, carbon fiber,
  brushed metal, paper stains, and excessive skeuomorphism.
- **Betamax identity throughout.** The stripe motif is mandatory. Page-title stripe rules use three
  colored bars: amber, orange, then red. Cream belongs to title text and the wordmark, not as a
  fourth page-title stripe.

Do not use the standalone `B` symbol as a recurring docs decoration. It can exist only if it is
already part of a fixed asset, not as a generic icon, footer badge, or callout marker.

## Colors

The palette is warm, dark, and analog. It must never become blue-black cyberpunk or bright modern
SaaS.

- **Outer margin / page void:** use `margin-dark` or `background`. This area should be the darkest
  part of the page so the content plane feels intentional.
- **Main content surface:** use `surface` with subtle noise. It should be a little lighter than the
  outer margin but still very dark.
- **Sidebar:** use `sidebar`, slightly darker than content, separated by a thin warm border.
- **Text:** use `text` for body and `text-strong` for headings. Body text must be cream/beige, never
  pure white.
- **Muted text:** use `text-muted` for captions, metadata, nav items, and secondary prose.
- **Links and active states:** use `link`, `accent`, or `accent-orange`. Links may be underlined or
  highlighted, but do not glow.
- **Stripe sequence:** page-title rules use amber, orange, then red. Cream text may visually pair
  with those three colored stripes in brand lockups.

Never introduce saturated blue, purple, cyan, or neon green unless a code sample itself requires it.
The docs chrome should remain brown/orange/red/cream.

## Typography

Use restrained typography that evokes a printed technical manual and a terminal-adjacent tool.

- **Brand lockup:** always use the real Betamax wordmark lockup in the top-left header, or a direct
  compact horizontal derivative of it. Do not rebuild the brand as generic text beside a standalone
  stripe icon.
- **Page titles:** use large warm cream display type with sturdy sans proportions. Titles should
  feel like printed manual display text, not serif book typography or futuristic techno lettering.
- **Body text:** may use a readable mono stack to preserve the technical manual feel. Keep body size
  large enough for real reading.
- **Labels:** use uppercase mono labels with letter spacing for nav groups, breadcrumbs, metadata,
  and command labels.
- **Code:** always monospace. Code should be among the highest-contrast elements on the page.

Use no more than three font families. If no custom fonts are available, use system stacks from the
tokens above rather than adding a new external dependency.

## Layout

Design primarily for a 16-inch MacBook viewport. At desktop widths, show the outer margins, the
fixed left navigation, and a broad content plane. The design should not look like a narrow mobile
page stretched across a large monitor.

Desktop structure:

1. **Top bar:** full-width, 72px high. Left: Betamax stripe hamburger + text lockup. Center or
   mid-left: search. Right: simple text/icon links. No neon highlights.
2. **Left sidebar:** fixed-width manual index rail around 300px. It is useful and should usually
   stay visible on desktop.
3. **Main content:** max width around 1220px. Align to the left of the content area, with breathing
   room to the right. The content plane should be darker than mockups with cream paper, but slightly
   lighter than the page margin.
4. **Right TOC:** optional. Use it only for long reference pages. On short guide pages, prefer no
   right TOC or a compact inline “At a glance” block.

Spacing rules:

- Use generous vertical rhythm around page headers, but keep body sections dense enough for docs.
- Section rows can use two columns: prose/summary on the left, code/table/note on the right.
- On pages like Quick Start, use numbered rows that make the workflow obvious.
- On pages like Tape Reference, use broader tables and card-like reference sections.
- At mobile/tablet widths, collapse the sidebar behind navigation and stack all section grids.

The main content should not be centered like a marketing landing page unless the page is
intentionally a splash/overview. Docs pages should feel like a manual index + work surface.

## Elevation & Depth

Depth comes from tonal layers, borders, inset panels, and very subtle shadows — not from heavy 3D
realism.

- Use thin warm borders (`border-subtle`, `border`, `border-strong`) to define structure.
- Use `surface-raised` sparingly for cards, callouts, and command blocks.
- Use low-opacity shadows only to separate the top bar, sidebar, and main cards. Avoid glossy
  highlights.
- Use subtle inner borders or inset shadows for command blocks and tables.
- Texture should be a low-opacity random noise overlay. Avoid repeated wallpaper, leather, cloth,
  carbon fiber, brushed metal, or visible pattern motifs.

The content must feel tactile but still be a web document.

## Shapes

Use rectangular analog shapes with small corner radii.

- Default radius: 4px to 8px.
- Cards and larger panels: up to 12px.
- Avoid pill-shaped modern SaaS controls except for tiny chips or keyboard key shapes.
- Avoid over-rounded CRT-like panels and large soft glassmorphism.
- Stripe bars should have square or barely rounded ends unless they are intentionally referencing
  the homepage’s rounded stripe corner motif.

## Components

### Brand Header

Non-negotiable: the docs header uses the Betamax logo lockup or a direct horizontal version of that
lockup. The logo should look analog and printed, not glowing.

Preferred header lockup:

- Cream `Betamax` wordmark.
- Three colored stripes associated directly with the wordmark.
- Compact enough to fit the 72px docs header without becoming a generic icon.
- Keep it top-left in desktop docs.

Optional rectangular badge variant:

- `Betamax` word above the red/orange/amber stripes inside a low-contrast badge.
- Use only when it is the actual logo asset or a faithful crop/scale of it, not as a replacement
  made from unrelated text and stripe shapes.

Do not use a standalone `B` as a generic docs logo or decorative stamp.

### Sidebar

The sidebar should feel like a manual contents rail, not a default Starlight app sidebar.

- Darker than the content surface.
- Thin vertical warm border on the right.
- Group headings in uppercase orange/amber labels.
- Nav items are cream/muted text.
- Active item: flat dark-brown fill, orange left rule or small orange indicator, cream text. No
  glow, no gradient shine.
- Keep spacing compact and readable.
- Avoid large decorative stripe blocks at the bottom. If a footer mark is needed, use a small text
  panel or one narrow stripe rule, not a large stack of stripes.

### Page Header

Each docs page should start with:

1. Eyebrow/breadcrumb label such as `AUTHORING / TAPE FILES` or `START / QUICK START`.
2. Large page title.
3. A horizontal Betamax stripe rule under the title.
4. One short description paragraph.

Do not place the full Betamax logo inside every page title. The global header already carries the
brand.

The stripe rule should usually appear **under the page title**, not as a dominant top-of-page
banner. It may span most of the content width. It should be strong enough to create identity but not
so large that it competes with the content.

### Guide Page Rows

Use for Quick Start, Tape Files, Outputs, Themes, Generated Media, Terminal Testing, and similar
teaching pages.

- Use section rows with a left prose column and a right code/example/note column.
- Numbered workflows should show a warm outlined number badge, not a bright digital pill.
- Put the command or example close to the prose explaining it.
- Prefer one clear command block per concept over many disconnected code blocks.
- If a section is mostly prose, allow a side note card or compact checklist.

### Reference Sections

Use for Tape Reference, State JSON, CLI reference, settings tables, and command reference pages.

- Use bordered reference modules with a heading, short explanation, and table/code content.
- Icons may be used sparingly and should be line-based, warm cream/orange, and analog. Do not use
  glowing digital icons.
- Tables should be full-width inside their module when possible.
- Long tables should prioritize legibility over decoration.

### Command Blocks

Command blocks are central to the docs and must be crisp.

- Background: very dark, nearly black-brown.
- Border: warm brown, 1px.
- Text: cream, with commands and prompts optionally amber/orange.
- Copy button: subtle outline/icon, no glowing hover.
- Optional label: small uppercase `COMMAND`, `TAPE`, `SHELL`, or `OUTPUT` above or inside the block.
- Do not put command blocks on bright cream unless a deliberate print-manual variant is being
  created. Dark mode is primary.

### Tables

Tables should feel like technical reference sheets.

- Header row uses uppercase orange/amber labels.
- Cell text remains cream and readable.
- Use thin warm dividers.
- Avoid overly dense microtext. Increase row height before reducing font size below 13px.
- For output tables, small left icons are acceptable if they improve scanning.

### Notes, Tips, and Warnings

Callouts should look like manual annotations.

- Use subtle border-left or top rule in accent color.
- Use a muted surface, not a bright colored fill.
- Label with `NOTE`, `TIP`, `WARNING`, or `COMPATIBILITY` in uppercase mono.
- Keep iconography minimal and non-glowing.

### Stripes

The stripe motif is mandatory but should be systematic.

Good uses:

- Under page titles.
- Thin divider in the top bar or content header.
- Small section cap on important cards.
- Tiny nav indicator or rule.
- Small footer detail.

Avoid:

- Large decorative stripe stacks in the bottom-left sidebar.
- Repeating stripe wallpaper.
- Stripes that overpower the content.
- Stripes used as neon light beams.

Page-title stripe order should be amber, orange, then red. Use cream as nearby title or wordmark
text, not as a fourth line in the rule.

## Page Type Guidance

### Quick Start

Goal: make Betamax usage obvious in under one minute.

Recommended structure:

1. Page header with stripe rule and short compatibility note.
2. Numbered workflow sections: Install, Repository Software, Create A Tape, Validate Tapes, List
   Themes, Repository Examples.
3. For each step, put prose on the left and the command block on the right.
4. Use notes for ffmpeg, Zig/mise, and validation behavior only where needed.
5. Do not include a right TOC unless the page becomes substantially longer.

### Tape Files

Goal: explain the authoring model.

Recommended structure:

- Intro paragraph.
- File Structure section with prose + a compact tape example.
- Tokenization section with bullets + examples.
- Synchronization and Hidden Work sections as rows with code and notes.
- Use right-side note cards when a rule is important.

### Outputs

Goal: help users choose output formats.

Recommended structure:

- Big output table near the top.
- Three explanatory sections: Choosing Outputs, Video Encoding, Checkpoints.
- Keep the table readable and wide.
- Use small output icons only if they make scanning easier.

### Themes And Styling

Goal: explain theme lookup and rendering controls.

Recommended structure:

- Theme lookup command block.
- Palette preview cards for common themes if available.
- Settings tables grouped by Layout, Frame Decoration, Timing/Cursor.
- Avoid page-wide color chaos; theme swatches should be controlled accents.

### Examples

Goal: show real tape files and previews.

Recommended structure:

- Example index table with tape name, demonstrates, useful docs.
- Preview cards for GIFs/screenshots.
- Keep previews in dark frames with small captions.
- Do not use the large homepage hardware frame on every example.

### Tape Reference

Goal: dense but navigable reference.

Recommended structure:

- Page header with stripe rule.
- Optional compact metadata card near the top.
- Bordered modules for Parsing Rules, Outputs, Settings, Command Reference, Key Commands, CLI
  Commands.
- Right TOC is acceptable here because the page is long.
- Tables should be large enough for real reading.

## Content Editing Guidance for Codex

When refactoring MDX for this design system:

- Preserve the existing technical facts and commands.
- Improve structure by grouping prose, code, notes, and tables into reusable components.
- Prefer semantic components over one-off HTML when possible.
- Do not invent unsupported Betamax commands.
- Do not reintroduce `record`, `serve`, or `publish` as primary workflows.
- Make Quick Start procedural and concise.
- Make reference pages modular and table-friendly.
- Keep examples copyable.
- Keep docs under `/betamax` base path compatible with relative links.

Useful component names to create or reuse:

- `DocsPageHeader`
- `BetamaxStripes`
- `ManualSection`
- `GuideStep`
- `CommandBlock`
- `ManualNote`
- `ReferenceModule`
- `SpecTable`
- `ExamplePreview`
- `PageNav`

These components should map cleanly to Starlight/Astro/MDX. If Starlight defaults conflict with this
design, override theme CSS rather than rewriting all content unless the page structure genuinely
needs custom components.

## Do's and Don'ts

### Do

- Do keep dark mode as the primary experience.
- Do use the stripe hamburger + Betamax text lockup in the top-left header.
- Do use the stripe rule under page titles.
- Do keep the main content darker than cream-paper mockups.
- Do make the page margin/outer background darker than the content surface.
- Do use subtle noise texture only.
- Do use warm cream text and analog orange/red accents.
- Do make code blocks and tables highly readable.
- Do use left sidebar grouping that feels like a manual index.
- Do let page layout vary by content type.

### Don't

- Don't use digital neon, cyan glow, glossy highlights, or futuristic UI effects.
- Don't use leather, heavy binder realism, obvious paper stains, or visible repeating patterns.
- Don't put terminal hardware around interior docs content.
- Don't use the standalone `B` symbol as a recurring ornament.
- Don't add a large stripe block to the sidebar bottom.
- Don't make cream/light mode the primary docs appearance.
- Don't let the right TOC stay by default on short pages if it adds clutter.
- Don't center all docs content like a marketing homepage.
- Don't reduce tables or code below comfortable reading sizes.

## Accessibility

- Maintain at least WCAG AA contrast for normal body text.
- Avoid placing small cream text over high-noise or high-contrast stripe backgrounds.
- Do not use color alone to communicate state; pair active states with position, border, icon, or
  label.
- Keep focus outlines visible and warm, e.g. amber outline with sufficient contrast.
- Respect reduced motion. Any stripe shimmer, hover movement, or texture animation should be
  disabled under `prefers-reduced-motion`.

## Implementation Notes

- Use CSS custom properties for all colors and spacing. Map these tokens into Starlight variables
  where possible.
- Add one global noise overlay using a small CSS gradient/noise asset or pseudo-element at very low
  opacity. Do not use a visible repeating texture.
- Use `background-color` layering rather than strong box shadows for hierarchy.
- Keep the homepage-specific terminal hardware and hero assets scoped to the splash page.
- Create custom MDX components only where the content structure benefits: guide steps, reference
  modules, command blocks, note cards, and page headers.
