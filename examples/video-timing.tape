Output examples/output/video-timing.gif
Output examples/output/video-timing.mp4
Output examples/output/video-timing.webm
Require ffmpeg
Require bash

Set Shell "bash"
Set Width 640
Set Height 300
Set FontSize 18
Set Framerate 30
Set CursorBlink false
Env BETAMAX_VIDEO_DEMO "printf '\\033[2J\\033[H\\033[?25lPRESS ENTER TO START\\n'; read -r; holds=(0.30 0.20 0.30 0.10 0.40 0.20 0.50); bar='################################'; for step in 1 2 3 4 5 6 7 8; do width=$((step * 4)); printf '\\033[2J\\033[H\\033[1;36mNATIVE VIDEO TIMING\\033[0m\\n\\n'; printf 'Frame %d / 8  -  %3d%%\\n\\n' \"$step\" \"$((step * 100 / 8))\"; printf '[\\033[32m%-32s\\033[0m]\\n' \"${bar:0:width}\"; printf '\\033[7;%dH\\033[33m[@]\\033[0m' \"$((step * 5))\"; if [ \"$step\" -lt 8 ]; then printf '\\033[9;1HRendering...'; sleep \"${holds[step - 1]}\"; else printf '\\033[9;1H\\033[1;32mCOMPLETE\\033[0m - hold this frame for 1.5 seconds\\n'; fi; done"
# Keep setup hidden; animate terminal cells rather than a presentation caption.
Hide
Type@0ms "eval \"$BETAMAX_VIDEO_DEMO\""
Enter
Wait+Screen "PRESS ENTER TO START"
Show
Enter
Wait+Screen "COMPLETE"
Sleep 1500ms
Hide
