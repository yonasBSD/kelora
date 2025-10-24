# Getting Started: Input, Display & Filtering

Master the essential commands for reading logs, controlling display output, and filtering by log level. This tutorial covers the foundation you'll use in every Kelora workflow.

## What You'll Learn

- Specify input formats with `-f` and `-j`
- Control what fields are displayed with `-b`, `-c`, `-k`, and `-K`
- Filter events by log level with `-l` and `-L`
- Export data in different formats with `-F` and `-J`
- Combine options for common workflows

## Prerequisites

- Kelora installed and in your PATH
- Basic command-line familiarity

## Sample Data

Commands below use example files from the repository:

- `examples/simple_json.jsonl` — JSON-formatted application logs with multiple services

If you cloned the project, run commands from the repository root.

---

## Part 1: Input Formats (`-f`, `-j`)

### Explicit Format Selection Required

**Important:** Kelora does **NOT** auto-detect format based on filename. The default is `-f line` (plain text). You must specify the format explicitly.

Let's see what happens without specifying the format:

=== "Command"

    ```bash
    kelora examples/simple_json.jsonl --take 2
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora examples/simple_json.jsonl --take 2
    ```

Notice it treats the entire JSON line as plain text (`line='...'`). Now with `-j`:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl --take 2
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl --take 2
    ```

**Three ways to read JSON logs:**

```bash
kelora -f json examples/simple_json.jsonl    # Explicit format
kelora -j examples/simple_json.jsonl         # -j is shortcut for -f json
kelora -f auto examples/simple_json.jsonl    # Auto-detect by examining content
```

**Key Points:**

- ✅ `-f auto` detects format by **examining the content** (not filename)
- ❌ Kelora does **NOT** look at file extensions (`.jsonl`, `.log`, `.csv`)
- ✅ Default is always `-f line` unless you specify otherwise
- ✅ Best practice: Be explicit with `-j` for JSON

### Common Input Formats

```bash
-f json         # JSON lines (or use -j shortcut)
-f logfmt       # key=value format
-f combined     # Apache/Nginx access logs
-f syslog       # Syslog format (RFC3164/RFC5424)
-f csv          # CSV with header
-f tsv          # Tab-separated values
-f line         # Plain text (default)
-f auto         # Auto-detect by content
```

---

## Part 2: Understanding the Default Display

Let's examine what Kelora shows by default:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl --take 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl --take 3
    ```

**The default output format shows:**

- ✅ **Field names and values** in `key='value'` format
- ✅ **Automatic wrapping** - long events wrap with indentation
- ✅ **Colors** (when terminal supports it)
- ✅ **Smart ordering** - timestamp, level, message first, then others alphabetically

**Key observations:**

1. Strings are quoted (`'Application started'`)
2. Numbers are not quoted (`max_connections=50`)
3. Fields wrap to next line when too long
4. Each event is separated by a blank line

---

## Part 3: Display Modifiers (`-b`, `-c`, `-k`, `-K`)

### Brief Mode (`-b`) - Values Only

Omit field names, show only values for compact output:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -b --take 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -b --take 3
    ```

**Use `-b` when:** You want compact, grep-friendly output.

### Core Fields (`-c`) - Essentials Only

Show only timestamp, level, and message:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -c --take 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -c --take 3
    ```

**Use `-c` when:** You want to focus on the essentials, hiding extra metadata.

### Select Fields (`-k`) - Choose What to Show

Choose exactly which fields to show (and in what order):

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -k level,service,message --take 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -k level,service,message --take 3
    ```

**Pro tip:** Fields appear in the order you specify!

### Exclude Fields (`-K`) - Hide Sensitive Data

Remove specific fields (like passwords, tokens, or verbose metadata):

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -K service,version --take 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -K service,version --take 3
    ```

**Use `-K` when:** Hiding sensitive data (passwords, API keys) or reducing noise.

---

## Part 4: Level Filtering (`-l`, `-L`)

### Include Levels (`-l`) - Show Only Specific Log Levels

Filter to show only errors and warnings:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -l error,warn
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -l error,warn
    ```

**Common patterns:**

```bash
kelora -j app.log -l error                    # Errors only
kelora -j app.log -l error,warn,critical      # Problems only (case-insensitive)
kelora -j app.log -l info                     # Application flow (skip debug noise)
```

### Exclude Levels (`-L`) - Hide Debug Noise

Remove verbose log levels:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -L debug,info --take 5
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -L debug,info --take 5
    ```

**Use `-L` when:** You want to exclude chatty debug/trace output.

---

## Part 5: Output Formats (`-F`, `-J`)

The default `key='value'` format is great for reading, but sometimes you need machine-readable output.

