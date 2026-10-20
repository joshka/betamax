# A picture as the backdrop. `image(path)` covers the output, cropping a 3:2 image to fit 16:9.
# The path is not found from the repository root, so it resolves beside this tape.
# A bare path such as `Set MarginFill "fixtures/margin-backdrop.jpg"` stretches like VHS.
Output examples/output/margin-image.gif
Output examples/output/margin-image.mp4

Set Preset Framed
Set Shell "bash"
Set MarginFill "image(fixtures/margin-backdrop.jpg)"
Set Width 960
Set Height 540
Set CursorBlink false
Set WindowTitle "Betamax — image margin"

Hide
Type "clear"
Enter
Show
Caption "MarginFill \"image(fixtures/margin-backdrop.jpg)\""
Type "printf 'The picture %s the margin.\n' fills"
Enter
Wait+Screen "picture fills"
Sleep 1s
Screenshot examples/output/margin-image.png
