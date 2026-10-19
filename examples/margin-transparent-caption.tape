# Captions and keyboard chips over a clear backdrop sit on a theme-colored plate.
Output examples/output/margin-transparent-caption.png
Output examples/output/margin-transparent-caption.gif

Set Preset Framed
Set Shell "bash"
Set MarginFill transparent
Set Width 960
Set Height 540
Set CursorBlink false
Set KeyboardOverlay Input
Set KeyboardOverlayLocation CaptionRow

Caption "Captions stay readable on any page"
Type "echo hello"
Enter
Wait+Screen "hello"
Sleep 600ms
