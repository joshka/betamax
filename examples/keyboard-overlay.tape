Output examples/output/keyboard-overlay.gif
Output examples/output/keyboard-overlay.png

Require printf

# Preserve the curated website/README presentation on regeneration.
Set Preset Default-0.1
Set Shell "bash"
Set Theme "GitHub Dark"
Set FontSize 24
Set Width 1040
Set Height 560
Set Margin 12
Set WindowBar Colorful
Set BorderRadius 8
Set TypingSpeed 35ms
Set CursorBlink false
Set KeyboardOverlay Input
Set KeyboardOverlayLocation BottomRight

Type "echo foo"
Enter
Wait+Screen "foo"

Sleep 1800ms

Type "echo bar"
Enter
Wait+Screen "bar"

Sleep 900ms
