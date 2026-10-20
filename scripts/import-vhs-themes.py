#!/usr/bin/env python3
"""Import VHS theme names so `Set Theme "<vhs name>"` resolves in Betamax.

VHS's themes.json (charmbracelet/vhs) predates iTerm2-Color-Schemes' 2025 name normalization, so
many VHS names differ from Betamax's bundled Ghostty themes. This script regenerates:

- resources/vhs/aliases.json: VHS name -> Betamax theme name, for every VHS name that is not
  already an exact Ghostty theme name.
- resources/vhs/themes/<name>: VHS palettes with no Ghostty counterpart, in Ghostty's theme
  format, under a normalized name.

Rerun it after refreshing either side:

    scripts/import-vhs-themes.py path/to/vhs/themes.json

Manual decisions live in ALIASES and NAMES below; everything else is derived.
"""

import argparse
import json
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GHOSTTY = ROOT / "crates/betamax-core/resources/ghostty/themes"
OUT = ROOT / "crates/betamax-core/resources/vhs"

ANSI = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    "brightBlack", "brightRed", "brightGreen", "brightYellow",
    "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
]
CORE = ANSI + ["background", "foreground"]

# VHS names whose Ghostty counterpart can't be found by name or identical palette.
ALIASES = {
    "DraculaPlus": "Dracula+",
    "Monokai Pro (Filter Octagon)": "Monokai Pro Octagon",
    "Monokai Pro (Filter Ristretto)": "Monokai Pro Ristretto",
    "Tinacious Design (Dark)": "Tinacious Design Dark",
    "Tinacious Design (Light)": "Tinacious Design Light",
    "TokyoNightStorm": "TokyoNight Storm",
}

# Normalized names for VHS-only themes, following iTerm2-Color-Schemes' naming style.
NAMES = {
    "coolnight": "Coolnight",
    "CyberPunk2077": "Cyberpunk 2077",
    "h4rithd": "H4rithd",
    "h4rithd.com": "H4rithd.com",
    "LiquidCarbonTransparentInverse": "Liquid Carbon Transparent Inverse",
    "OneDark": "One Dark",
    "OneStar": "One Star",
    "TokyoNightLight": "TokyoNight Light",
}


def key(name):
    """Case- and punctuation-insensitive lookup key; matches `theme_key` in theme.rs."""
    return re.sub(r"[^a-z0-9+]", "", name.lower())


def ghostty_palette(path):
    colors = {}
    for line in path.read_text().splitlines():
        if m := re.match(r"\s*palette\s*=\s*(\d+)\s*=\s*#?(\w+)", line):
            colors[ANSI[int(m[1])]] = m[2].lower()
        elif m := re.match(r"\s*(background|foreground)\s*=\s*#?(\w+)", line):
            colors[m[1]] = m[2].lower()
    return tuple(colors.get(k) for k in CORE)


def vhs_palette(theme):
    return tuple(theme[k].lower().lstrip("#") for k in CORE)


def normalize(name):
    return NAMES.get(name) or re.sub(r"[_-]+", " ", name[:1].upper() + name[1:])


def ghostty_source(vhs_name, theme, revision):
    lines = [f"# Imported from VHS themes.json ({revision}) as \"{vhs_name}\"."]
    for credit in (theme.get("meta") or {}).get("credits") or []:
        lines.append(f"# Author: {credit['name']} <{credit['link']}>")
    lines += [f"palette = {i}={theme[k].lower()}" for i, k in enumerate(ANSI)]
    lines += [f"background = {theme['background'].lower()}"]
    lines += [f"foreground = {theme['foreground'].lower()}"]
    if theme.get("cursor"):
        lines += [f"cursor-color = {theme['cursor'].lower()}"]
    if theme.get("selection"):
        lines += [f"selection-background = {theme['selection'].lower()}"]
    return "\n".join(lines) + "\n"


def revision_of(path):
    result = subprocess.run(
        ["git", "-C", str(path.parent), "log", "-1", "--format=charmbracelet/vhs@%h", "--", path.name],
        capture_output=True, text=True,
    )
    return result.stdout.strip() or "charmbracelet/vhs"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("themes_json", type=Path, help="VHS themes.json")
    args = parser.parse_args()

    vhs = json.loads(args.themes_json.read_text())
    revision = revision_of(args.themes_json.resolve())
    ghostty = {p.name: ghostty_palette(p) for p in GHOSTTY.iterdir() if p.is_file()}
    by_key = {key(name): name for name in ghostty}
    by_palette = {}
    for name in sorted(ghostty):
        by_palette.setdefault(ghostty[name], name)

    aliases, bundled = {}, {}
    for theme in vhs:
        name = theme["name"]
        if name in ghostty:
            continue
        target = ALIASES.get(name) or by_key.get(key(name)) or by_palette.get(vhs_palette(theme))
        if target is None:
            target = normalize(name)
            bundled[target] = ghostty_source(name, theme, revision)
        assert target in ghostty or target in bundled, f"{name} -> missing {target}"
        if target != name:
            aliases[name] = target

    themes_dir = OUT / "themes"
    shutil.rmtree(themes_dir, ignore_errors=True)
    themes_dir.mkdir(parents=True)
    for name, source in bundled.items():
        (themes_dir / name).write_text(source)
    document = {
        "source": f"{revision} themes.json",
        "aliases": dict(sorted(aliases.items(), key=lambda item: item[0].lower())),
    }
    (OUT / "aliases.json").write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n")
    print(f"{len(aliases)} aliases, {len(bundled)} bundled VHS themes")


if __name__ == "__main__":
    main()
