Output examples/output/keyboard-overlay-top-left.png

Require printf

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
Set KeyboardOverlayLocation TopLeft

Type "echo foo"
Enter
Wait+Screen "foo"

Type "echo bar"
Enter
Wait+Screen "bar"
