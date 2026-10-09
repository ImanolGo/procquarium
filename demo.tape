# Record a short demo GIF with vhs (https://github.com/charmbracelet/vhs):
#
#   vhs demo.tape
#
# The fixed seed makes the recording repeatable.

Output demo.gif

Set Shell "bash"
Set FontFamily "DejaVu Sans Mono"
Set FontSize 16
Set Width 1000
Set Height 620
Set Padding 16
Set Theme "Dracula"
Set TypingSpeed 45ms

# A calm tank with kernel threads (crabs) and feeding enabled.
Type "procquarium --seed 7 --max-fish 30 --kernel --feed"
Enter
Sleep 7s

# Drop some food; nearby fish swim over and eat it.
Type "fff"
Sleep 5s

# Show the process names, then select a fish for its details box.
Type "l"
Sleep 1s
Tab
Sleep 3s

# Pause for a moment, then quit.
Space
Sleep 2s
Type "q"
Sleep 500ms
