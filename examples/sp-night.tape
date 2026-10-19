Output examples/output/sp-night.png

Require printf
Set Shell "bash"
Set Theme "sp_night_noite"
Set Width 900
Set Height 360
Set FontSize 22
Set FontFamily "Menlo"
Set LetterSpacing 0
Set Padding 24
Set Margin 44
Set MarginFill "linear(#232640, #0f101a)"
Set WindowBar Colorful
Set WindowBarColor "#0f101a"
Set WindowTitle "SP Night — Noite Paulista"
Set Shadow true
Set WindowBorder 1
Set BorderRadius 11
Set CursorBlink false
Set TypingSpeed 0ms

Hide
Type "printf '\033[2J\033[HSP Night — Noite Paulista\n\n\033[31mred\033[0m  \033[32mgreen\033[0m  \033[33myellow\033[0m  \033[34mblue\033[0m  \033[35mmagenta\033[0m  \033[36mcyan\033[0m\n\nOfficial Ghostty palette · MIT\nCopyright 2026 sp night\n'"
Enter
Wait+Screen "Official Ghostty palette"
Show
Sleep 100ms
