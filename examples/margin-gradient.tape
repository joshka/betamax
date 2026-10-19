Output examples/output/margin-gradient.png
Output examples/output/margin-gradient.gif

Set Shell "bash"
Set Theme '{"name":"Night","background":"#151723","foreground":"#d3d7eb"}'
Set Width 960
Set Height 720
Set FontSize 22
Set Padding 20
Set Margin 44
Set MarginFill "linear(to: bottom-right, #4d5b9e 0%, #232640 42%, #0f101a 100%)"
Set WindowBar Colorful
Set BorderRadius 11
Set Framerate 10
Set CursorBlink false

Hide
Type "clear; printf 'Gradient margin fill\n\nA backdrop shared by screenshots and animations.\n'"
Enter
Wait+Screen "A backdrop shared"
Caption "A positioned gradient behind the window and caption"
Show
Sleep 500ms
