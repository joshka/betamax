#!/usr/bin/env python3
"""Render/check feature tapes and build a portable gallery, using only Python and ffmpeg."""

import argparse
import base64
import html
import json
import os
from pathlib import Path
import subprocess
import struct
import sys

ROOT = Path(__file__).resolve().parent.parent
SCENES = json.loads((ROOT / "scripts/feature-gallery.json").read_text())
WIDTH, HEIGHT = 900, 540


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def decode(path, all_frames=False):
    if path.suffix == ".png":
        data = path.read_bytes()
        require(data[:8] == b"\x89PNG\r\n\x1a\n", f"Invalid PNG: {path}")
        require(struct.unpack(">II", data[16:24]) == (WIDTH, HEIGHT), f"PNG dimensions: {path}")
    frames = [] if all_frames else ["-frames:v", "1"]
    result = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(path), *frames, "-f", "rawvideo",
         "-pix_fmt", "rgb24", "-"], check=True, capture_output=True, timeout=30,
    )
    size = WIDTH * HEIGHT * 3
    require(result.stdout and len(result.stdout) % size == 0, f"Invalid frame size: {path}")
    return [result.stdout[i:i + size] for i in range(0, len(result.stdout), size)]


def pixel(frame, x, y):
    offset = (y * WIDTH + x) * 3
    return tuple(frame[offset:offset + 3])


def count_color(frame, predicate):
    return sum(predicate(r, g, b) for r, g, b in zip(frame[::3], frame[1::3], frame[2::3]))


