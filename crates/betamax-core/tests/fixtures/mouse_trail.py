"""Mouse trail probe: draws @ at each reported pointer cell, with a move counter and elapsed ms.

Used by `tests/mouse_timing.rs` and handy for checking where and when mouse input lands in a
recording. Standard library only; press q to quit."""
import os, re, sys, termios, tty, time
fd = sys.stdin.fileno()
old = termios.tcgetattr(fd)
tty.setraw(fd)
out = sys.stdout
out.write("\x1b[?1049h\x1b[?25l\x1b[?1003h\x1b[?1006h\x1b[2J\x1b[Hready")
out.flush()
moves = 0
t0 = None
buf = b""
try:
    while True:
        buf += os.read(fd, 4096)
        if b"q" in buf.split(b"\x1b")[0]:
            break
        while True:
            m = re.search(rb"\x1b\[<(\d+);(\d+);(\d+)([Mm])", buf)
            if not m:
                break
            buf = buf[m.end():]
            col, row = int(m[2]), int(m[3])
            moves += 1
            now = time.monotonic()
            t0 = t0 or now
            out.write(f"\x1b[{row};{col}H@\x1b[1;1Hmoves {moves:4d} t {int((now-t0)*1000):6d}ms   ")
            out.flush()
        if buf.endswith(b"q"):
            break
finally:
    out.write("\x1b[?1003l\x1b[?1006l\x1b[?1049l\x1b[?25h")
    out.flush()
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