### JSON Output (`-F json` or `-J`)

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -J --take 2
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -J --take 2
    ```

**Use JSON when:** Piping to `jq`, saving to file, or integrating with other tools.

### CSV Output (`-F csv`)

Perfect for spreadsheet export:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -F csv -k timestamp,level,service,message --take 4
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -F csv -k timestamp,level,service,message --take 4
    ```

**Use CSV when:** Exporting to Excel, Google Sheets, or data analysis tools.

### Logfmt Output (`-F logfmt`)

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -F logfmt --take 2
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -F logfmt --take 2
    ```

**Use logfmt when:** You want parseable output that's also human-readable.

### Inspect Output (`-F inspect`) - Debug with Types

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -F inspect --take 1
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -F inspect --take 1
    ```

**Use inspect when:** Debugging type mismatches or understanding field types.

### No Output (`-F none`) - Stats Only

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -F none --stats
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -F none --stats
    ```

**Use `-F none --stats` when:** You want to analyze log structure without seeing the events.

---

## Part 6: Practical Combinations

### Exercise 1: Find Errors, Show Essentials

Show only errors with just timestamp, service, and message:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -l error -k timestamp,service,message
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -l error -k timestamp,service,message
    ```

### Exercise 2: Export Problems to CSV

Export warnings and errors to CSV for Excel analysis:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -l error,warn -k timestamp,level,service,message -F csv
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -l error,warn -k timestamp,level,service,message -F csv
    ```

### Exercise 3: Compact View Without Debug

Brief output excluding debug noise:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -L debug -b --take 5
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -L debug -b --take 5
    ```

---

## Quick Reference Cheat Sheet

### Input Formats
```bash
-f json         # JSON lines (or use -j shortcut)
-f logfmt       # key=value format
-f combined     # Apache/Nginx access logs
-f syslog       # Syslog format
-f csv          # CSV with header
-f line         # Plain text (default)
-f auto         # Auto-detect by content
```

### Display Modifiers
```bash
-b              # Brief: values only, no field names
-c              # Core: timestamp + level + message only
-k level,msg    # Keys: show only these fields (in this order)
-K password,ip  # Exclude: hide these fields
```

### Level Filtering
```bash
-l error,warn   # Include: show only these levels
-L debug,trace  # Exclude: hide these levels
```

### Output Formats
```bash
-F default      # Pretty key='value' with colors (default)
-F json         # JSON lines (or use -J shortcut)
-F csv          # CSV with header
-F logfmt       # Logfmt key=value
-F inspect      # Debug with types
-F none         # No output (use with --stats)
```

---

## Understanding the Pipeline Order

Kelora processes your options in this order:

```
1. Read file      (-f json, -j)
2. Filter levels  (-l error, -L debug)
3. Select fields  (-k, -K, -c)
4. Format output  (-F csv, -J, -b)
5. Write output   (stdout or -o file)
```

**This means:**

- `-l` filters happen **before** `-k` (you can filter on fields you won't see in output)
- `-b` affects display, not what gets filtered
- `-F none --stats` still processes everything, just doesn't show events

---

## Common Workflows

### Error Analysis Pipeline
```bash
kelora -j app.log -l error -k timestamp,service,message -F csv -o errors.csv
# Filter → Select fields → Export to CSV → Save to file
```

### Quick Scan (Hide Noise)
```bash
kelora -j app.log -L debug,trace -b --take 20
# Exclude verbose levels → Brief output → First 20 events
```

### Investigation Mode (Full Detail)
```bash
kelora -j app.log -l warn,error,critical -K password,token
# Show problems → Hide sensitive data → Keep all other fields
```

### Stats-Only Analysis
```bash
kelora -j app.log -F none --stats
# No event output → Show processing statistics
```

---

## When to Use What

| Goal | Use | Example |
|------|-----|---------|
| **Find errors fast** | `-l error` | `kelora -j app.log -l error -c` |
| **Hide debug spam** | `-L debug,trace` | `kelora -j app.log -L debug` |
| **Export to Excel** | `-F csv` | `kelora -j app.log -F csv -o report.csv` |
| **Pipe to jq** | `-J` | `kelora -j app.log -J \| jq '.level'` |
| **Quick scan** | `-b --take 20` | `kelora -j app.log -b --take 20` |
| **Hide secrets** | `-K password,token` | `kelora -j app.log -K password,apikey` |
| **See types** | `-F inspect` | `kelora -j app.log -F inspect` |

---

## Next Steps

Once you're comfortable with these basics, continue to:

- **[Working with Time](working-with-time.md)** - Time filtering with `--since` and `--until`
- **[Scripting Transforms](scripting-transforms.md)** - Custom filters and transformations with Rhai
- **[Metrics and Tracking](metrics-and-tracking.md)** - Aggregate data with `track_*()` functions
- **[Parsing Custom Formats](parsing-custom-formats.md)** - Handle non-standard log formats
