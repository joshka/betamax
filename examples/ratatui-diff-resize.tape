# Set BETAMAX_DIFF_VIEWER to a built viewer from merged ratatui-diff main.
Set Shell "bash"
Set Width 1340
Set Height 680
Set FontFamily "JetBrains Mono"
Set FontSize 24
Set LetterSpacing 0
Set Padding 20
Set Theme "Aardvark Ink"
Set TypingSpeed 0ms
Set CursorBlink false
Hide
Type "exec \"$BETAMAX_DIFF_VIEWER\" --aardvark-ink"
Enter
Wait+Screen@5s "timeout_ms: 1500"
Caption "Before: source diff, no selection"
Screenshot target/terminal-input/diff-before.png
State target/terminal-input/diff-before.json
# Cell coordinates are pinned to this fixture and geometry.
MouseMove 29 10
MouseDown Left
MouseMove 32 10
MouseUp Left
Type "c"
Wait+Screen@5s 'Text preview: "1500"'
Caption "After: mouse drag selects new 1500; c previews exact source"
Screenshot target/terminal-input/diff-after.png
State target/terminal-input/diff-after.json
Resize 50 16
Wait+Screen@5s "Arrows select · c preview · Esc clear"
Caption "Resized: 50x16 grid retains the selected source and preview"
Screenshot target/terminal-input/diff-resize-50x16.png
State target/terminal-input/diff-resize-50x16.json
Resize 92 25
Wait+Screen@5s "Home/End"
Caption "Restored: 92x25 grid retains exact 1500 source selection"
Screenshot target/terminal-input/diff-resize-restored.png
State target/terminal-input/diff-resize-restored.json
Type "q"
