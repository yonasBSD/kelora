# Quickstart

Get started with Kelora in minutes. This guide shows real examples from parsing to advanced transformations.

## Prerequisites

- Kelora installed and on your PATH
- Clone the repository: `git clone https://github.com/dloss/kelora`

## Parse and Filter Logs

Parse JSON logs and filter by level, showing only specific fields:

```bash exec="on" source="above" result="ansi"
kelora -j examples/simple_json.jsonl -l error -k timestamp,service,message
```

The `-j` flag parses JSON, `-l` filters by log level (comma-separated for multiple: `-l warn,error`), and `-k` selects which fields to show. Use `-b` for brief output (values only).

## Filter and Transform with Scripts

Filter events and add computed fields using Rhai expressions:

```bash exec="on" source="above" result="ansi"
kelora -j examples/simple_json.jsonl \
  --filter 'e.service == "database"' \
  -e 'e.duration_s = e.get_path("duration_ms", 0) / 1000' \
  -k timestamp,message,duration_s
```

The `--filter` keeps events where the expression returns `true`. The `-e` flag transforms events by adding or modifying fields.

## Track Metrics

Count events by service, suppressing event output:

```bash exec="on" source="above" result="ansi"
kelora -j examples/simple_json.jsonl \
  -e 'track_count(e.service)' \
  -F none -m
```

Use `track_count()`, `track_sum()`, `track_min()`, and `track_max()` to collect metrics. The `-m` flag displays results at the end.

## Convert Between Formats

Kelora converts between any formats. Examples:

```bash exec="on" source="above" result="ansi"
kelora -f syslog examples/simple_syslog.log -F json -n 3
```

```bash exec="on" source="above" result="ansi"
kelora -f combined examples/web_access_large.log.gz -F csv -k ip,status,request -n 3
```

The `-f` flag specifies input format, `-F` specifies output format (we could have used `-J` as a shortcut for JSON). Gzipped files are automatically decompressed.

## Common Patterns

```bash
# Stream processing
tail -f /var/log/app.log | kelora -j -l error

# Multiple files with wildcards
kelora -j logs/*.jsonl -l error

# Extract prefixes (Docker Compose logs, etc.)
docker compose logs | kelora --extract-prefix container --filter 'e.container == "web_1"'
```

## Next Steps

- **[Tutorials](tutorials/parsing-custom-formats.md)** - Learn core skills step-by-step
- **[How-To Guides](how-to/find-errors-in-logs.md)** - Solve specific problems
- **[Function Reference](reference/functions.md)** - Explore all 40+ built-in functions
- **[CLI Reference](reference/cli-reference.md)** - Complete flag documentation

## Quick Reference

```bash
# Get help
kelora --help              # Complete CLI reference
kelora --help-functions    # All built-in Rhai functions
kelora --help-rhai         # Rhai scripting guide

# Input/output formats
-j                        # Parse JSON (short for -f json)
-J                        # Output JSON (short for -F json)
-f auto                   # Auto-detect format

# Filtering and selection
-l error,warn             # Filter by log level
-k field1,field2          # Select specific fields
--filter 'expression'     # Custom Rhai filter
--since "1 hour ago"      # Time-based filtering

# Transformation and metrics
-e 'expression'           # Transform events with Rhai
-m                        # Show tracked metrics
-s                        # Show processing statistics

# Output control
-b                        # Brief mode (values only)
-n 100                    # Limit output (--take)
```
