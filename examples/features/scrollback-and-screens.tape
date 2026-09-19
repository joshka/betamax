# Keep the history
# Scrollback survives alternate-screen entry and restoration.
Output target/features/scrollback-and-screens.gif
Output target/features/scrollback-and-screens.png
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

Hide
Type "python3 \"$BETAMAX_FEATURE_ROOT/scripts/feature-scene.py\" scrollback"
Enter
Wait+Line "ARCHIVE READY"
Show
State target/features/archive-primary.json
Screenshot target/features/archive-primary.png
Sleep 600ms
Enter
Wait+Line "ALTERNATE READY"
State target/features/archive-alternate.json
Screenshot target/features/archive-alternate.png
Sleep 800ms
Enter
Wait+Screen "ARCHIVE READY"
State target/features/scrollback-and-screens.json
Screenshot target/features/scrollback-and-screens.png
Sleep 600ms
