# CLI Reference

Complete command-line interface reference for Kelora. For quick start examples, see the [Quickstart Guide](../quickstart.md).

## Synopsis

```bash
kelora [OPTIONS] [FILES]...
```

## Processing Modes

Kelora supports two processing modes:

| Mode | When to Use | Characteristics |
|------|-------------|-----------------|
| **Sequential (default)** | Streaming, interactive, ordered output | Events processed one at a time in order |
| **Parallel (`--parallel`)** | High-throughput batch processing | Events processed in parallel batches across cores |

## Common Examples

```bash
# Find errors in access logs
> kelora access.log --levels error,critical

# Transform JSON logs with Rhai
> kelora -j app.json --exec 'e.duration_ms = e.end_time - e.start_time'

# Extract specific fields from NGINX logs
> kelora nginx.log -f combined --keys method,status,path
```

## Arguments

### Files

```bash
[FILES]...
```

Input files to process. If omitted, reads from stdin. Use `-` to explicitly specify stdin.

**Examples:**
```bash
> kelora app.log                    # Single file
> kelora logs/*.jsonl               # Multiple files with glob
> kelora file1.log file2.log        # Multiple files explicit
> tail -f app.log | kelora -j       # From stdin
> kelora -                          # Explicitly read stdin
```

## Global Options

### Help and Version

| Flag | Description |
|------|-------------|
| `-h, --help` | Print complete help (use `-h` for summary) |
| `-V, --version` | Print version information |

### Help Topics

| Flag | Description |
|------|-------------|
| `--help-rhai` | Rhai scripting guide and stage semantics |
| `--help-functions` | All 40+ built-in Rhai functions |
| `--help-examples` | Practical log analysis patterns |
| `--help-time` | Timestamp format reference (chrono format strings) |
| `--help-multiline` | Multi-line event detection strategies |

## Input Options

### Format Selection

#### `-f, --input-format <FORMAT>`

Specify input format. Supports standard formats, column parsing, and CSV with type annotations.

**Standard Formats:**

- `json` - JSON lines (one JSON object per line)
- `line` - Plain text (default, one line per event)
- `csv` - CSV with header row
- `tsv` - Tab-separated values with header
- `logfmt` - Key-value pairs (logfmt format)
- `syslog` - Syslog RFC5424 and RFC3164
- `combined` - Apache/Nginx log formats (Common + Combined)
- `cef` - ArcSight Common Event Format
- `auto` - Auto-detect format

**Column Parsing:**
```bash
-f 'cols:timestamp(2) level *message'
```

**CSV with Types:**
```bash
-f 'csv status:int bytes:int response_time:float'
```

**Examples:**
```bash
> kelora -f json app.log
> kelora -f combined nginx.log
> kelora -f 'cols:ts(2) level *msg' custom.log  # `ts` is auto-detected as a timestamp
```

#### `-j`

Shortcut for `-f json`.

```bash
> kelora -j app.jsonl
# Equivalent to: kelora -f json app.jsonl
```

### File Processing

#### `--file-order <FILE_ORDER>`

Control file processing order.

**Values:**

- `cli` - Process files in command-line order (default)
- `name` - Sort files alphabetically by name
- `mtime` - Sort files by modification time (oldest first)

```bash
> kelora --file-order mtime logs/*.log
```

### Line Filtering

#### `--skip-lines <N>`

Skip the first N input lines.

```bash
> kelora --skip-lines 10 app.log
```

#### `--keep-lines <REGEX>`

Keep only input lines matching regex pattern (applied before `--ignore-lines`).

```bash
> kelora --keep-lines 'ERROR|WARN' app.log
```

#### `--ignore-lines <REGEX>`

Ignore input lines matching regex pattern.

```bash
> kelora --ignore-lines '^#' app.log    # Skip comments
```

### Timestamp Configuration {#timestamp-options}

#### `--ts-field <FIELD>`

Custom timestamp field name for parsing.

```bash
> kelora -j --ts-field created_at app.log
```

#### `--ts-format <FORMAT>`

Custom timestamp format using chrono format strings. See `--help-time` for format reference.

```bash
> kelora --ts-format '%Y-%m-%d %H:%M:%S' app.log
> kelora --ts-format '%d/%b/%Y:%H:%M:%S %z' access.log
```

#### `--input-tz <TIMEZONE>`

Timezone for naive input timestamps (without timezone info). Default: UTC.

**Values:**

- `UTC` - Coordinated Universal Time
- `local` - System local time
- Named timezones: `Europe/Berlin`, `America/New_York`, etc.

```bash
> kelora --input-tz local app.log
> kelora --input-tz Europe/Berlin app.log
```

### Multi-line Events

#### `-M, --multiline <STRATEGY>`

Multi-line event detection strategy. See `--help-multiline` for details.

