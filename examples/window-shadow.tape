Output examples/output/window-shadow.png

Require printf

Set Shell "bash"
Set Width 800
Set Height 460
Set FontSize 22
Set Padding 20
Set Theme '{"name":"Shadow Night","background":"#151723","foreground":"#d3d7eb"}'
Set Margin 44
Set MarginFill "linear(#232640, #0f101a)"
Set WindowBar Colorful
Set BorderRadius 11
Set Shadow true
Set ShadowBlur 28
Set ShadowOffset 9
Set ShadowOpacity 0.55
Set CursorBlink false

Hide
Type "printf 'Window shadow\n\nA soft frame for terminal recordings.\n'"
Enter
Wait+Screen "A soft frame"
Show
Sleep 100ms