def check(source):
    def state(name):
        return json.loads((source / f"{name}.json").read_text())

    def frame(name):
        return decode(source / f"{name}.png")[0]

    for scene in SCENES:
        name = scene["slug"]
        metadata = json.loads(subprocess.check_output(
            ["ffprobe", "-v", "error", "-show_streams", "-of", "json", str(source / f"{name}.gif")]
        ))["streams"][0]
        require((metadata["width"], metadata["height"]) == (WIDTH, HEIGHT), name)
        require(float(metadata["duration"]) >= 0.5, f"Empty/short animation: {name}")
        for checkpoint in scene["checkpoints"]:
            require(state(checkpoint)["size"][0] >= 55, f"Unexpected grid: {checkpoint}")
            frame(checkpoint)

    edited = state("input-and-keys")
    require("Environment  field-guide" in edited["viewport_text"], "Environment was not injected")
    require("Accepted  release-candidate-1" in edited["viewport_text"], "Typing/paste/edit result")
    require("bytes=1b5b42,1b5b42,09,10,0d" in edited["viewport_text"], "Key bytes or repeat count")
    require(state("input-cursor")["cursor"]["visible"], "Editing cursor missing")
    require(not edited["cursor"]["visible"], "Completion cursor not hidden")

    specimen = state("text-and-unicode")
    require("café  /  e\u0301  /  日 本 語   /  λ → ∞" in specimen["viewport_text"], "Unicode changed")
    for label, attribute, expected in [("Bold", "bold", True), ("Italic", "italic", True),
                                       ("Strike", "strikethrough", True), ("Inverse", "inverse", True),
                                       ("Faint", "faint", True), ("Hidden", "invisible", True),
                                       ("Underline", "underline", "single")]:
        require(any(isinstance(span, list) and label in span[0]
                    and specimen["styles"][span[1]].get(attribute) == expected
                    for row in specimen["viewport"] for span in row), f"Missing {label} style")
    require(any(style.get("fg") == "#5eead4" for style in specimen["styles"]), "Truecolor state")
    ink = count_color(frame("text-and-unicode"), lambda r, g, b: r < 130 and g > 180 and b > 140)
    require(ink > 800, f"Missing mint glyphs/blocks: {ink}")

    dark, light = state("layout-dark"), state("layout-light")
    require(dark["viewport_text"] == light["viewport_text"], "Theme changed content")
    require(dark["size"] == light["size"], "Theme changed grid")
    for name, background in [("layout-dark", (30, 30, 46)), ("layout-light", (255, 255, 255))]:
        rendered = frame(name)
        require(pixel(rendered, 4, 270) == (17, 24, 39), f"Margin: {name}")
        require(pixel(rendered, 450, 510) == background, f"Theme background: {name}")
        require(pixel(rendered, 16, 16) == (17, 24, 39), f"Rounded corner: {name}")
        require(pixel(rendered, 450, 20) != (17, 24, 39), f"Panel top: {name}")
    # Colorful window controls occupy opposite sides in the two layouts.
    require(pixel(frame("layout-dark"), 32, 32) != (30, 30, 46), "Left window controls")
    require(any(pixel(frame("layout-light"), x, 32) != (255, 255, 255)
                for x in range(810, 870)), "Right window controls")

    clean, caption, overlay = [state(n) for n in
                               ["presentation-clean", "presentation-caption", "presentation-annotated"]]
    require(clean == caption == overlay, "Presentation changed terminal state")
    images = [frame(n) for n in ["presentation-clean", "presentation-caption", "presentation-annotated"]]
    boundary = WIDTH * 480 * 3
    require(images[0][:boundary] == images[1][:boundary] == images[2][:boundary],
            "Presentation overwrote terminal canvas")
    require(images[0][boundary:] != images[1][boundary:], "Caption did not render")
    require(images[1][boundary:] != images[2][boundary:], "Keyboard chip did not render")

    require(state("presentation-cleared") == clean, "Clearing caption changed state")
    require(frame("presentation-cleared") == images[0], "Caption/chip did not clear")

    magenta = lambda r, g, b: r > 220 and g < 40 and b > 220
    require(count_color(frame("hidden-checkpoint"), magenta) > 100000, "Hidden screenshot missing")
    require("HIDDEN SETUP" in state("hidden-checkpoint")["viewport_text"], "Hidden State missing")
    require("HIDDEN SETUP" not in state("hidden-setup")["viewport_text"], "Setup not replaced")
    for index, rendered in enumerate(decode(source / "hidden-setup.gif", all_frames=True)):
        require(count_color(rendered, magenta) == 0, f"Hidden setup leaked into frame {index}")

    primary, alternate, restored = [state(n) for n in
                                     ["archive-primary", "archive-alternate", "scrollback-and-screens"]]
    require(primary["scrollback_rows"] > 0, "No scrollback captured")
    history = primary["scrollback_text"] + primary["viewport_text"]
    for number in range(1, 25):
        require(f"ARCHIVE {number:02d}" in history, f"Missing archive entry {number}")
    require("ALTERNATE READY" in alternate["viewport_text"], "Alternate screen missing")
    require("ARCHIVE" not in alternate["viewport_text"], "Primary leaked into alternate screen")
    require(primary == restored, "Alternate-screen exit did not restore exact state")
    require(frame("archive-primary") == frame("scrollback-and-screens"), "Restored pixels changed")
    return [scene["title"] + ": " + scene["expected"] for scene in SCENES]


