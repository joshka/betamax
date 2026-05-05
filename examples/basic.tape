Output examples/output/basic.gif

Require printf

Set Shell "bash"
Set Theme "Aardvark Blue"
Set FontSize 28
Set Width 900
Set Height 480
Set Margin 12
Set WindowBar Colorful
Set WindowBarSize 34
Set BorderRadius 8
Set CursorBlink false

Type "printf 'hello from betamax\n'"
Enter
Wait+Screen "hello from betamax"
Sleep 700ms
Hide
Type "exit"
Enter
