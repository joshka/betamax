Output examples/output/window-title-from-app.gif
Output examples/output/window-title-from-app.png
Output examples/output/window-title-from-app.json

Set Shell "bash"
Set Theme "GitHub Dark"
Set Width 720
Set Height 280
Set FontFamily "Menlo"
Set FontSize 18
Set Padding 16
Set Margin 24
Set MarginFill "#232640"
Set WindowBar Colorful
Set WindowBarSize 40
Set BorderRadius 10
Set WindowTitle "title set by tape"
Set WindowTitleFromTerminal true
Set CursorBlink false
Set TypingSpeed 0ms

Hide
Type "stty -echo; PS1=''; printf '\033[?25l\033[2J\033[HThe app changes only the window title.\n'"
Enter
Wait+Screen "The app changes only"
Caption "1. Initial title from the tape"
Show
Sleep 1s
Screenshot examples/output/title-from-app-tape.png

Caption "2. The app emits OSC 0"
Type "printf '\033]0;title set by app\007'"
Enter
Sleep 1s
Screenshot examples/output/title-from-app-set.png

Caption "3. The app emits OSC 2"
Type "printf '\033]2;title updated by app\033\\'"
Enter
Sleep 1s
Screenshot examples/output/title-from-app-updated.png

Caption "4. The tape takes over again"
WindowTitle "title reset by tape"
Sleep 1s
Screenshot examples/output/title-from-app-reset.png

Caption "5. A later app update takes over"
Type "printf '\033]2;title set again by app\007'"
Enter
Sleep 1s
Screenshot examples/output/title-from-app-again.png
