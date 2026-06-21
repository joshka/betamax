Output examples/output/outputs.gif
Output examples/output/outputs.png
Output examples/output/outputs.json
Output examples/output/outputs-frames

Require printf

Set Shell "bash"
Set Theme "Catppuccin Mocha"
Set FontSize 24
Set Width 964
Set Height 554
Set Margin 12
Set WindowBar ColorfulRight
Set BorderRadius 8

Screenshot examples/output/outputs-checkpoint.png

Type "printf 'gif png json screenshot state frames\n'"
Enter
Wait+Screen "frames"

State examples/output/outputs-checkpoint.json

Sleep 700ms
Hide
Type "exit"
Enter
