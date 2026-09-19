#!/usr/bin/env python3
"""Build a standalone, script-free gallery from renderer PNG/JSON checkpoints."""

import argparse
import base64
import html
import json
import os
from pathlib import Path


LABELS = {
    "layout": "Full-screen file picker — selected row",
    "layout-cleared": "Partial redraw — cleared selection",
    "unicode": "Wide characters and combining marks",
    "unicode-fragmented": "Unicode — byte-by-byte input",
    "primary": "Primary screen before alternate screen",
    "alternate": "Alternate screen",
    "restored": "Restored primary screen",
    "inverse-hidden": "Inverse colors — hidden cursor",
    "cursor-block": "Visible block cursor",
    "wide-glyph": "Complete wide glyph and following column",
    "wide-styled": "Wide glyph with adjacent background colors",
    "wide-cursor": "Cursor over the wide continuation cell",
    "wide-replace-before": "Wide glyph before replacement",
    "wide-replace": "Replace the leading cell",
    "wide-erase-before": "Wide glyph before erasure",
    "wide-erase": "Erase both occupied cells",
    "wide-erase-continuation-before": "Wide glyph before continuation erasure",
    "wide-erase-continuation": "Erase the continuation cell",
    "wide-replace-continuation-before": "Wide glyph before continuation replacement",
    "wide-replace-continuation": "Replace the continuation cell",
}
EXPECTED = list(LABELS)


def build_gallery(source, destination, platform, outcome, revision):
    """Return checkpoint count; keep incomplete pairs useful and absence explicit."""
    names = {p.stem for p in source.glob("*.png")} | {
        p.stem for p in source.glob("*.json")
    }
    if not names:
        destination.unlink(missing_ok=True)
        return 0

    escape = html.escape
    missing = [name for name in EXPECTED if name not in names]
    cards = []
    ordered = [name for name in LABELS if name in names] + sorted(names - LABELS.keys())
    for name in ordered:
        label = escape(LABELS.get(name, name))
        image_path = source / f"{name}.png"
        state_path = source / f"{name}.json"
        image = '<p class="warning">PNG checkpoint unavailable.</p>'
        if image_path.is_file():
            encoded = base64.b64encode(image_path.read_bytes()).decode("ascii")
            image = f'<img alt="{label}" src="data:image/png;base64,{encoded}">'
        state = "JSON checkpoint unavailable."
        if state_path.is_file():
            try:
                state = json.dumps(json.loads(state_path.read_text()), indent=2, ensure_ascii=False)
            except (ValueError, UnicodeError):
                state = "Invalid JSON checkpoint:\n" + state_path.read_text(errors="replace")
        cards.append(f'<section><h2>{label}</h2><p><code>{escape(name)}</code></p>{image}'
                     f'<details><summary>State JSON</summary><pre>{escape(state)}</pre></details></section>')

    missing_notice = (f'<p class="warning">Missing checkpoints: {escape(", ".join(missing))}.</p>'
                      if missing else "")
    document = f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Renderer fixtures — {escape(platform)}</title>
<style>
body {{font: 16px/1.5 system-ui, sans-serif; background: #f4f6fa; color: #182338;
       max-width: 1100px; margin: auto; padding: 24px;}}
h1 {{margin-bottom: 4px;}} h2 {{font-size: 1.15rem;}}
.warning {{background: #fff2ce; border-left: 4px solid #a86b00; padding: 12px;}}
.gallery {{display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 420px), 1fr)); gap: 20px;}}
section {{background: white; padding: 16px; border: 1px solid #c8d1df; border-radius: 8px; min-width: 0;}}
img {{display: block; max-width: 100%; height: auto;}}
summary {{cursor: pointer; font-weight: bold; margin-top: 16px;}}
pre {{overflow: auto; max-height: 32rem; padding: 12px; background: #eef1f6; font-size: 13px;}}
code {{overflow-wrap: anywhere;}}
</style></head><body>
<h1>Renderer fixtures — {escape(platform)}</h1>
<p>Revision: <code>{escape(revision)}</code> · Test step: <strong>{escape(outcome)}</strong>
· {len(names)} checkpoints</p>
<p>Captured output, not an assertion that every pictured behavior is correct.
A failed or interrupted test run may leave partial checkpoints.</p>
<p>Wide-glyph acceptance checks run in normal CI: the selected fallback must cover 界,
ink must span both cells, and edits, adjacent backgrounds, and cursor overlays are checked.
Missing CJK font coverage fails the suite with a setup error.</p>
{missing_notice}<main class="gallery">{"".join(cards)}</main>
</body></html>
'''
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(document, encoding="utf-8")
    return len(names)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--outcome", default="not recorded")
    parser.add_argument("--revision", default="local")
    args = parser.parse_args()
    count = build_gallery(args.source, args.destination, args.platform, args.outcome, args.revision)
    print(f"Wrote {count} checkpoints to {args.destination}" if count else "No renderer checkpoints available.")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            stream.write(f"available={'true' if count else 'false'}\n")


if __name__ == "__main__":
    main()
