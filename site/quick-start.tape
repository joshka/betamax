# Homepage quick-start recording.
#
# This is separate from `examples/quick-start.tape` because the homepage places the rendered
# terminal inside a TV frame. Keep this tape frameless: no Betamax margin, window bar, or border
# radius. The TV image supplies the outer frame.
Output site/public/assets/betamax-hero-quick-start.gif

Require cargo
Require cargo-binstall

Set Shell "bash"
Set Theme "Betamax Brownout"
# The TV frame wants a 780x520 terminal viewport with 80px of overscan padding around it.
Set Width 940
Set Height 680
Set Padding 80
Set Framerate 12
Set TypingSpeed 8ms
Set CursorBlink false

Env PS1 '\[\e[33m\]> \[\e[0m\]'

Hide
Type "export PATH=\"$(pwd)/target/debug:$HOME/.cargo/bin:$PATH\""
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

Type "cargo binstall -y betamax"
Sleep 500ms
Enter
Wait+Screen@2m "Done"
Sleep 2s

Type "betamax --help"
Sleep 500ms
Enter
Wait+Screen "--version"
Sleep 2s

Type "betamax new new.tape"
Sleep 500ms
Enter
Wait
Sleep 2s

Type "sed -n '/^Output/,$p' new.tape"
Sleep 500ms
Enter
Wait+Screen "Output new.gif"
Sleep 2s

Type "betamax run new.tape"
Sleep 500ms
Enter
Wait+Screen@2m "wrote new.gif"
Sleep 2s

Type "ls -lh new.gif"
Sleep 500ms
Enter
Wait
Sleep 4s

Hide
Type "rm -f new.gif new.tape"
Enter
Wait
Type "cd / && rmdir \"$workdir\""
Enter