```bash
> kelora -M json app.log              # JSON events across lines
> kelora -M '^\\d{4}-' app.log        # Events start with date
```

### Prefix Extraction

#### `--extract-prefix <FIELD>`

Extract text before separator to specified field (runs before parsing).

```bash
> docker compose logs | kelora --extract-prefix service
```

#### `--prefix-sep <STRING>`

Separator string for prefix extraction. Default: `|`

```bash
> kelora --extract-prefix node --prefix-sep ' :: ' cluster.log
```

### Column Format Options

#### `--cols-sep <SEPARATOR>`

Column separator for `cols:<spec>` format. Default: whitespace.

```bash
> kelora -f 'cols:name age city' --cols-sep ',' data.txt
```

## Processing Options

### Scripting Stages

#### `--begin <SCRIPT>`

Run Rhai script once before processing any events. Typical use: initialize lookup tables or shared context in the global `conf` map.

**Available helpers:**

- `read_lines(path)` - Read file as array of lines
- `read_file(path)` - Read file as string

```bash
> kelora -j --begin 'conf.users = read_json("users.json")' app.log
```

#### `--filter <EXPRESSION>`

Boolean filter expression. Events where expression returns `true` are kept. Multiple filters are combined with AND logic.

```bash
> kelora -j --filter 'e.status >= 400' app.log
> kelora -j --filter 'e.service == "api"' --filter 'e.level == "ERROR"' app.log
```

#### `-e, --exec <SCRIPT>`

Transform/process script evaluated on each event. Multiple `--exec` scripts run in order.

```bash
> kelora -j --exec 'e.duration_s = e.duration_ms / 1000' app.log
> kelora -j --exec 'track_count(e.service)' app.log
```

#### `-E, --exec-file <FILE>`

Execute Rhai script from file (runs in exec stage).

```bash
> kelora -j -E transform.rhai app.log
```

#### `-I, --include <FILE>`

Include Rhai files before script stages (library imports).

```bash
> kelora -j -I helpers.rhai --exec 'e.custom = my_helper(e)' app.log
```

#### `--end <SCRIPT>`

Run once after processing completes (post-processing stage). Access global `metrics` map from `track_*()` calls here.

```bash
> kelora -j \
    --exec 'track_count(e.service)' \
    --end 'print("Total services: " + metrics.len())' \
    app.log
```

### File System Access

#### `--allow-fs-writes`

Allow Rhai scripts to create directories and write files. Required for file helpers like `append_file()` or `mkdir()`.

```bash
> kelora -j --allow-fs-writes --exec 'append_file("errors.txt", e.message)' app.log
```

### Window Functions

#### `--window <SIZE>`

Enable sliding window of N+1 recent events. Required for `window_*()` functions.

```bash
> kelora -j --window 5 --exec 'e.recent_statuses = window_values("status")' app.log
```

### Timestamp Conversion

#### `--convert-ts <FIELDS>`

Convert timestamp fields to RFC3339 format (ISO 8601 compatible). Modifies event data - affects all output formats.

```bash
> kelora -j --convert-ts timestamp,created_at app.log
```

## Error Handling Options

### Strict Mode

#### `--strict`

Exit on first error (fail-fast behavior). Parsing errors, filter errors, or exec errors will immediately abort processing.

```bash
> kelora -j --strict app.log
```

#### `--no-strict`

Disable strict mode explicitly (resilient mode is default).

### Verbosity

#### `-v, --verbose`

Show detailed error information. Use multiple times for more verbosity: `-v`, `-vv`, `-vvv`.

```bash
> kelora -j --verbose app.log
```

### Quiet Mode

#### `-q, --quiet`

Graduated quiet mode with explicit levels:

| Level | Effect |
|-------|--------|
| `-q` | Suppress kelora diagnostics (errors, stats, context markers) |
| `-qq` | Additionally suppress event output (same as `-F none`) |
| `-qqq` | Additionally suppress script side effects (`print()`, `eprint()`) |

```bash
> kelora -qq --exec 'track_count("errors")' app.log     # Only metrics
> kelora -qqq app.log; echo "Exit: $?"                  # Exit code only
```

## Filtering Options

### Level Filtering

#### `-l, --levels <LEVELS>`

Include only events with specified log levels (comma-separated, case-insensitive).

```bash
> kelora -j --levels error app.log
> kelora -j --levels error,warn,critical app.log
```

#### `-L, --exclude-levels <LEVELS>`

Exclude events with specified log levels (comma-separated, case-insensitive).

```bash
> kelora -j --exclude-levels debug,trace app.log
```

### Field Selection

#### `-k, --keys <FIELDS>`

Output only specified top-level fields (comma-separated list).

```bash
> kelora -j --keys timestamp,level,message app.log
```

#### `-K, --exclude-keys <FIELDS>`

Exclude specified fields from output (comma-separated list).

