# VHS tape for homepage hero
# Shows three quick pipelines from docs/index.md (filters, custom parsing, visualization)

Output docs/screenshots/hero.gif

# Settings
Set FontSize 24
Set Width 1400
Set Height 800
Set Padding 20
Set Theme "Catppuccin Mocha"

# First example: slow/failing requests in logfmt
Hide
Type "kelora -f logfmt examples/traffic_logfmt.log --since "
Type '"'
Type "2024-07-17T12:00:00Z"
Type '"'
Type " --filter 'e.status.to_int() >= 500 || e.latency_ms.to_int() > 1200'"
Enter
Enter
Enter
Show

# Second example: parse custom rollout logs to JSON
Hide
Type "kelora -f "
Type "'"
Type "cols:ts level service request_id *message"
Type "'"
Type " examples/release_pipe.log --cols-sep '|' --levels warn,error -F json"
Enter
Enter
Enter
Show

# Third example: levelmap visualization
Hide
Type "kelora -j examples/levelmap-demo.jsonl -F levelmap"
Enter
Show

# Wait for output to complete then pause
Sleep 5s
