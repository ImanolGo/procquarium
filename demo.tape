# Record a short demo GIF with vhs (https://github.com/charmbracelet/vhs):
#
#   vhs demo.tape
#
# The fixed seed makes the recording repeatable.

Output demo.gif

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 16
Set Width 1000
Set Height 620
Set Padding 16
Set Theme "Dracula"
Set TypingSpeed 45ms

Type "procquarium --seed 7 --max-fish 40"
Enter
Sleep 7s

Type "l"
Sleep 3s

Type " "
Sleep 2s

Type "q"
Sleep 500ms
