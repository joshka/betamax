Output examples/output/viewport-scroll.gif
Output examples/output/viewport-scroll.json

Require seq

Set Shell "bash"
Set Theme "Aardvark Blue"
Set FontSize 22
Set Width 924
Set Height 414
Set Padding 36
Set Margin 12
Set WindowBar Rings
Set BorderRadius 8

Type "seq -f 'history line %02g' 1 30"
Enter
Wait+Screen "history line 30"
Sleep 500ms

# Move the viewport back through scrollback. The program sees no input.
ScrollUp@60ms 20
Sleep 800ms
ScrollDown@60ms 8
Sleep 800ms

# Typing returns the viewport to the prompt, as in VHS and Ghostty.
Type "echo back at the bottom"
Enter
Wait+Screen "back at the bottom"
Sleep 1s
