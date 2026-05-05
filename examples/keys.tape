Output examples/output/keys.gif

Require printf

Set Shell "bash"
Set Theme "GitHub Dark"
Set FontSize 24
Set Width 940
Set Height 500
Set Margin 12
Set WindowBar Colorful
Set BorderRadius 8
Set TypingSpeed 30ms
Set CursorBlink false

Type "read -e line; printf 'edited=%s\n' \"$line\""
Enter
Sleep 300ms

Type "abcdef"
Left 3
Backspace
Delete
End
Enter
Wait+Screen "edited=abef"

Type "sleep 5"
Ctrl+C
Wait

Type "printf 'home/end, arrows, delete, ctrl-c\n'"
Enter
Wait+Screen "ctrl-c"

Sleep 700ms
Hide
Type "exit"
Enter
