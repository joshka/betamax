# Unicode selection must retain source newlines while display lines wrap.
Set Shell "bash"
Set Width 1340
Set Height 680
Set FontFamily "JetBrains Mono"
Set FontSize 24
Set LetterSpacing 0
Set Padding 20
Set Theme "Aardvark Ink"
Set TypingSpeed 0ms
Set CursorBlink false
Hide
Type "exec \"$BETAMAX_DIFF_VIEWER\" --fixture unicode --aardvark-ink"
Enter
Wait+Screen@5s "café"
Resize 50 16
Type "w"
Type "v"
Home
End
Type "c"
Resize 250 25
State target/terminal-input/unicode-title.json
Resize 50 16
Down
End
Type "c"
Down
End
Type "c"
Resize 250 25
State target/terminal-input/unicode-selection.json
Resize 50 16
AssertText "Text preview:"
Caption "Unicode source selection across wrapped display lines"
Screenshot target/terminal-input/unicode-selection.png
Type "q"
