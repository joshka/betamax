# Multiple files and hunks exercise navigation, scrolling, and resize clamping.
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
Type "exec \"$BETAMAX_DIFF_VIEWER\" --fixture multi-file --aardvark-ink"
Enter
Wait+Screen@5s "src/routes.txt"
Type "/revised"
Enter
AssertText "1/9"
Type "f"
AssertText "2/9"
Type "f"
AssertText "3/9"
Type "f"
AssertText "4/9"
AssertText "setting 04: revised"
Type "s"
AssertText "4/9 · Split"
Resize 50 16
AssertText "4/9 · Split"
Resize 50 5
AssertText "4/9"
Resize 92 25
AssertText "setting 04: revised"
Escape
End
AssertText "check 44: verified"
State target/terminal-input/multi-file-end.json
Home
AssertText "src/routes.txt"
Caption "Nine search matches across three files survive mode and resize changes"
Screenshot target/terminal-input/multi-file.png
Type "q"
