Output examples/output/window-border.png
Require printf
Set Shell "bash"
Set Width 800
Set Height 460
Set FontSize 22
Set Padding 20
Set Theme '{"name":"Border Night","background":"#151723","foreground":"#d3d7eb"}'
Set Margin 44
Set MarginFill "linear(#232640, #0f101a)"
Set WindowBar Colorful
Set BorderRadius 11
Set WindowBorder 1
Set CursorBlink false
Hide
Type "printf 'Window border\n\nA subtle outline for terminal recordings.\n'"
Enter
Wait+Screen "A subtle outline"
Show
Sleep 100ms
