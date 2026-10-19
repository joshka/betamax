# Captions and keyboard chips over a clear backdrop. WebP keeps smooth alpha edges; GIF, with
# single-bit transparency, backs the caption row with a theme-colored plate.
Output examples/output/margin-transparent-caption.gif
Output examples/output/margin-transparent-caption.webp

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
