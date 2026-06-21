Output examples/output/screenshot.gif
Output examples/output/state.json

Require printf

Set Shell "bash"
Set Theme "TokyoNight Night"
Set FontSize 24
Set Width 924
Set Height 514
Set Margin 12
Set WindowBar ColorfulRight
Set BorderRadius 8

Screenshot examples/output/screenshot.png

Type "printf 'this frame also has a screenshot\n'"
Enter
Wait+Screen "screenshot"
Screenshot examples/output/screenshot-after.png
State examples/output/state-after.json
Sleep 800ms
Hide
Type "exit"
Enter
