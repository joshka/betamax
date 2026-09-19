# Room to breathe / light
# Matched content with padding, margins, window chrome and rounded corners.
Output target/features/layout-light.gif
Output target/features/layout-light.png
Env BETAMAX_ACCENT "15;118;110"
Require python3
Set Shell "bash"
Set Theme "GitHub Light Default"
Set FontFamily "DejaVu Sans Mono"
Set Width 900
Set Height 540
Set FontSize 20
Set LetterSpacing 0
Set LineHeight 1.2
Set Padding 32
Set Margin 16
Set MarginFill "#111827"
Set WindowBar ColorfulRight
Set WindowBarSize 32
Set BorderRadius 12
Set Framerate 10
Set TypingSpeed 0ms
Set CursorBlink false
Set WaitTimeout 5s

Hide
Type "python3 \"$BETAMAX_FEATURE_ROOT/scripts/feature-scene.py\" layout"
Enter
Wait+Line "LAYOUT READY"
Show
State target/features/layout-light.json
Screenshot target/features/layout-light.png
Sleep 1s
