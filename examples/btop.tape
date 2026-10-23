# Record btop, a dense full-screen system monitor: truecolor gradients, braille graphs, box
# drawing, and a redraw on every update.
Output examples/output/btop.gif

Require btop

Set Columns 100
Set Rows 30
Set CursorBlink false

# Start btop before recording, with a throwaway config directory so it uses its defaults and leaves
# no config. Updating every 300 ms instead of the default 2 s moves the graphs visibly; a few hidden
# seconds give them data, so the recording opens on a full screen.
Hide
Type "export XDG_CONFIG_HOME=$(mktemp -d)"
Enter
Wait
Type "btop --update 300"
Enter
Wait+Screen "Cpu%"
Sleep 3s
Show
Sleep 4s

# Walk down the process list.
Down@300ms 5
Sleep 2s

# Hide the network box, then bring it back.
Type "3"
Sleep 2s
Type "3"
Sleep 2s

# Open the main menu, which shows the version, and move to Quit.
Type "m"
Wait+Screen "/v[0-9]+\.[0-9]+\.[0-9]+/"
Sleep 2s
Down@600ms 2
Sleep 1500ms

# Quit after recording stops, so the loop goes from the menu back to btop.
Hide
Enter
Wait
Type "rm -rf \"$XDG_CONFIG_HOME\""
Enter
