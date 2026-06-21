Output examples/output/themes.gif

Require printf

Set Shell "bash"
Set Theme "Dracula"
Set FontSize 24
Set Width 992
Set Height 588
Set Margin 16
Set MarginFill "#111111"
Set WindowBar Colorful
Set WindowBarSize 36
Set BorderRadius 10

Type "printf '\033[31mred\033[0m \033[32mgreen\033[0m \033[34mblue\033[0m\n'"
Enter
Wait+Screen "blue"
Type "printf '\033[35mghostty theme: Dracula\033[0m\n'"
Enter
Wait+Screen "Dracula"
Sleep 900ms
Hide
Type "exit"
Enter
