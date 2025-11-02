# VHS Screenshots for Kelora Documentation

This directory contains VHS tape scripts for generating animated GIF screenshots used in the Kelora documentation.

## What is VHS?

[VHS](https://github.com/charmbracelet/vhs) is a tool for generating terminal GIFs as code. It reads `.tape` files containing instructions and outputs animated GIFs.

## Prerequisites

Install VHS:

```bash
# macOS
brew install vhs

# Other platforms: see https://github.com/charmbracelet/vhs#installation
```

## Generating Screenshots

### Regenerate All Screenshots

```bash
just screenshots
```

This command:
1. Builds Kelora in release mode
2. Runs all `.tape` files in this directory
3. Outputs GIFs to `docs/screenshots/`

### Generate Individual Screenshots

```bash
vhs vhs/hero.tape
vhs vhs/mark-gaps.tape
vhs vhs/levelmap.tape
vhs vhs/error-triage.tape
vhs vhs/colored-output.tape
```

## Screenshot Inventory

| Tape File | Output | Used In | Shows |
|-----------|--------|---------|-------|
| `hero.tape` | `hero.gif` | Homepage (hero) | Multiline stacktraces with `--before-context`/`--after-context` colored highlighting |
| `mark-gaps.tape` | `mark-gaps.gif` | CLI Reference | Time gap markers with `--mark-gaps` |
| `levelmap.tape` | `levelmap.gif` | Formats Reference | Compact levelmap output format |
| `error-triage.tape` | `error-triage.gif` | CLI Reference | Error filtering with context highlighting |
| `colored-output.tape` | `colored-output.gif` | Basics Tutorial | Default formatter with colored key-value pairs |

## Creating New Screenshots

1. Create a new `.tape` file in this directory
2. Use this template:

```tape
# Description of what this screenshot shows
Output docs/screenshots/your-screenshot.gif

# Settings
Set FontSize 14
Set Width 1200
Set Height 600
Set Padding 20
Set Theme "Dracula"

# Type your command
Type "./target/release/kelora [your command here]"
Sleep 500ms
Enter

# Wait for output
Sleep 3s

# Pause for reading
Sleep 2s
```

3. Test it: `vhs vhs/your-screenshot.tape`
4. Integrate into docs with: `![Alt text](../screenshots/your-screenshot.gif)`

## Tips

- **Escaping quotes**: Break complex commands into multiple `Type` commands to avoid escaping issues
- **Timing**: Adjust `Sleep` durations to ensure commands complete and viewers can read output
- **Terminal size**: Keep width around 1200px for readability in docs
- **Theme**: We use "Dracula" for intense, vibrant colors
- **File size**: GIFs can be large. Keep recordings under 10 seconds when possible

## Troubleshooting

**VHS parse errors with quotes:**
```tape
# Instead of:
Type "kelora --filter 'e.contains(\"error\")'"

# Use:
Type "kelora --filter 'e.contains("
Type '"'
Type "error"
Type '"'
Type ")'"
```

**Command not found:**
Ensure you've run `cargo build --release` first. The tape files use `./target/release/kelora`.

**Output too fast:**
Increase the `Sleep` duration after `Enter`.
