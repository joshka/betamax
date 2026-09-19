# SGR 2 reduces foreground opacity on both dark and light backgrounds.
Output examples/output/faint-text.gif
Require printf
Set Shell bash
Set Width 560
Set Height 230
Set FontSize 24
Set Padding 24
Set CursorBlink false
Set TypingSpeed 0ms
Env PS1 ""

Hide
Type "printf '\033[2J\033[H\033[?25lNormal  Terminal text\n\033[2mFaint   Terminal text\033[22m\n\033[4;9;2mFaint + underline + strike\033[0m\n\033[38;2;32;48;64;48;2;240;232;216mNormal  Terminal text\n\033[2mFaint   Terminal text\033[0m\nREADY\n'"
Enter
Wait+Screen /READY/
Show
Screenshot examples/output/faint-text.png
State examples/output/faint-text.json
Sleep 1s
