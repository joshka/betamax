Output examples/output/hide-show.gif

Require printf

Set Shell "bash"
Set Theme "GitHub Dark"
Set FontSize 24
Set Width 968
Set Height 558
Set Margin 14
Set WindowBar RingsRight
Set BorderRadius 10
Set TypingSpeed 30ms

Hide
Type "printf 'warming cache...\n'"
Enter
Wait+Screen "warming cache"
Type "printf 'compiling hidden setup...\n'"
Enter
Wait+Screen "compiling hidden setup"
Show

Type "printf 'ready: only the useful output is visible\n'"
Enter
Wait+Screen "ready:"
Sleep 900ms
Hide
Type "exit"
Enter
