# VHS tape for context highlighting demo
# Shows syslog parsing with multiline and --after-context

Output docs/screenshots/context-highlighting.gif

# Settings
Set FontSize 24
Set Width 1400
Set Height 800
Set Padding 20
Set Theme "Catppuccin Mocha"

# Execute command without showing typing
Hide
Type "./target/release/kelora -f syslog examples/syslog_multiline.log --multiline timestamp --filter 'e.msg.contains("
Type '"'
Type "ERROR"
Type '"'
Type ")' -C 1 -k ts,host,program,msg"
Enter
Show

# Wait for output to complete then pause
Sleep 5s
