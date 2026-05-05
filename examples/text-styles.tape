Output examples/output/text-styles.gif
Output examples/output/text-styles.json

Require printf

Set Shell "bash"
Set Theme "Dracula"
Set FontSize 24
Set Width 980
Set Height 520
Set Margin 14
Set WindowBar Colorful
Set BorderRadius 8

Type "printf '\033[1mbold\033[0m \033[4munderline\033[0m \033[7minverse\033[0m\n'"
Enter
Wait+Screen "inverse"

Type "printf '\033[38;2;255;120;80mtruecolor\033[0m \033[32mansi-green\033[0m\n'"
Enter
Wait+Screen "ansi-green"

State examples/output/text-styles-after.json

Sleep 800ms
Hide
Type "exit"
Enter
