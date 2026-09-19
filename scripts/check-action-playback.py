#!/usr/bin/env python3
"""Check the action's PR-built CLI evidence, including decoded frames and timing."""

import json
from pathlib import Path
import subprocess
import sys


def check(directory):
    manifest = json.loads((directory / "manifest.json").read_text())
    assert not manifest["problems"], manifest["problems"]
    assert manifest["results"] and all(
        row["status"] == "passed" for row in manifest["results"]
    ), manifest["results"]
    media = {
        item["type"]: directory / item["name"]
        for item in manifest["media"]
        if item["label"].startswith("examples/ci-playback.tape (")
    }
    assert set(media) == {"image/gif", "image/png", "video/mp4", "video/webm"}, media
    for kind, file in media.items():
        if kind == "image/png":
            continue
        probe = subprocess.run(
            ["ffprobe", "-v", "error", "-show_format", "-of", "json", str(file)],
            check=True, capture_output=True, text=True,
        )
        duration = float(json.loads(probe.stdout)["format"]["duration"])
        # Three seconds of visible holds, plus Show frames and encoding quantization.
        assert 2.9 <= duration <= 3.4, (file, duration)
        colors = []
        for timestamp in (0.5, 2.0):
            decoded = subprocess.run(
                ["ffmpeg", "-v", "error", "-i", str(file), "-ss", str(timestamp),
                 "-frames:v", "1", "-vf", "fps=10,format=rgb24,crop=1:1:240:120",
                 "-f", "rawvideo", "-"],
                check=True, capture_output=True,
            )
            assert len(decoded.stdout) == 3, f"{file}: missing frame at {timestamp}s"
            colors.append(tuple(decoded.stdout))
        # Sample the blank terminal background, not text/cursor or compression noise.
        red, blue = colors
        assert red[0] > red[2] + 50 and blue[2] > blue[0] + 50, (
            f"{file}: expected red then blue, got {colors}"
        )
        print(f"{file.name}: {duration:.3f}s, red {red} -> blue {blue}")


if __name__ == "__main__":
    check(Path(sys.argv[1]))
