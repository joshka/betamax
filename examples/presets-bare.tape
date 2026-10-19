# The same scene with the Bare baseline. Framed is also the implicit default.
Output examples/output/presets-bare.png
Set Preset Bare
Set Shell "bash"
Set CursorBlink false
Set WindowTitle "Betamax — text and Unicode"
Hide
Type "python3 scripts/feature-scene.py styles"
Enter
Wait+Screen "Text with character"
Show
Sleep 100ms
