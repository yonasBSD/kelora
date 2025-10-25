# VHS tape for mark-gaps demo
# Shows time gap separators between events

Output docs/screenshots/mark-gaps.gif

# Settings
Set FontSize 24
Set Width 1400
Set Height 800
Set Padding 20
Set Theme "Catppuccin Mocha"

# Execute command without showing typing
Hide
Type "kelora -j examples/simple_json.jsonl --mark-gaps 30s -n 15"
Enter
Show

# Wait for output to complete then pause
Sleep 5s
