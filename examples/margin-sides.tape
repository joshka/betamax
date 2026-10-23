Output examples/output/margin-sides.png

Set Shell "bash"
Set Theme '{"name":"Night","background":"#151723","foreground":"#d3d7eb"}'
Set Width 960
Set Height 600
Set FontSize 22
Set Padding 20
Set Margin "40 200 120 40"
Set MarginFill "linear(angle: 135deg, #4d5b9e, #0f101a)"
Set WindowBar Colorful
Set BorderRadius 11
Set CursorBlink false

Hide
Type "clear; printf 'Per-side margins\n\nTop, right, bottom, left, as in CSS.\n'"
Enter
Wait+Screen "as in CSS"
Caption "Room below and to the right of the window"
Show
Sleep 500ms
