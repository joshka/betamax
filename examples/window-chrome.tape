Output examples/output/window-chrome.png

Require clear
Set Shell "bash"
Set Width 1000
Set Height 550
Set FontFamily "Menlo"
Set FontSize 18
Set Padding 20
Set Theme '{"name":"Chrome Night","background":"#151723","foreground":"#d3d7eb"}'
Set Margin 44
Set MarginFill "linear(to: bottom, #232640, #0f101a)"
Set Shadow true
Set WindowBar Colorful
Set WindowBarColor "#302b40"
Set WindowTitle "Continuous window chrome"
Set BorderRadius 11
Set WindowBorder 1
Set WindowBorderOpacity 0.28
Set CursorBlink false

Hide
Type "clear; printf 'Crisp circles and continuous corners.\n\nTerminal pixels stay at their original resolution.\n'"
Enter
Wait+Screen "Terminal pixels stay"
Show
Sleep 100ms
