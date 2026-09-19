# Hide shell setup so playback measures only the two visible holds.
Require printf
Set Shell "bash"
Set Width 480
Set Height 240
Set FontSize 20
Set Framerate 20
Set PlaybackSpeed 1
Set TypingSpeed 0ms
Set CursorBlink false

Hide
Type "printf '\033[?25l\033[41m\033[2J\033[HRED_READY\n'"
Enter
Wait+Screen /^RED_READY/
Show
Sleep 1s
Hide
Type "printf '\033[44m\033[2J\033[HBLUE_READY\n'"
Enter
Wait+Screen /^BLUE_READY/
Show
Sleep 2s
Hide
Type "exit"
Enter
