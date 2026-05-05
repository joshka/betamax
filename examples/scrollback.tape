Output examples/output/scrollback.gif
Output examples/output/scrollback.json

Require printf

Set Shell "bash"
Set Theme "Aardvark Blue"
Set FontSize 22
Set Width 900
Set Height 360
Set Padding 36
Set Margin 12
Set WindowBar Rings
Set BorderRadius 8

Type "for i in 01 02 03 04 05 06 07 08 09 10 11 12; do printf 'scrollback line %s\n' \"$i\"; done"
Enter
Wait+Screen "scrollback line 12"
State examples/output/scrollback-after.json

Sleep 700ms
Hide
Type "exit"
Enter
