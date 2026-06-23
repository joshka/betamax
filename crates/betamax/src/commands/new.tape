# Betamax tape reference:
# https://www.joshka.net/betamax/reference/tape-reference/
#
# This file includes a compact command summary. The canonical reference documents each command's
# behavior, each setting's default value, and the current VHS differences.
#
# Output:
#   Output <path>.gif               Create a GIF output at the given <path>
#   Output <path>.png               Create a final-frame PNG output at the given <path>
#   Output <path>.mp4               Create an MP4 output at the given <path>
#   Output <path>.webm              Create a WebM output at the given <path>
#   Output <path>.json              Write final terminal state at the given <path>
#   Output <dir>                    Write a numbered PNG frame sequence
#
# Require:
#   Require <string>                Ensure a program is on the $PATH to proceed
#
# Settings:
#   Set FontSize <number>           Set the font size of the terminal
#   Set FontFamily <string>         Set the font family of the terminal
#   Set Height <number>             Set the height of the terminal
#   Set Width <number>              Set the width of the terminal
#   Set LetterSpacing <float>       Set the font letter spacing (tracking)
#   Set LineHeight <float>          Set the font line height
#   Set LoopOffset <float>%         Set the starting frame offset for the GIF loop
#   Set Theme <json|string>         Set the theme of the terminal
#   Set Padding <number>            Set the padding of the terminal
#   Set Framerate <number>          Set the framerate of the recording
#   Set PlaybackSpeed <float>       Set the playback speed of the recording
#   Set MarginFill <#000000>        Set the color the margin will be filled with.
#   Set Margin <number>             Set the size of the outer margin.
#   Set BorderRadius <number>       Set terminal border radius, in pixels.
#   Set WindowBar <string>          Set window bar type. (one of: Rings, RingsRight, Colorful, ColorfulRight)
#   Set WindowBarSize <number>      Set window bar size, in pixels. Default is 30.
#   Set TypingSpeed <time>          Set the typing speed of the terminal. Default is 50ms.
#   Set CursorBlink <bool>          Toggle cursor blinking in captured frames
#   Set KeyboardOverlay <mode>      Draw recent input labels over generated media
#                                   Modes: Off, Keys, Input, All
#   Set KeyboardOverlayLocation <location>
#                                   Locations: CaptionRow, TopLeft, TopRight, BottomLeft, BottomRight
#   Set WaitTimeout <time>          Set the default timeout for Wait commands
#   Set WaitPattern <regex>         Set the default regex for bare Wait commands
#
# Screenshot:
#   Screenshot <path>.png           Capture a PNG screenshot at this point in the tape
#
# State:
#   State <path>.json               Capture compact terminal text, styles, and scrollback
#
# Sleep:
#   Sleep <time>                    Sleep for a set amount of <time> in seconds
#
# Type:
#   Type[@<time>] "<characters>"    Type <characters> into the terminal with a
#                                   <time> delay between each character
#
# Clipboard:
#   Copy "<characters>"             Copy text into the tape clipboard
#   Paste                           Paste the tape clipboard into the terminal
#
# Wait:
#   Wait[@<time>] [/regex/|text]    Wait for the current line to match
#   Wait+Line[@<time>] [/regex/|text]
#                                   Wait for the current line to match
#   Wait+Screen[@<time>] [/regex/|text]
#                                   Wait for the screen to match
#
# Keys:
#   Escape[@<time>] [number]        Press the Escape key
#   Backspace[@<time>] [number]     Press the Backspace key
#   Delete[@<time>] [number]        Press the Delete key
#   Insert[@<time>] [number]        Press the Insert key
#   Down[@<time>] [number]          Press the Down key
#   Enter[@<time>] [number]         Press the Enter key
#   Space[@<time>] [number]         Press the Space key
#   Tab[@<time>] [number]           Press the Tab key
#   Left[@<time>] [number]          Press the Left Arrow key
#   Right[@<time>] [number]         Press the Right Arrow key
#   Up[@<time>] [number]            Press the Up Arrow key
#   Down[@<time>] [number]          Press the Down Arrow key
#   PageUp[@<time>] [number]        Press the Page Up key
#   PageDown[@<time>] [number]      Press the Page Down key
#   Home[@<time>] [number]          Press the Home key
#   End[@<time>] [number]           Press the End key
#   Shift+Tab[@<time>] [number]     Press Shift+Tab
#   F1[@<time>] [number]            Press a function key. Supports F1 through F25
#   Ctrl+<key>                      Press Control + <key> (e.g. Ctrl+C)
#   Alt+<key>                       Press Alt + <key> (e.g. Alt+F)
#   Ctrl+Alt+Shift+<key>            Press combined modifiers
#
# Display:
#   Hide                            Stop recording frames while still executing commands
#   Show                            Resume recording and capture the current terminal frame

Output {name}.gif

Require echo

Set Shell "bash"
Set FontSize 32
Set Width 1200
Set Height 600

Type "echo 'Welcome to Betamax!'" Sleep 500ms  Enter

Sleep 5s
