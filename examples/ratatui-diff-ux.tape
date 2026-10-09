# Build the pinned merged viewer, then set BETAMAX_DIFF_VIEWER to its executable.
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
AssertCells 29 10 '["1","5","0","0"]'
AssertStyle 29 10 4 '{"fg":"#75cf84","bg":"#254835","bold":true}'
Type "i"
AssertStyle 29 10 4 '{"fg":"#75cf84","bg":"#162b25"}'
Type "i"
AssertStyle 29 10 4 '{"fg":"#75cf84","bg":"#254835","bold":true}'
Type "n"
AssertAbsent "   1  1"
Type "n"
AssertText "   1  1"
Type "/status"
Enter
Wait+Screen@5s "1/3"
AssertText "/status · 1/3"
Type "f"
AssertText "/status · 2/3"
Type "f"
AssertText "/status · 3/3"
Type "f"
AssertText "/status · 1/3"
Type "F"
AssertText "/status · 3/3"
Type "s"
AssertText "3/3 · Split"
Type "s"
AssertText "3/3 · Unified"
Type "/zzzz"
Enter
AssertText "/zzzz · 0/0"
Escape
AssertAbsent "/zzzz"
# Select new 1500, then verify old-side 500 independently.
MouseMove 29 10
MouseDown Left
MouseMove 32 10
MouseUp Left
Type "c"
AssertText 'Text preview: "1500"'
Escape
MouseMove 29 8
MouseDown Left
MouseMove 31 8
MouseUp Left
Type "c"
AssertText 'Text preview: "500"'
Escape
# A gutter click reports metadata and must not select source.
MouseMove 3 8
MouseDown Left
MouseUp Left
Type "c"
AssertAbsent "Text preview:"
# File headers and blank split counterparts must not select source either.
MouseMove 12 3
MouseDown Left
MouseUp Left
Type "c"
AssertAbsent "Text preview:"
Type "s"
MouseMove 20 10
MouseDown Left
MouseUp Left
Type "c"
AssertAbsent "Text preview:"
Type "s"
# Return to the new source, select with keyboard from the pointer anchor.
MouseMove 29 10
Type "v"
Right 4
Type "c"
AssertText 'Text preview: "1500"'
Type "swtm"
AssertText "Split"
AssertText "Split · wrapped"
AssertText "Monochrome"
AssertText 'Text preview: "1500"'
Caption "Search and source selection verified through real terminal input"
Screenshot target/terminal-input/ux-composed.png
State target/terminal-input/ux-composed.json
Resize 50 16
AssertText 'Text preview: "1500"'
AssertText "Arrows select"
Resize 50 5
AssertText "Split"
Resize 92 25
AssertText 'Text preview: "1500"'
# Escape clears selection before search entry; canceling query restores normal input.
Escape
Type "/cancel"
Escape
AssertAbsent "/cancel"
Type "q"
