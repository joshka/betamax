# Real PTY event decoder. Run from the Betamax checkout root.
Require python3
Set Shell "python3 crates/betamax-core/tests/fixtures/terminal_events.py"
Set Width 1000
Set Height 480
Set FontFamily "JetBrains Mono"
Set FontSize 20
Set Padding 16
Set Theme "Aardvark Ink"
Set TypingSpeed 0ms
Set CursorBlink false
Hide
Wait+Screen@5s "ready"
Caption "Before: pointer at cell (0, 0), no buttons held"
Screenshot target/terminal-input/demo-before.png
MouseMove 4 2
MouseDown Left
Wait+Screen@5s "mouse down left 4 2 none"
MouseMove 8 3
Wait+Screen@5s "mouse move left 8 3 none"
MouseUp Left
Wait+Screen@5s "mouse up left 8 3 none"
Caption "After: left drag from cell (4, 2) to (8, 3)"
Screenshot target/terminal-input/demo-after.png
State target/terminal-input/demo-after.json
Type "q"
Wait+Screen@5s "bye"
