# Inspect continuous borders and blocks with extra space between text cells.
Output examples/output/cell-graphics.gif

Require printf

Set Shell "bash"
Set Theme "Catppuccin Mocha"
Set TypingSpeed 0ms
Env PS1 ""
Set Width 780
Set Height 350
Set FontSize 20
Set LetterSpacing 1
Set LineHeight 1.2
Set Padding 40
Set WindowBar Colorful
Set BorderRadius 8
Set CursorBlink false

Hide
Type "printf '\033[2J\033[H\033[?25l\033[36m┌────────────────────────────────────────────┐\n│ Betamax · Terminal previews                │\n└────────────────────────────────────────────┘\n\033[0m┌─ Rendering demo ───────────────────────────┐\n│\033[32m████████████████████████████████████████████\033[0m│\n└────────────────────────────────────────────┘\n ▏▎▍▌▋▊▉█  ▁▂▃▄▅▆▇█\n'"
Enter
Sleep 500ms
Show
Screenshot examples/output/cell-graphics.png
Sleep 1s
