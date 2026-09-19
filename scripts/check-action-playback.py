#!/usr/bin/env python3
"""Assert CLI tape state and decoded media without platform-specific font goldens."""

import json
from pathlib import Path
import subprocess
import sys


FORMATS = {
    "image/gif": "gif", "image/png": "png", "video/mp4": "h264", "video/webm": "vp9",
}
TAPES = {"examples/ci-playback.tape", "examples/ci-interaction.tape"}


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def probe(file):
    result = subprocess.run(
        ["ffprobe", "-v", "error", "-show_format", "-show_streams", "-of", "json", str(file)],
        check=True, capture_output=True, text=True, timeout=15,
    )
    return json.loads(result.stdout)


def pixels(file, timestamp=None):
    seek = [] if timestamp is None else ["-ss", str(timestamp)]
    result = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(file), *seek, "-frames:v", "1",
         "-vf", "fps=20,format=rgb24", "-f", "rawvideo", "-"],
        check=True, capture_output=True, timeout=15,
    )
    require(
        len(result.stdout) == 480 * 240 * 3,
        f"{file}: missing/wrong-size frame at {timestamp}",
    )
    return result.stdout


def background(file, timestamp=None):
    frame = pixels(file, timestamp)
    offset = (120 * 480 + 240) * 3
    return tuple(frame[offset:offset + 3])


def is_color(rgb, channel):
    return rgb[channel] > max(rgb[index] for index in range(3) if index != channel) + 50


def check_interaction(checkpoints):
    state = json.loads((checkpoints / "interaction.json").read_text())
    line = "ENV=injected INPUT=paste-ok"
    require(state["viewport_text"].splitlines() == [line, "READY", ">"], state["viewport_text"])
    require(not state["cursor"]["visible"], "ANSI cursor hide did not reach State")
    require(state["scrollback_rows"] == 0, "unexpected scrolling in compact fixture")
    spans = state["viewport"][0]
    require(len(spans) == 1 and isinstance(spans[0], list), spans)
    text, style_index = spans[0]
    style = state["styles"][style_index]
    require(text == line and style.get("bold") and style.get("fg") == "#ff5028", style)
    frame = pixels(checkpoints / "interaction.png")
    ink = check_ink(frame, checkpoints / "interaction.png")
    print(f"interaction: exact edited/pasted output, truecolor/bold state, {ink} orange glyph pixels")


def check_ink(frame, file):
    # Tolerate antialiasing, font differences, GIF quantization and lossy video encoding.
    ink = sum(
        r > 180 and 30 < g < 150 and b < 120
        for r, g, b in zip(frame[::3], frame[1::3], frame[2::3])
    )
    require(ink > 100, f"{file}: missing orange styled text ({ink} pixels)")
    return ink


def check(directory, checkpoints):
    manifest = json.loads((directory / "manifest.json").read_text())
    require(not manifest["problems"], manifest["problems"])
    rows = manifest["results"]
    require(len(rows) == len(TAPES) and {row["tape"] for row in rows} == TAPES, rows)
    require(all(row["status"] == "passed" for row in rows), rows)
    for tape in sorted(TAPES):
        media = [item for item in manifest["media"] if item["label"].startswith(f"{tape} (")]
        require(
            len(media) == len(FORMATS) and {item["type"] for item in media} == set(FORMATS),
            media,
        )
        for item in media:
            kind = item["type"]
            file = directory / item["name"]
            metadata = probe(file)
            stream = metadata["streams"][0]
            require((stream["width"], stream["height"]) == (480, 240), (file, stream))
            require(stream["codec_name"] == FORMATS[kind], (file, stream))
            if kind.startswith("video/"):
                require(stream["pix_fmt"] == "yuv420p", (file, stream))
            if tape.endswith("ci-interaction.tape"):
                check_ink(pixels(file, None if kind == "image/png" else 0.25), file)
                continue
            if kind == "image/png":
                color = background(file)
                require(is_color(color, 2), f"{file}: expected final visible blue PNG, got {color}")
                print(f"{file.name}: final PNG blue {color}")
                continue
            duration = float(metadata["format"]["duration"])
            require(2.9 <= duration <= 3.4, (file, duration))
            # Bracket the transition and include the final hold, catching frozen/truncated clips.
            for timestamp, channel in [(0.2, 0), (0.8, 0), (1.3, 2), (2.8, 2)]:
                color = background(file, timestamp)
                require(is_color(color, channel), f"{file} at {timestamp}s: wrong phase {color}")
            print(f"{file.name}: {duration:.3f}s, red through 0.8s, blue from 1.3s through 2.8s")
    check_interaction(checkpoints)


if __name__ == "__main__":
    checkpoints = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("target/ci-tapes")
    check(Path(sys.argv[1]), checkpoints)