```bash
> kelora -j --exclude-keys password,token,secret app.log
```

### Time Range Filtering

#### `--since <TIME>`

Include events from this time onward. Accepts journalctl-style timestamps.

**Formats:**

- Absolute: `2024-01-15T12:00:00Z`, `2024-01-15 12:00`
- Relative: `1h`, `-30m`, `yesterday`

```bash
> kelora -j --since '1 hour ago' app.log
> kelora -j --since yesterday app.log
> kelora -j --since 2024-01-15T10:00:00Z app.log
```

#### `--until <TIME>`

Include events until this time. Accepts journalctl-style timestamps.

**Formats:**

- Absolute: `2024-01-15T12:00:00Z`, `2024-01-15 12:00`
- Relative: `1h`, `+30m`, `tomorrow`

```bash
> kelora -j --until '30 minutes ago' app.log
> kelora -j --until tomorrow app.log
> kelora -j --until 2024-01-15T18:00:00Z app.log
```

### Output Limiting

#### `-n, --take <N>`

Limit output to the first N events (after filtering).

```bash
> kelora -j --take 100 app.log
> kelora -j --levels error --take 10 app.log
```

### Context Lines

#### `-B, --before-context <N>`

Show N lines before each match (requires filtering with `--filter` or `--levels`).

```bash
> kelora -j --levels error --before-context 2 app.log
```

#### `-A, --after-context <N>`

Show N lines after each match (requires filtering).

```bash
> kelora -j --levels error --after-context 3 app.log
```

#### `-C, --context <N>`

Show N lines before and after each match (requires filtering).

```bash
> kelora -j --levels error --context 2 app.log
```

## Output Options

### Output Format

#### `-F, --output-format <FORMAT>`

Output format. Default: `default`

**Values:**

- `default` - Key-value format with colors
- `json` - JSON lines (one object per line)
- `logfmt` - Key-value pairs (logfmt format)
- `inspect` - Debug format with type information
- `levelmap` - Grouped by log level
- `csv` - CSV with header
- `tsv` - Tab-separated values with header
- `csvnh` - CSV without header
- `tsvnh` - TSV without header
- `none` - No event output (only metrics/stats)

```bash
> kelora -j -F json app.log
> kelora -j -F csv app.log
> kelora -j -F none --stats app.log
```

#### `-J`

Shortcut for `-F json`.

```bash
> kelora -j -J app.log
# Equivalent to: kelora -f json -F json app.log
```

### Output Destination

#### `-o, --output-file <FILE>`

Write formatted events to file instead of stdout.

```bash
> kelora -j -F json -o output.json app.log
```

### Core Fields

#### `-c, --core`

Output only core fields (timestamp, level, message).

```bash
> kelora -j --core app.log
```

## Default Format Options

These options only affect the default formatter (`-F default`).

### Brief Mode

#### `-b, --brief`

Output only field values (omit field names).

```bash
> kelora -j --brief app.log
```

### Nested Structures

#### `--expand-nested`

Expand nested structures (maps/arrays) with indentation.

```bash
> kelora -j --expand-nested app.log
```

### Word Wrapping

#### `--wrap`

Enable word-wrapping (default: on).

#### `--no-wrap`

Disable word-wrapping (overrides `--wrap`).

```bash
> kelora -j --no-wrap app.log
```

### Timestamp Display

#### `-z, --show-ts-local`

Display timestamps as local RFC3339 (ISO 8601 compatible). Display-only - only affects default formatter output.

```bash
> kelora -j -z app.log
# Output: 2024-01-15T10:30:00+01:00
```

#### `-Z, --show-ts-utc`

Display timestamps as UTC RFC3339 (ISO 8601 compatible). Display-only - only affects default formatter output.

```bash
> kelora -j -Z app.log
# Output: 2024-01-15T09:30:00Z
```

## Display Options

### Colors

#### `--force-color`

Force colored output (even when piping to file).

```bash
> kelora -j --force-color app.log > output.txt
```

#### `--no-color`

Disable colored output.

```bash
> kelora -j --no-color app.log
```

### Gap Markers

#### `--mark-gaps <DURATION>`

Insert centered marker when time delta between events exceeds duration.

```bash
> kelora -j --mark-gaps 30s app.log    # Mark 30+ second gaps
> kelora -j --mark-gaps 5m app.log     # Mark 5+ minute gaps
```

### Emoji

#### `--no-emoji`

Disable emoji prefixes in output.

```bash
> kelora -j --no-emoji app.log
```

## Performance Options

### Parallel Processing

#### `--parallel`

Enable parallel processing across multiple cores. Higher throughput, may reorder output.

```bash
> kelora -j --parallel app.log
```

#### `--no-parallel`

Disable parallel processing explicitly (sequential mode is default).

#### `--threads <N>`

