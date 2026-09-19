# Compare plain text with SGR 4 underline and SGR 9 strikethrough.
Output examples/output/text-decorations.gif

Require printf
Set Shell bash
Set Theme "Catppuccin Mocha"
Set Width 500
Set Height 220
Set FontSize 24
Set Padding 24
Set CursorBlink false
Set TypingSpeed 0ms
Env PS1 ""

Hide
Type "printf '\033[2J\033[H\033[?25lPlain text\n\033[4mUnderlined text\033[0m\n\033[9mStrikethrough text\033[0m\n\033[4;9mBoth decorations\033[0m\n'"
Enter
Wait+Screen /Both/
Show
Screenshot examples/output/text-decorations.png
State examples/output/text-decorations.json
Sleep 1s
