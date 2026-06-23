Output examples/output/presentation-overlays.gif
Output examples/output/presentation-overlays.png

Require printf

Set Shell "bash"
Set Theme "GitHub Dark"
Set FontSize 24
Set Width 1040
Set Height 600
Set Margin 16
Set WindowBar Colorful
Set BorderRadius 8
Set TypingSpeed 35ms
Set CursorBlink false
Set KeyboardOverlay Input
Set KeyboardOverlayLocation CaptionRow

Caption "Caption and keyboard chips stay below the terminal"
Type "printf 'hotbar: [F1 Help] [F2 Open] [F3 Save]\n'"
Enter
Wait+Screen "hotbar"

Sleep 900ms

Caption "The bottom terminal row remains visible"
Type "echo done"
Enter
Wait+Screen "done"

Sleep 900ms
