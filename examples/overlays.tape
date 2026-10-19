# Captions and keyboard chips share a row below the terminal.
Output examples/output/overlays.webm
Output examples/output/overlays.png

Require clear
Set Shell "bash"
Set Width 960
Set Height 540
Set CursorBlink false
Set Framerate 12
Set TypingSpeed 40ms
Set WindowTitle "Captions and keyboard chips"
Set KeyboardOverlay Input
Set KeyboardOverlayLocation CaptionRow

Hide
Type "clear"
Enter
Wait
Show

Caption "Step 1: run a command"
Sleep 600ms
Type "echo hello"
Sleep 300ms
Enter
Wait
Sleep 1s

Caption "Step 2: interrupt a long task"
Type "sleep 10"
Enter
Sleep 800ms
Ctrl+C
Wait
Sleep 1500ms
