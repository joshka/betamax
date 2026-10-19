# The same scene with the Default-0.1 baseline. Framed is also the implicit default.
Output examples/output/presets-default-0.1.png
Set Preset Default-0.1
Set Shell "bash"
Set CursorBlink false
Set WindowTitle "Betamax — text and Unicode"
Hide
Type "python3 scripts/feature-scene.py styles"
Enter
Wait+Screen "Text with character"
Show
Sleep 100ms
