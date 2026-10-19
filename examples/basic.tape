Output examples/output/basic.gif

Require printf

# Preserve the curated website/README presentation on regeneration.
Set Preset Default-0.1
Set Shell "bash"
Set Theme "Aardvark Blue"
Set FontSize 28
Set Width 924
Set Height 538
Set Margin 12
Set WindowBar Colorful
Set WindowBarSize 34
Set BorderRadius 8
Set CursorBlink false

Type "printf 'hello from betamax\n'"
Enter
Wait+Screen "hello from betamax"
Sleep 2s
Hide
Type "exit"
Enter
