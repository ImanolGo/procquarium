complete -c procquarium -l interval -d 'Seconds between process samples' -r
complete -c procquarium -l max-fish -d 'Maximum number of fish in the tank' -r
complete -c procquarium -l user -d 'Only show processes owned by this user' -r
complete -c procquarium -l filter -d 'Only show processes whose name matches this regex' -r
complete -c procquarium -l seed -d 'Fixed random seed, for a reproducible tank' -r
complete -c procquarium -l config -d 'Path to a TOML config file (theme and defaults)' -r -F
complete -c procquarium -l record -d 'Write every snapshot as a JSON line to this file' -r -F
complete -c procquarium -l replay -d 'Replay snapshots from a recording instead of sampling' -r -F
complete -c procquarium -l colors -d 'Colour handling: auto (default), truecolor, 256 or none' -r -f -a "auto\t'Truecolour when the terminal advertises it, else 256-colour'
truecolor\t'Always 24-bit RGB'
256\t'Always the nearest of the 256 xterm colours'
none\t'No colour, the same as setting `NO_COLOR`'"
complete -c procquarium -l kernel -d 'Include kernel threads'
complete -c procquarium -l ascii -d 'Use plain ASCII glyphs instead of the Unicode ones'
complete -c procquarium -l screensaver -d 'Exit on any key or mouse event; hide labels and the info box'
complete -c procquarium -l feed -d 'Let `f` drop food; fish that eat get a small priority nudge (needs privileges to raise priority, and only ever touches your own processes)'
complete -c procquarium -l no-mouse -d 'Don\'t capture the mouse, so text selection keeps working'
complete -c procquarium -l kill -d 'Allow `k` to send SIGTERM to the selected process (asks first)'
complete -c procquarium -l dump -d 'Print one snapshot as a table and exit'
complete -c procquarium -s h -l help -d 'Print help (see more with \'--help\')'
complete -c procquarium -s V -l version -d 'Print version'
