Output examples/output/video.gif
Output examples/output/video.mp4
Output examples/output/video.webm

Require printf

Set Shell "bash"
Set Theme "Catppuccin Mocha"
Set FontSize 24
Set Width 900
Set Height 480
Set Margin 12
Set WindowBar Colorful
Set BorderRadius 8
Set PlaybackSpeed 1.25
Set LoopOffset 10%

Type "printf 'one capture can write gif, mp4, and webm\n'"
Enter
Wait+Screen "webm"
Type "printf 'ffmpeg is only used for video containers\n'"
Enter
Wait+Screen "containers"
Sleep 900ms
Hide
Type "exit"
Enter
