Output examples/output/webp-lossy.webp
Require bash

Set Shell "bash"
Set Width 640
Set Height 300
Set FontSize 18
Set Padding 20
Set Margin 12
Set Framerate 30
Set TypingSpeed 40ms
# Truecolor gradients compress poorly as lossless frames; quality 85 keeps text free of ghosts.
Set WebpQuality 85

Hide
Type@0ms "PS1='$ '; clear"
Enter
Show

Type "for r in $(seq 0 9 255); do printf '\e[48;2;%d;90;%dm ' $r $((255-r)); done; printf '\e[0m\n'"
Enter
Sleep 600ms
Type "echo lossy WebP keeps typing crisp"
Enter
Sleep 1.5s
