"""Controlled title-only bursts with screen acknowledgements for PTY integration tests."""
import os
import sys
import tty

tty.setraw(0)
if len(sys.argv) > 1:
    os.write(1, b'\x1b]2;startup title\x07')
os.write(1, b'\x1b[?25lready')
for_command = {
    b'a': b'\x1b]0;title set by app\x07\r\napp-ready',
    b'b': b'\x1b]2;title updated by app\x1b\\\r\nupdate-ready',
    b'r': b'\x1b]2;title updated by app\x07\r\nrestored-ready',
    b'e': (b'\x1b]2;title set by app\x07\x1b]2;title updated by app\x07'
           b'\x1b]2;title set by app\x07\r\nburst-ready'),
    b'c': b'\x1b]2;\x07\r\nclear-ready',
}
while command := os.read(0, 1):
    if command == b'q':
        break
    os.write(1, for_command[command])
