# The same scene with the Inset baseline. Framed is also the implicit default.
Output examples/output/styles-inset.png
Set Style Inset
Set Shell "bash"
Set CursorBlink false
Set WindowTitle "Betamax — text and Unicode"
Hide
Type "python3 scripts/feature-scene.py styles"
Enter
Wait+Screen "Text with character"
Show
Sleep 100ms
