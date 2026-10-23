"""Small full-screen task picker for the insta snapshot example: `j` moves the selection down."""

import sys
import tty

TASKS = ["write tape", "run test", "review snapshot"]


def draw(selected):
    rows = ["\x1b[1;97;44mTasks\x1b[0m"]
    for index, task in enumerate(TASKS):
        rows.append(f"\x1b[1;32m> {task}\x1b[0m" if index == selected else f"  {task}")
    rows += ["", "\x1b[2mj move · q quit\x1b[0m"]
    sys.stdout.write("\x1b[H\x1b[2J" + "\r\n".join(rows))
    sys.stdout.flush()


tty.setraw(sys.stdin.fileno())
# Alternate screen and hidden cursor, like most full-screen TUIs.
sys.stdout.write("\x1b[?1049h\x1b[?25l")
selected = 0
draw(selected)
while (key := sys.stdin.read(1)) != "q":
    if key == "j":
        selected = min(selected + 1, len(TASKS) - 1)
    draw(selected)
