# Text with character
# Emphasis, ANSI and truecolor, combining marks, CJK and cell graphics.
Output target/features/text-and-unicode.gif
Output target/features/text-and-unicode.png
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
Set WaitPattern "STYLES READY"

Hide
Type "python3 \"$BETAMAX_FEATURE_ROOT/scripts/feature-scene.py\" styles"
Enter
Wait
Show
State target/features/text-and-unicode.json
Screenshot target/features/text-and-unicode.png
Sleep 1s
