Output examples/output/window-bar-color.png

Require printf

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

Type "printf 'A darker window bar\n'"
Enter
Wait+Screen "A darker window bar"
Sleep 200ms