Number of worker threads for parallel processing. Default: 0 (auto-detect cores).

```bash
> kelora -j --parallel --threads 4 app.log
```

#### `--batch-size <N>`

Batch size for parallel processing. Larger batches improve throughput but increase memory usage.

```bash
> kelora -j --parallel --batch-size 5000 app.log
```

#### `--batch-timeout <MS>`

Flush partially full batches after idle period (milliseconds). Lower values reduce latency; higher values improve throughput.

Default: 200ms

```bash
> kelora -j --parallel --batch-timeout 100 app.log
```

#### `--unordered`

Disable ordered output for maximum parallel performance.

```bash
> kelora -j --parallel --unordered app.log
```

## Metrics and Statistics

### Statistics

#### `-s, --stats`

Show processing statistics at end (default: off).

```bash
> kelora -j --stats app.log
```

#### `--no-stats`

Disable processing statistics explicitly (default: off).

#### `-S, --stats-only`

Print processing statistics only (implies `-F none`).

```bash
> kelora -j --stats-only app.log
```

### Tracked Metrics

#### `-m, --metrics`

Show metrics recorded via `track_*()` functions in Rhai scripts.

```bash
> kelora -j --exec 'track_count(e.service)' --metrics app.log
```

#### `--no-metrics`

Disable tracked metrics explicitly (default: off).

#### `--metrics-file <FILE>`

Persist metrics map to disk as JSON.

```bash
> kelora -j --exec 'track_count(e.service)' --metrics-file metrics.json app.log
```

## Configuration Options

### Configuration File

Kelora uses a configuration file for defaults and aliases. See [Configuration System](../concepts/configuration-system.md) for details.

#### `-a, --alias <ALIAS>`

Use alias from configuration file.

```bash
> kelora -a errors app.log
```

#### `--config-file <FILE>`

Specify custom configuration file path.

```bash
> kelora --config-file /path/to/custom.ini app.log
```

#### `--show-config`

Show current configuration with precedence information and exit.

```bash
> kelora --show-config
```

#### `--edit-config`

Edit configuration file in default editor and exit.

```bash
> kelora --edit-config
```

#### `--ignore-config`

Ignore configuration file (use built-in defaults only).

```bash
> kelora --ignore-config app.log
```

#### `--save-alias <NAME>`

Save current command as alias to configuration file.

```bash
> kelora -j --levels error --keys timestamp,message --save-alias errors
# Later use: kelora -a errors app.log
```

## Exit Codes

Kelora uses standard Unix exit codes:

| Code | Meaning |
|------|---------|
| `0` | Success (no errors) |
| `1` | Processing errors (parse/filter/exec errors) |
| `2` | Invalid usage (CLI errors, file not found) |
| `130` | Interrupted (Ctrl+C) |
| `141` | Broken pipe (normal in Unix pipelines) |

**Examples:**
```bash
> kelora -j app.log && echo "Clean" || echo "Has errors"
> kelora -qq app.log; echo "Exit code: $?"
```

## Environment Variables

### Configuration

- **`TZ`** - Default timezone for naive timestamps (overridden by `--input-tz`)

### Rhai Scripts

Access environment variables in scripts using `get_env()`:

```bash
> kelora -j --exec 'e.build = get_env("BUILD_ID", "unknown")' app.log
```

## Common Option Combinations

### Error Analysis

```bash
# Find errors with context
> kelora -j --levels error --context 2 app.log

# Count errors by service
> kelora -j --levels error --exec 'track_count(e.service)' --metrics app.log
```

### Performance Analysis

```bash
# Find slow requests
> kelora -f combined --filter 'e.request_time.to_float() > 1.0' nginx.log

# Track response time percentiles
> kelora -f combined \
    --exec 'track_bucket("latency", e.request_time.to_float() * 1000)' \
    --metrics nginx.log
```

### Data Export

```bash
# Export to JSON
> kelora -j -F json -o output.json app.log

# Export to CSV
> kelora -j -F csv --keys timestamp,level,service,message -o report.csv app.log
```

### Real-Time Monitoring

=== "Linux/macOS"

    ```bash
    > tail -f app.log | kelora -j --levels error,warn
    ```

=== "Windows"

    ```powershell
    > Get-Content -Wait app.log | kelora -j --levels error,warn
    ```

### High-Performance Batch Processing

```bash
# Parallel processing with optimal batch size
> kelora -j --parallel --batch-size 5000 --unordered large.log

# Compressed archives
> kelora -j --parallel logs/*.log.gz
```

## See Also

- [Quickstart Guide](../quickstart.md) - Get started in 5 minutes
- [Function Reference](functions.md) - All 40+ built-in Rhai functions
- [Pipeline Model](../concepts/pipeline-model.md) - How processing stages work
- [Configuration System](../concepts/configuration-system.md) - Configuration files and aliases
