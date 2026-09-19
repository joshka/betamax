# Reveal the result
# Hidden work reaches State and Screenshot, but never animated frames.
Output target/features/hidden-setup.gif
Output target/features/hidden-setup.png
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
Type "python3 \"$BETAMAX_FEATURE_ROOT/scripts/feature-scene.py\" visibility"
Enter
Wait+Line "HIDDEN SETUP"
State target/features/hidden-checkpoint.json
Screenshot target/features/hidden-checkpoint.png
Enter
Wait+Line "VISIBLE READY"
Show
State target/features/hidden-setup.json
Screenshot target/features/hidden-setup.png
Sleep 1s
