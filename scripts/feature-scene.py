#!/usr/bin/env python3
"""Small deterministic terminal subject for the feature tapes; no third-party packages."""

import base64
import os
import struct
import sys
import termios
import tty
import zlib

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


def cell_edges():
    """Fill every terminal cell and connect a Unicode border at all four grid edges."""
    columns, rows = os.get_terminal_size()
    lines = ["┌" + "─" * (columns - 2) + "┐"]
    lines += ["│" + " " * (columns - 2) + "│" for _ in range(rows - 2)]
    lines += ["└" + "─" * (columns - 2) + "┘"]
    write(ESC + "?25l" + ESC + "48;2;40;140;90m" + ESC + "38;2;255;255;255m" + ESC + "2J")
    for row, line in enumerate(lines, 1):
        write(f"{ESC}{row};1H{line}")


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


# Swatch colors sit on the 6-level sixel palette so every protocol can draw them exactly; the
# gradient around them is gray, so a checker can count each protocol's pixels by color.
IMAGE_SWATCHES = {"kitty": (255, 102, 0), "placeholder": (0, 204, 255),
                  "sixel": (153, 255, 0), "iterm2": (204, 0, 255)}
IMAGE_SIZE = (168, 110)
# First kitty row/column diacritics (https://sw.kovidgoyal.net/kitty/graphics-protocol/).
DIACRITICS = "̅̍̎̐̒̽̾̿͆͊͋͌͐͑͒͗"


def image_pixels(swatch):
    width, height = IMAGE_SIZE
    rows = []
    for y in range(height):
        row = []
        for x in range(width):
            dx, dy = x - width // 2, y - height // 2
            if dx * dx + dy * dy < 34 * 34:
                row.append(swatch)
            else:
                level = 51 * ((x * 3 // width) + (y * 2 // height))
                row.append((level, level, level))
        rows.append(row)
    return rows


def png(rows):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))
    raw = b"".join(b"\0" + bytes(c for px in row for c in px) for row in rows)
    header = struct.pack(">IIBBBBB", len(rows[0]), len(rows), 8, 2, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw))
            + chunk(b"IEND", b""))


def sixel(rows):
    width, height = len(rows[0]), len(rows)
    out = [f'\033P0;1;0q"1;1;{width};{height}']
    palette = {}
    for row in rows:
        for px in row:
            if px not in palette:
                palette[px] = len(palette)
                out.append("#%d;2;%d;%d;%d" % ((palette[px],) + tuple(c * 100 // 255 for c in px)))
    for band in range(0, height, 6):
        bits = {}
        for x in range(width):
            for bit in range(min(6, height - band)):
                color = palette[rows[band + bit][x]]
                bits.setdefault(color, [0] * width)[x] |= 1 << bit
        for color, values in bits.items():
            out.append(f"#{color}" + "".join(chr(63 + v) for v in values) + "$")
        out.append("-")
    return "".join(out) + "\033\\"


def kitty_chunks(control, payload):
    data = base64.b64encode(payload).decode()
    parts = [data[i:i + 4096] for i in range(0, len(data), 4096)]
    out = ""
    for index, part in enumerate(parts):
        more = int(index + 1 < len(parts))
        head = f"{control},m={more}" if index == 0 else f"m={more}"
        out += f"\033_G{head};{part}\033\\"
    return out


def images():
    write(ESC + "2J" + ESC + "H" + ESC + "?25l")
    write("\033[1;38;2;94;234;212mBETAMAX  /  FIELD GUIDE\033[0m\r\n\r\n")
    write("\033[1mPictures in the terminal\033[0m\r\nOne image per protocol, drawn by the program.")
    columns, rows = 14, 5
    panels = [("kitty", "kitty"), ("placeholder", "placeholders"), ("sixel", "sixel"),
              ("iterm2", "iTerm2")]
    for index, (name, label) in enumerate(panels):
        x = 3 + index * 17
        write(f"{ESC}13;{x}H{ESC}0m{label}{ESC}7;{x}H")
        pixels = image_pixels(IMAGE_SWATCHES[name])
        if name == "kitty":
            write(kitty_chunks(f"a=T,f=100,q=2,c={columns},r={rows}", png(pixels)))
        elif name == "placeholder":
            write(kitty_chunks(f"a=T,U=1,i=42,f=100,q=2,c={columns},r={rows}", png(pixels)))
            for row in range(rows):
                write(f"{ESC}{7 + row};{x}H{ESC}38;5;42m")
                write("".join("\U0010EEEE" + DIACRITICS[row] + DIACRITICS[col]
                              for col in range(columns)) + ESC + "39m")
        elif name == "sixel":
            write(sixel(pixels))
        else:
            encoded = base64.b64encode(png(pixels)).decode()
            write(f"\033]1337;File=inline=1;width={columns};height={rows}:{encoded}\a")
    write(f"{ESC}15;1H\r\nIMAGES READY")


if __name__ == "__main__":
    mode = termios.tcgetattr(sys.stdin)
    mode[3] &= ~termios.ECHO
    termios.tcsetattr(sys.stdin, termios.TCSANOW, mode)
    {"styles": styles, "layout": layout, "visibility": visibility,
     "scrollback": scrollback, "input": input_scene, "cell-edges": cell_edges, "images": images}[sys.argv[1]]()
    # Keep static scenes alive so shell prompts cannot alter checkpoints.
    if sys.argv[1] in {"styles", "layout", "cell-edges", "images"}:
        input()