def gallery(source, outcome, diagnostics):
    escape = html.escape

    def media(name, extension):
        path = source / f"{name}.{extension}"
        if not path.exists():
            return '<p class="missing">Checkpoint unavailable</p>'
        encoded = base64.b64encode(path.read_bytes()).decode()
        return f'<img alt="{escape(name)}" src="data:image/{extension};base64,{encoded}">'

    cards = []
    for scene in SCENES:
        name = scene["slug"]
        comparisons = "".join(f'<figure>{media(n, "png")}<figcaption>{escape(n)}</figcaption></figure>'
                              for n in scene["checkpoints"])
        tape = (ROOT / f"examples/features/{name}.tape").read_text()
        cards.append(f'<section id="{name}"><h2>{escape(scene["title"])}</h2>'
                     f'<p>{escape(scene["description"])}</p>{media(name, "gif")}'
                     f'<p><strong>Expected:</strong> {escape(scene["expected"])}</p>'
                     f'<details><summary>Compare named checkpoints</summary><div class="pairs">{comparisons}</div></details>'
                     f'<details><summary>Reproduce · {name}.tape</summary><pre>{escape(tape)}</pre></details></section>')
    provenance = f'{sys.platform} · {os.environ.get("GITHUB_SHA", "local working copy")}'
    (source / "feature-gallery.html").write_text('''<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Betamax · Field guide</title><style>
:root {color-scheme:dark} body {font:17px/1.6 system-ui,sans-serif;background:#111827;color:#e5e7eb;
max-width:1100px;margin:auto;padding:32px} h1 {font-size:40px;line-height:1.2} h2 {color:#5eead4}
section {padding:24px;margin:32px 0;border:1px solid #374151;border-radius:16px;background:#171f30}
img {display:block;max-width:100%;height:auto;border-radius:8px} pre {white-space:pre-wrap;overflow:auto;
font-size:13px} summary {cursor:pointer;color:#5eead4;margin:16px 0} .pairs {display:grid;
grid-template-columns:repeat(auto-fit,minmax(min(100%,420px),1fr));gap:16px} figure {margin:0}
figcaption {font:14px monospace;overflow-wrap:anywhere} .missing {color:#fda4af} a {color:#5eead4}
</style><header><p>BETAMAX / FIELD GUIDE</p><h1>Terminal features you can see.</h1>
<p>Seven small scenes, shared visual language, explicit expected results. Animations demonstrate
behavior; named checkpoints make before/after comparisons inspectable.</p></header>'''
        + f'<p>{escape(provenance)} · Assertions: <strong>{escape(outcome)}</strong></p>'
        + f'<details><summary>Assertion results</summary><pre>{escape(diagnostics)}</pre></details>'
        + ''.join(cards) + '</html>')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--render", metavar="CLI", help="render all tapes with this locally built CLI")
    parser.add_argument("--media-directory", type=Path, help="action output directory with manifest.json")
    args = parser.parse_args()
    source = ROOT / "target/features"
    source.mkdir(parents=True, exist_ok=True)
    outcome, diagnostics = "FAILED", ""
    try:
        if args.render:
            # Remove only this suite's known artifacts so a partial rerun cannot show stale evidence.
            names = {scene["slug"] for scene in SCENES}
            names.update(name for scene in SCENES for name in scene["checkpoints"])
            for name in names:
                for extension in ("png", "gif", "json"):
                    (source / f"{name}.{extension}").unlink(missing_ok=True)
            binary = str(Path(args.render).resolve())
            env = {**os.environ, "BETAMAX_FEATURE_ROOT": str(ROOT)}
            for scene in SCENES:
                print(f'Rendering {scene["title"]}', flush=True)
                subprocess.run([binary, "run", "--quiet", f'examples/features/{scene["slug"]}.tape'],
                               cwd=ROOT, env=env, check=True, timeout=90)
        if args.media_directory:
            manifest = json.loads((args.media_directory / "manifest.json").read_text())
            require(not manifest["problems"], manifest["problems"])
            expected = {f'examples/features/{scene["slug"]}.tape' for scene in SCENES}
            require({row["tape"] for row in manifest["results"]} == expected, "Scene inventory")
            require(all(row["status"] == "passed" for row in manifest["results"]), "Scene failure")
            for scene in SCENES:
                prefix = f'examples/features/{scene["slug"]}.tape ('
                items = [m for m in manifest["media"] if m["label"].startswith(prefix)]
                require({m["type"] for m in items} == {"image/gif", "image/png"}, scene["slug"])
                for item in items:
                    if item["type"] == "image/gif":
                        (source / f'{scene["slug"]}.gif').write_bytes((args.media_directory / item["name"]).read_bytes())
        diagnostics = "\n".join(check(source))
        outcome = "PASSED"
        print(diagnostics)
    except Exception as error:
        diagnostics = str(error)
        raise
    finally:
        gallery(source, outcome, diagnostics)


if __name__ == "__main__":
    main()
