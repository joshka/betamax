#!/usr/bin/env python3
"""Small deterministic terminal subject for the feature tapes; no third-party packages."""

import os
import sys
import termios
import tty

ESC = "\033["


def write(text):
    text = text.replace("38;2;94;234;212m", "38;2;" + os.environ.get("BETAMAX_ACCENT", "94;234;212") + "m")
    print(text, end="", flush=True)


def page(title, subtitle, body):
    write(ESC + "2J" + ESC + "H" + ESC + "?25l")
    write("\033[1;38;2;94;234;212mBETAMAX  /  FIELD GUIDE\033[0m\r\n\r\n")
    write(f"\033[1m{title}\033[0m\r\n{subtitle}\r\n\r\n{body}")


def styles():
    page("Text with character", "Color, emphasis and Unicode in one terminal.",
         "\033[1mBold\033[0m  \033[3mItalic\033[0m  \033[4mUnderline\033[0m  "
         "\033[9mStrike\033[0m\r\n"
         "\033[38;2;94;234;212mTruecolor mint\033[0m  \033[38;5;214mIndexed amber\033[0m\r\n"
         "\033[7m Inverse \033[0m  \033[2mFaint\033[0m  \033[8mHidden\033[0m visible\r\n\r\n"
         "┌────────────────────────────────────┐\r\n"
         "│ café  /  e\u0301  /  日本語  /  λ → ∞    │\r\n"
         "└────────────────────────────────────┘\r\n"
         "\033[38;2;94;234;212m████████\033[0m░░░░  2 of 3 complete\r\n\r\nSTYLES READY")


def layout():
    page("Room to breathe", "The same content, framed in two themes.",
         "┌────────────────────────────────────┐\r\n"
         "│  capture.tape                 2 KB │\r\n"
         "│  \033[38;2;94;234;212mreview.png\033[0m                  48 KB │\r\n"
         "│  release.webm               120 KB │\r\n"
         "└────────────────────────────────────┘\r\n\r\n"
         "  Padding keeps text clear of chrome.\r\n"
         "  A rounded frame separates the page.\r\n\r\nLAYOUT READY")


def visibility():
    write("\033[48;2;255;0;255m\033[2J\033[HHIDDEN SETUP")
    input()
    write("\033[0m")
    page("Reveal the result", "Setup executes while recording is hidden.",
         "\033[38;2;94;234;212m✓\033[0m Environment prepared\r\n"
         "\033[38;2;94;234;212m✓\033[0m Screen wait satisfied\r\n"
         "\033[38;2;94;234;212m✓\033[0m Only this result enters the animation\r\n\r\nVISIBLE READY")
    input()


def scrollback():
    write(ESC + "2J" + ESC + "H" + ESC + "?25l")
    for number in range(1, 25):
        write(f"  ARCHIVE {number:02d}  captured checkpoint\r\n")
    write("ARCHIVE READY")
    input()
    write(ESC + "?1049h")
    page("A temporary workspace", "Alternate screen keeps the archive intact.",
         "  Review the current checkpoint.\r\n"
         "  Return to restore the previous viewport.\r\n\r\nALTERNATE READY")
    input()
    write(ESC + "?1049l")
    input()


def input_scene():
    # A raw-mode key inspector makes received bytes observable, independent of readline versions.
    original = termios.tcgetattr(sys.stdin)
    tty.setraw(sys.stdin.fileno())
    try:
        value = ""
        keys = []
        page("From intent to input", "Type, paste, edit, then send a key sequence.",
             f"  Environment  {os.environ['BETAMAX_PROJECT']}\r\n\r\n  Input  ")
        write(ESC + "?25h")
        while True:
            byte = os.read(sys.stdin.fileno(), 1)
            if byte == b"\r":
                break
            if byte == b"\x7f":
                value = value[:-1]
            else:
                value += byte.decode("ascii")
            write("\r" + ESC + "2K  Input  " + value)
        write(ESC + "?25l" + "\r\n\r\n  Keys   ")
        # Read exact bytes rather than timing escape sequences.
        expected = [b"\x1b[B", b"\x1b[B", b"\t", b"\x10", b"\r"]
        for sequence in expected:
            received = b""
            while len(received) < len(sequence):
                received += os.read(sys.stdin.fileno(), len(sequence) - len(received))
            keys.append(received.hex())
        write("Down · Down · Tab · Ctrl+P · Enter\r\n\r\n")
        write(f"  Accepted  {value}\r\nINPUT READY\r\n")
        write("bytes=" + ",".join(keys))
        os.read(sys.stdin.fileno(), 1)
    finally:
        termios.tcsetattr(sys.stdin, termios.TCSADRAIN, original)


if __name__ == "__main__":
    mode = termios.tcgetattr(sys.stdin)
    mode[3] &= ~termios.ECHO
    termios.tcsetattr(sys.stdin, termios.TCSANOW, mode)
    {"styles": styles, "layout": layout, "visibility": visibility,
     "scrollback": scrollback, "input": input_scene}[sys.argv[1]]()
    # Keep static scenes alive so shell prompts cannot alter checkpoints.
    if sys.argv[1] in {"styles", "layout"}:
        input()
