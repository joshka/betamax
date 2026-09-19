Output examples/output/webp.webp
Output examples/output/webp.png
Require bash

Set Shell "bash"
Set Width 640
Set Height 300
Set FontSize 18
Set Padding 20
Set Margin 12
Set BorderRadius 12
Set Framerate 30
Set CursorBlink false

# Hide setup and use explicit state transitions with uneven presentation holds.
Hide
Type@0ms "PS1=''; printf '\\033[2J\\033[H\\033[?25l\\033[1;36mLOSSLESS WEBP\\033[0m\\n\\nStep 1 / 3: original colors'"
Enter
Wait+Screen "original colors"
Show
Sleep 300ms
Screenshot examples/output/webp-start.webp

Hide
Type@0ms "printf '\\033[2J\\033[H\\033[1;35mLOSSLESS WEBP\\033[0m\\n\\nStep 2 / 3: moving forward\\n\\n    [========>       ]'"
Enter
Wait+Screen "moving forward"
Show
Sleep 700ms

Hide
Type@0ms "printf '\\033[2J\\033[H\\033[1;32mLOSSLESS WEBP\\033[0m\\n\\nStep 3 / 3: complete\\n\\n    [================]\\n\\nFinal frame holds for 1.5 seconds'"
Enter
Wait+Screen "Final frame holds"
Show
Sleep 1500ms
Screenshot examples/output/webp-final.webp
# Exclude shell cleanup from the animation and keep the final hold intact.
Hide
