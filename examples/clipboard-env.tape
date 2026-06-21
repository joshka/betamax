Output examples/output/clipboard-env.gif

Require printf

Env BETAMAX_EXAMPLE clipboard-env

Set Shell "bash"
Set Theme "TokyoNight Night"
Set FontSize 24
Set Width 1004
Set Height 554
Set Margin 12
Set WindowBar RingsRight
Set BorderRadius 8
Set TypingSpeed 25ms

Copy "pasted-from-betamax"
Type "printf 'env=%s clipboard=' \"$BETAMAX_EXAMPLE\" && printf '%s\n' "
Paste
Enter
Wait+Screen "env=clipboard-env clipboard=pasted-from-betamax"

Sleep 700ms
Hide
Type "exit"
Enter
