# VHS tape for colored key-value output demo
# Shows default formatter with highlighted keys

Output docs/screenshots/colored-output.gif

# Settings
Set FontSize 24
Set Width 1400
Set Height 700
Set Padding 20
Set Theme "Catppuccin Mocha"

# Execute command without showing typing
Hide
Type "./target/release/kelora -f combined examples/simple_combined.log -k timestamp,ip,status,method,path -n 6"
Enter
Show

# Wait for output to complete then pause
Sleep 5s
