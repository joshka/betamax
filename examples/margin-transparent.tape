# A clear backdrop: the window and its shadow sit on whatever page shows the image.
Output examples/output/margin-transparent.png
Output examples/output/margin-transparent.webp
Output examples/output/margin-transparent.gif

Set Preset Framed
Set Shell "bash"
Set MarginFill transparent
Set Width 960
Set Height 540
Set CursorBlink false
Set WindowTitle "Betamax — transparent margin"

Hide
Type "printf 'Transparent margin\n\nThe page shows through.\n'"
Enter
Wait+Screen "shows through"
Show
Sleep 300ms
