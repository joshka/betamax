# The same scene with the VHS baseline. Framed is also the implicit default.
Output examples/output/styles-vhs.png
Set Style VHS
Set Shell "bash"
Set CursorBlink false
Set WindowTitle "Betamax — text and Unicode"
Hide
Type "python3 scripts/feature-scene.py styles"
Enter
Wait+Screen "Text with character"
Show
Sleep 100ms
