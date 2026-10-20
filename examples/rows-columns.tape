Output examples/output/rows-columns.gif

Require tput

Set Shell "bash"
Set Columns 60
Set Rows 12
Set Padding 16
Set TypingSpeed 40ms

Type "echo $(tput cols) columns by $(tput lines) rows"
Enter
Wait+Screen "rows"
Type "printf '%.0s=' $(seq $(tput cols))"
Enter
Wait
Sleep 1.5s

Hide
Type "exit"
Enter
