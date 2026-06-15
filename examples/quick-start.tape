# Render the quick-start demo and a still frame for theme/layout inspection.
Output examples/output/quick-start.gif

Require cargo
Require cargo-binstall

# Use the local Brownout Ghostty theme and a warm outer margin.
Set Shell "bash"
Set Theme "Betamax Brownout"
Set Width 1000
Set Height 820
Set Margin 16
Set MarginFill "#20140d"
Set WindowBar Colorful
Set WindowBarSize 34
Set BorderRadius 8
Set Framerate 12
Set TypingSpeed 8ms
Set CursorBlink false

# Match the prompt to the Brownout ANSI palette instead of Betamax's default purple prompt.
Env PS1 '\[\e[33m\]> \[\e[0m\]'

# Prepare a temporary work directory without showing setup commands in the recording.
Hide
Type "export PATH=\"$HOME/.cargo/bin:$PATH\""
Enter
Wait
Type "workdir=$(mktemp -d)"
Enter
Wait
Type "cd \"$workdir\""
Enter
Wait
Type "clear"
Enter
Wait
Show
Screenshot examples/output/quick-start-start.png

# Install or confirm the released CLI binary.
Type "cargo binstall -y betamax"
Sleep 500ms
Enter
Wait+Screen@2m "Done"
Sleep 2s

# Show the installed command surface.
Type "betamax --help"
Sleep 500ms
Enter
Wait+Screen "--version"
Sleep 2s

# Create a starter tape.
Type "betamax new new.tape"
Sleep 500ms
Enter
Wait
Sleep 2s

# Show the executable body of the generated tape without the reference comments.
Type "sed -n '/^Output/,$p' new.tape"
Sleep 500ms
Enter
Wait+Screen "Output new.gif"
Sleep 2s

# Run the generated tape and show Betamax's own progress output.
Type "betamax run new.tape"
Sleep 500ms
Enter
Wait+Screen@2m "wrote new.gif"
Sleep 2s

# Verify the generated GIF.
Type "ls -lh new.gif"
Sleep 500ms
Enter
Wait+Screen "/-rw.*new\\.gif/"
Sleep 4s

# Remove temporary files after recording stops.
Hide
Type "rm -f new.gif new.tape"
Enter
Wait
Type "cd / && rmdir \"$workdir\""
Enter
