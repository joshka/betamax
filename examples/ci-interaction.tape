# A compact consumer workflow. Anchored waits cannot match the echoed command.
Require printf
Env BETAMAX_CI injected
Set Shell "bash"
Set Width 480
Set Height 240
Set FontSize 20
Set Framerate 20
Set TypingSpeed 0ms
Set CursorBlink false
Set WaitTimeout 5s

Hide
# Paste and edit an argument before execution; the resulting state proves both worked.
Type "printf '\033[2J\033[H\033[?25l\033[1;38;2;255;80;40mENV=%s INPUT=%s\033[0m\nREADY\n' \"$BETAMAX_CI\" "
Copy "paste-X"
Paste
Backspace
Type "ok"
Enter
Wait+Screen "/^ENV=injected INPUT=paste-ok/"
Wait+Line />$/
Show
State target/ci-tapes/interaction.json
Screenshot target/ci-tapes/interaction.png
Sleep 500ms
Hide
Type "exit"
Enter
