# Record a short demo GIF with vhs (https://github.com/charmbracelet/vhs):
#
#   vhs demo.tape
#
# The fixed seed makes the tank repeatable.

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

# Show the process names.
Type "l"
Sleep 2s

# Search for a process by name.
Type "/co"
Sleep 2s
Enter
Sleep 2s

# Select a fish: the details box, CPU sparkline and family line.
Tab
Sleep 4s

# Drop some food; nearby fish swim over and eat it.
Type "fff"
Sleep 4s

# Pause for a moment, then quit.
Space
Sleep 2s
Type "q"
Sleep 500ms
