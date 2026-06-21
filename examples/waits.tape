Output examples/output/waits.gif

Require printf

Set Shell "bash"
Set Theme "Aardvark Blue"
Set FontSize 24
Set Width 964
Set Height 554
Set Margin 12
Set WindowBar Rings
Set BorderRadius 8
Set TypingSpeed 25ms
Set WaitTimeout 3s
Set WaitPattern ">"

Type "printf 'line wait target\n'"
Enter
Wait+Line ">"

Type "printf 'screen marker: ready\n'"
Enter
Wait+Screen "screen marker"

Type "printf 'regex marker: item-42\n'"
Enter
Wait+Screen "/item-[0-9]+/"

Sleep 700ms
Hide
Type "exit"
Enter
