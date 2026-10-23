# Pictures in the terminal
# Kitty graphics, kitty unicode placeholders, sixel and iTerm2 inline images.
Output target/features/inline-images.gif
Output target/features/inline-images.png
Require python3
# Keep exact pixel fixtures independent of the implicit presentation style.
Set Preset Default-0.1
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
Set WaitPattern "IMAGES READY"

Hide
Type "python3 \"$BETAMAX_FEATURE_ROOT/scripts/feature-scene.py\" images"
Enter
Wait
Show
State target/features/inline-images.json
Screenshot target/features/inline-images.png
Sleep 1s
