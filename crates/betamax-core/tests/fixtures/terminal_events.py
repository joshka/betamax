"""Independent PTY event reporter; standard library only, no Betamax encoder imports."""

import os
import re
import signal
import sys
import termios
import tty
import time


def emit(message):
    sys.stdout.write(message + "\r\n")
    sys.stdout.flush()


def resize(_signum=None, _frame=None):
    size = os.get_terminal_size()
    emit(f"resize {size.columns} {size.lines}")


def configure(mode):
    sys.stdout.write("\x1b[?1000l\x1b[?1002l\x1b[?1003l")
    if mode != "0":
        sys.stdout.write(f"\x1b[?{ {'1': 1000, '2': 1002, '3': 1003}[mode] }h")
    emit("mode " + mode)


def mouse(code, column, row, release=False):
    modifiers = "+".join(name for bit, name in [(4, "shift"), (8, "alt"), (16, "ctrl")] if code & bit) or "none"
    base = code & ~(4 | 8 | 16 | 32)
    if code & 64:
        action = ["up", "down", "left", "right"][base - 64]
        button = "wheel"
    else:
        action = "up" if release or base == 3 else "move" if code & 32 else "down"
        button = ["left", "middle", "right", "none"][base]
    emit(f"mouse {action} {button} {column} {row} {modifiers}")


original = termios.tcgetattr(sys.stdin.fileno())
tty.setraw(sys.stdin.fileno())
signal.signal(signal.SIGWINCH, resize)
try:
    sys.stdout.write("\x1b[?1006h\x1b[?1002h")
    emit("ready")
    resize()
    pending = b""
    while True:
        chunk = os.read(sys.stdin.fileno(), 4096)
        if not chunk:
            break
        pending += chunk
        while pending:
            if pending.startswith(b"\x1b[<"):
                match = re.match(rb"\x1b\[<(\d+);(\d+);(\d+)([Mm])", pending)
                if match is None:
                    break
                code, column, row = (int(value) for value in match.groups()[:3])
                mouse(code, column - 1, row - 1, match[4] == b"m")
                pending = pending[match.end():]
            elif pending.startswith(b"\x1b[M"):
                if len(pending) < 6:
                    break
                code, column, row = pending[3:6]
                mouse(code - 32, column - 33, row - 33)
                pending = pending[6:]
            elif pending.startswith(b"\x1b"):
                keys = {b"\x1b[A": "up", b"\x1b[B": "down", b"\x1b[C": "right", b"\x1b[D": "left"}
                if len(pending) < 3:
                    break
                sequence, pending = pending[:3], pending[3:]
                emit("key " + keys.get(sequence, sequence.hex()))
            else:
                key, pending = pending[:1], pending[1:]
                if key == b"q":
                    emit("bye")
                    sys.exit(0)
                elif key in (b"0", b"1", b"2", b"3"):
                    configure(key.decode())
                elif key == b"h":
                    sys.stdout.write("\x1b[2J\x1b[H\x1b[1;38;2;255;0;0mA界e\u0301\x1b[0m")
                    sys.stdout.flush()
                elif key == b"d":
                    sys.stdout.write("\x1b[2J\x1b[HTRANSIENT\x1b[2J\x1b[HFINAL")
                    sys.stdout.flush()
                elif key == b"o":
                    emit("armed")
                    # Keep the PTY readable even when a busy CI runner delays this process.
                    # Scheduled sleeps can accidentally exceed the assertion's quiet window.
                    deadline = time.monotonic() + 6
                    while time.monotonic() < deadline:
                        sys.stdout.write("\rbusy")
                        sys.stdout.flush()
                elif key == b"b":
                    emit("armed")
                    time.sleep(0.25)
                    emit("FORBIDDEN")
                elif key == b"x":
                    sys.stdout.write("\x1b[?1006l")
                    emit("format x10")
                elif key == b"s":
                    sys.stdout.write("\x1b[?1006h")
                    emit("format sgr")
                else:
                    emit("key " + key.hex())
finally:
    sys.stdout.write("\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l")
    sys.stdout.flush()
    termios.tcsetattr(sys.stdin.fileno(), termios.TCSANOW, original)
