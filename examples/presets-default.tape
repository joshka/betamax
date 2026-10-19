# The same scene with the Default baseline, currently Framed.
Output examples/output/presets-default.png
Set Preset Default
Set Shell "bash"
Set CursorBlink false
Set WindowTitle "Betamax — text and Unicode"
Hide
Type "python3 scripts/feature-scene.py styles"
Enter
Wait+Screen "Text with character"
Show
Sleep 100ms
