#!/usr/bin/env python3
"""Verify the pinned consumer's source preview and live resize checkpoints."""

import json
from pathlib import Path
import unicodedata

ROOT = Path(__file__).resolve().parent.parent
CHECKPOINTS = ROOT / "target/terminal-input"


def state(name):
    return json.loads((CHECKPOINTS / f"{name}.json").read_text())


def preview(name):
    lines = state(name)["viewport_text"].splitlines()
    line = next(line for line in lines if "Text preview: " in line)
    text = line.split("Text preview: ", 1)[1]
    # State JSON records a space for every wide-cell continuation. Remove exactly that cell;
    # preserve any real source space following it. The pinned preview escapes combining/ZWJ
    # characters, so the remaining wide scalars each occupy two cells independently.
    result = []
    index = 0
    while index < len(text):
        char = text[index]
        result.append(char)
        index += 1
        if unicodedata.east_asian_width(char) in ("W", "F"):
            assert text[index:index + 1] == " ", f"missing continuation for {char!r}"
            index += 1
    return "".join(result)


def rust_debug(text):
    result = '"'
    for char in text:
        result += {
            "\n": "\\n", "\r": "\\r", "\t": "\\t", '"': '\\"', "\\": "\\\\",
            "\u0301": "\\u{301}", "\u200d": "\\u{200d}",
        }.get(char, char)
    return result + '"'


source = (ROOT / "examples/fixtures/terminal-ux-unicode-source.txt").read_text().rstrip("\n")
assert preview("unicode-title") == rust_debug(source.splitlines()[0])
assert preview("unicode-selection") == rust_debug(source), preview("unicode-selection")
assert preview("unicode-selection").count("\\n") == 2, "display wraps must not add source newlines"
assert state("diff-resize-50x16")["size"] == [50, 16]
assert state("diff-resize-restored")["size"] == [92, 25]
assert preview("diff-resize-50x16") == '"1500"'
assert preview("diff-resize-restored") == '"1500"'
assert "check 44: verified" in state("multi-file-end")["viewport_text"]
print("terminal UX: exact Unicode source, source newlines, resize anchors, and multi-file scrolling pass")
