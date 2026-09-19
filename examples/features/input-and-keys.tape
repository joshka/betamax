# From intent to input
# Environment injection, clipboard editing, key repeats and modifiers.
Output target/features/input-and-keys.gif
Output target/features/input-and-keys.png
Env BETAMAX_PROJECT field-guide
Require python3
Set Shell "bash"
Set Theme "Catppuccin Mocha"
Set FontFamily "DejaVu Sans Mono"
Set Width 900
Set Height 540
Set FontSize 20
Set LetterSpacing 0
Set LineHeight 1.2
Set Padding 32
Set Margin 16
Set MarginFill "#111827"
Set WindowBar Colorful
Set WindowBarSize 32
Set BorderRadius 12
Set Framerate 10
Set TypingSpeed 0ms
Set CursorBlink false
Set WaitTimeout 5s
Set KeyboardOverlay Input
Set KeyboardOverlayLocation BottomRight

Hide
Type "python3 \"$BETAMAX_FEATURE_ROOT/scripts/feature-scene.py\" input"
Enter
Wait+Screen "Environment  field-guide"
Show
Type@60ms "release-"
Copy "candidate-X"
Paste
Backspace
Type "1"
Screenshot target/features/input-cursor.png
State target/features/input-cursor.json
Enter
Down@100ms 2
Tab
Ctrl+P
Enter
Wait+Screen "INPUT READY"
State target/features/input-and-keys.json
Screenshot target/features/input-and-keys.png
Sleep 800ms
