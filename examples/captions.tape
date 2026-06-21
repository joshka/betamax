Output examples/output/captions.gif
Output examples/output/captions.png

Require printf
Require sleep

Set Shell "bash"
Set Theme "Catppuccin Mocha"
Set FontSize 23
Set Framerate 12
Set Width 960
Set Height 540
Set Margin 12
Set WindowBar Colorful
Set BorderRadius 8

Caption "Step 1: hidden setup produced this review frame"
Hide
Type "printf 'loading...\n'; sleep 2; printf 'checkpoint ready\n'"
Enter
Wait+Screen "checkpoint ready"
Show
Sleep 700ms

Caption "Step 2: checkpoint output is visible"
Screenshot examples/output/captions-checkpoint.png
Sleep 900ms
