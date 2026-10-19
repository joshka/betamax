Output examples/output/window-bar-color.png

Require printf
Require clear

Set Shell "bash"
Set Width 720
Set Height 360
Set FontSize 22
Set Padding 20
Set Margin 28
Set MarginFill "#232640"
Set WindowBar RingsRight
Set WindowBarColor "#0f101a"
Set Theme '{"name":"Night","background":"#151723","foreground":"#d3d7eb"}'
Set BorderRadius 11
Set CursorBlink false

Hide
Type "clear; printf 'RingsRight with a darker window bar\n'"
Enter
Wait+Screen "darker window bar"
Show
Sleep 200ms
