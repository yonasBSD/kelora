# VHS tape for error triage demo
# Shows filtering for errors with context - only some lines match

Output docs/screenshots/error-triage.gif

# Settings
Set FontSize 24
Set Width 1400
Set Height 700
Set Padding 20
Set Theme "Catppuccin Mocha"

# Execute command without showing typing - filter for only ERROR level
Hide
Type "./target/release/kelora -j examples/simple_json.jsonl --filter 'e.level == "
Type '"'
Type "ERROR"
Type '"'
Type "' -B 1 -A 1 -k timestamp,level,service,message"
Enter
Show

# Wait for output to complete then pause
Sleep 5s
