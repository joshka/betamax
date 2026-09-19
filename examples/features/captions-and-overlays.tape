# Present the action
# Caption changes and keyboard chips leave terminal state untouched.
Output target/features/captions-and-overlays.gif
Output target/features/captions-and-overlays.png
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
Set KeyboardOverlay Keys
Set KeyboardOverlayLocation CaptionRow

Hide
Type "python3 \"$BETAMAX_FEATURE_ROOT/scripts/feature-scene.py\" layout"
Enter
Wait+Line "LAYOUT READY"
Show
Caption ""
State target/features/presentation-clean.json
Screenshot target/features/presentation-clean.png
Caption "Review the capture before publishing"
Screenshot target/features/presentation-caption.png
State target/features/presentation-caption.json
# Ctrl+L is observed as a key by the overlay; canonical input does not redraw this app.
Ctrl+L
Screenshot target/features/presentation-annotated.png
State target/features/presentation-annotated.json
Sleep 1700ms
Caption ""
Screenshot target/features/presentation-cleared.png
State target/features/presentation-cleared.json
Sleep 500ms
