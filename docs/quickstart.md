# Quickstart

Get started with Kelora in 5 minutes. This guide will walk you through parsing, filtering, and transforming logs using real example files.

## Prerequisites

- Kelora installed and on your PATH
- Clone the repository to access example files: `git clone https://github.com/dloss/kelora`

## Step 1: Parse JSON Logs

Let's start with a simple JSON log file. Parse it and see all events:

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl --take 3
```

The `-f json` flag tells Kelora to parse each line as JSON. By default, Kelora outputs events in `key=value` format. The `--take 3` flag limits output to the first 3 events.

## Step 2: Filter by Log Level

Show only error-level events:

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl --levels error
```

The `--levels` flag filters events by their log level field. You can specify multiple levels: `--levels warn,error`.

## Step 3: Select Specific Fields

Extract just the fields you care about:

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  --keys timestamp,service,message \
  --take 3
```

The `--keys` flag limits output to specified top-level fields. Use `--brief` to show only values:

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  --keys timestamp,service,message \
  --brief \
  --take 3
```

## Step 4: Filter with Custom Logic

Use Rhai scripts to filter events with custom conditions:

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  --filter 'e.service == "database"' \
  --keys timestamp,service,message
```

The `--filter` flag evaluates a Rhai expression. Events where the expression returns `true` are kept.

## Step 5: Transform Event Data

Add computed fields using `--exec`:

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  --filter 'e.service == "database"' \
  --exec 'e.duration_s = e.get_path("duration_ms", 0) / 1000' \
  --keys timestamp,message,duration_s
```

The `--exec` flag runs Rhai code to modify events. Here we convert milliseconds to seconds.

## Step 6: Track Metrics

Count events by service and show metrics, suppressing event output:

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  --exec 'track_count(e.service)' \
  -F none \
  --metrics
```

The `track_count()` function increments a counter for each unique value. The `-F none` flag suppresses event output, and `--metrics` displays the accumulated counts.

## Step 7: Convert Between Formats

Kelora can convert logs from any input format to any output format:

### Syslog to JSON

```bash exec="on" source="above" result="ansi"
kelora -f syslog examples/simple_syslog.log -F json --take 3
```

### Logfmt to JSON

```bash exec="on" source="above" result="ansi"
kelora -f logfmt examples/simple_logfmt.log -F json --take 3
```

### Apache/Nginx Logs to CSV

```bash exec="on" source="above" result="ansi"
kelora -f combined examples/web_access_large.log.gz \
  -F csv \
  --keys ip,status,request \
  --take 3
```

The `-f` flag specifies input format, `-F` specifies output format. Kelora normalizes all formats to a common event structure.

## Common Patterns

### Stream Processing

Process logs as they're written:

=== "Linux/macOS"

    ```bash
    > tail -f /var/log/app.log | kelora -j --levels error
    ```

=== "Windows"

    ```powershell
    > Get-Content -Wait app.log | kelora -j --levels error
    ```

### Gzipped Files

Kelora automatically decompresses `.gz` files:

```bash
> kelora -f json app.log.gz --levels error
```

### Multiple Files

Process multiple files in sequence:

```bash
> kelora -f json logs/*.jsonl --levels error
```

## Next Steps

Now that you've seen the basics, dive deeper:

- **[Tutorials](tutorials/parsing-custom-formats.md)** - Learn core skills step-by-step
- **[How-To Guides](how-to/find-errors-in-logs.md)** - Solve specific problems
- **[Function Reference](reference/functions.md)** - Explore all 40+ built-in functions
- **[CLI Reference](reference/cli-reference.md)** - Complete flag documentation

## Quick Recipes

Need a refresher later? These bite-sized snippets mirror the built-in fixtures so
you can rehearse common tasks quickly. Run them from the repository root so the
`examples/` paths resolve.

### Narrow to a specific service

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  --filter 'e.service == "database"' \
  --exec 'e.duration_s = e.get_path("duration_ms", 0) / 1000' \
  --keys timestamp,message,duration_s
```

### Slice logs by prefix before parsing

```bash exec="on" source="above" result="ansi"
cat examples/prefix_docker.log | \
  kelora --extract-prefix container --prefix-sep ' | ' \
    --filter 'e.container == "web_1"'
```

## Quick Reference

```bash
# Common flags
kelora --help              # Complete CLI reference
kelora --help-functions    # All built-in Rhai functions
kelora --help-examples     # Common usage patterns
kelora --help-rhai         # Rhai scripting guide

# Format shortcuts
-j                        # Shorthand for -f json
-f auto                   # Auto-detect format
-F json                   # Output as JSON

# Filtering
--levels error            # Filter by log level
--filter 'expression'     # Custom Rhai filter
--since "1 hour ago"      # Time-based filtering
--until "2024-01-01"      # Upper time bound

# Transformation
--exec 'expression'       # Transform events
--keys field1,field2      # Select fields
--stats                   # Show processing statistics
--metrics                 # Show tracked metrics

# Performance
--parallel                # Use multiple cores
--take 100                # Limit to first 100 events
```
