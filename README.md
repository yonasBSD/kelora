# Kelora

Scriptable log processor for the command line. Treats logs as structured events and lets you filter, transform, and analyze them using embedded [Rhai](https://rhai.rs) scripts with 40+ built-in functions.

> [!WARNING]  
> Experimental tool. [Vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding). APIs may change without notice.

## How It Works

Kelora parses log lines into structured events (`e.level`, `e.timestamp`, `e.message`), then processes them through a pipeline: filters decide which events to keep, exec scripts transform the data, and formatters produce output. It's a programmable Unix pipeline for log data.

## Quick Start

```bash
# Find all errors in the last hour
kelora --since 1h -l error app.log

# Filter and enrich JSON logs  
kelora -f json app.log --filter 'e.status.to_int() >= 500' --exec 'e.severity = if e.status.to_int() >= 500 { "critical" } else { "normal" }'

# Parse structured log columns directly
echo "2025-09-22 INFO alice login success" | kelora -f 'cols:date level user action *details' --keys user,action

# Count HTTP status codes with metrics
kelora -f combined --exec 'track_count("status_" + e.status)' --metrics access.log

# Real-time monitoring with pattern detection
kubectl logs -f app | kelora -j --parallel --levels warn,error

# Process gzipped logs (auto-detects compression for files and pipes)
kelora -f json --filter 'e.level == "error"' app.log.gz
cat app.log.gz | kelora -f json --filter 'e.level == "error"'

# Monitor for brute force attacks using sliding windows
kelora -j auth.log --window 3 --filter 'e.event == "login_failed"' \
  --exec 'if window_values(window, "user").len() >= 2 { eprint("🚨 Brute force detected from " + e.ip) }'

# Analyze entire JSON configuration as single document
kelora -f json -M whole config.json --exec 'e.user_count = e.users.len(); e.admin_count = e.users.filter(|u| u.role == "admin").len()'
```

## Install

```bash
# Install from crates.io
cargo install kelora

# Or build from source
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```

Pre-built binaries are available in the [releases section](https://github.com/dloss/kelora/releases) of this GitHub repository.

## Core Concepts

**Events** are structured objects created from log lines. Access fields like `e.level` or `e["content-type"]`, add new ones with `e.severity = "critical"`. Works like JSON objects your scripts can read and modify.

**Pipeline** processes data through independent stages: `Input → Parse → Filter → Transform → Format → Output`. Mix any parser with any script with any formatter.

**Scripts** provide programmable logic:
- **Filters**: Boolean expressions (`e.status.to_int() >= 500`) that decide which events to keep
- **Execs**: Transform statements (`e.category = "error"`) that modify events  
- **Windows**: Access recent events (`window[1].user`) for pattern detection

## Common Tasks

**Finding Problems**: Filter by criteria with `-l error`, `--filter 'e.status.to_int() >= 500'`, or time ranges like `--since 1h --levels error,fatal`.

**Understanding Patterns**: Count and measure with `--exec 'track_count("by_status_" + e.status)'`, track averages with `track_avg("response_time", e.duration)`. View results with `--metrics` or `--stats`.

**Detecting Sequences**: Use `--window N` to access recent events. Detect changes with `window[0].status != window[1].status` or patterns like `window_values(window, "user").len() >= 2` for repeated users.

**Transforming Data**: Add/modify fields with `--exec 'e.severity = if e.status.to_int() >= 500 { "critical" } else { "normal" }'`. Chain transformations with multiple `--exec` statements.

## Input Formats

Each parser creates events with different fields:

|Format        |Fields Created                      |Example                              |
|--------------|------------------------------------|------------------------------------|
|`line`        |`line`                              |Clean text processing                |
|`raw`         |`raw`                               |Preserves text artifacts for analysis|
|`json`        |All JSON keys                       |`{"level":"info","msg":"started"}`  |
|`logfmt`      |Key-value pairs                     |`level=info msg="started" user=alice`|
|`syslog`      |`timestamp`, `host`, `facility`, `message`|`Jan 15 14:30:45 host app: message`|
|`cef`         |`vendor`, `product`, `severity` + extensions|ArcSight CEF logs               |
|`csv/tsv`     |Column headers as fields            |Structured data files               |
|`combined`    |`ip`, `status`, `request`, `method`, `path`, `request_time`|Apache/NGINX web server logs|
|`cols:<spec>` |Fields defined by column spec       |`-f 'cols:ts(2) level *msg'`        |

All formats support gzip compression automatically (detects magic bytes `1F 8B 08`) for both files and stdin. No additional flags needed - works with pipes, `.gz` files, and files without extensions. Use `-f format` to specify (`-j` is a shortcut for `-f json`).

### Raw vs Line Format

Choose between text preservation and clean output:

**Raw Format** (`-f raw`): Preserves ALL text artifacts including newlines, backslashes, and carriage returns. Perfect for text analysis where you need to count lines or preserve exact formatting:

```bash
# Count lines in multiline events
kelora -f raw -M whole --exec 'e.line_count = e.raw.split("\\n").len()' file.txt

# Preserve backslash continuations for analysis
kelora -f raw --exec 'if e.raw.contains("\\\\") { e.has_continuation = true }' file.txt
```

**Line Format** (`-f line`): Provides clean, readable output by replacing newlines with spaces. Better for general processing and display:

```bash
# Clean multiline processing
kelora -f line -M timestamp --filter 'e.line.contains("error")' file.txt
```

Use Raw when you need the full text structure, Line when you want clean output.

### Prefix Extraction

Extract prefixed text from logs before parsing with `--extract-prefix FIELD`. Useful for Docker Compose logs, service-prefixed logs, and any format with separators:

```bash
# Docker Compose logs: "web_1 | message"
docker compose logs | kelora --extract-prefix service --filter 'e.service == "web_1"'

# Custom separator: "auth-service :: message" 
kelora --extract-prefix service --prefix-sep " :: " --filter 'e.service.contains("auth")' app.log

# Works with any format
kelora -f json --extract-prefix container input.log
```

Prefix extraction runs before parsing, so the extracted prefix becomes a field in the parsed event. Default separator is `|`, configurable with `--prefix-sep`.

### Column Format (`cols:<spec>`)

Parse delimited text into structured fields using declarative specifications. Syntax: `name` (single), `name(n)` (multiple joined), `-` (skip), `*name` (remainder, must be last).

```bash
# Basic: whitespace-separated columns
kelora -f 'cols:date level user action *details' app.log

# Custom separator: pipe-delimited
kelora -f 'cols:ip user timestamp *request' --cols-sep '|' app.log

# Field grouping and skipping
kelora -f 'cols:ts(2) level - *msg' app.log  # ts="date time", skip 4th column

# Integration with processing pipeline
kelora -f 'cols:date level user action' --filter 'e.level == "ERROR"' \
       --exec 'track_count("user_" + e.user)' --metrics app.log
```

Default: whitespace splitting. Use `--cols-sep 'delim'` for custom separators (supports multi-character). Missing columns become empty/null (resilient mode) or fail with `--strict`.

## Built-in Functions

**Text Extraction**: `extract_re(pattern)` finds regex matches, `extract_ip()` pulls IP addresses, `parse_kv("=", ";")` converts key-value pairs to fields.

**Safe Conversion**: `to_number()`, `to_bool()` safely convert types, `mask_ip(octets)` anonymizes IPs, `upper()`, `lower()`, `trim()` normalize text.

**Time Operations**: `to_datetime(string, format, timezone)` converts text into a timestamp, `to_duration("5m")` converts strings into durations, `now_utc()` gets current time.

**Array Processing**: `emit_each(array)` fans out arrays into individual events, `emit_each(array, base)` adds common fields to each. Transforms nested data like `{"users": [{"name": "alice"}, {"name": "bob"}]}` into separate events for each user. Original event is suppressed.

**Column Mapping**: For programmatic use, `parse_cols()` is available in Rhai scripts: `e = e.line.parse_cols("ts(2) level *msg")` or with custom separators `e.line.parse_cols("spec", "|")`. For most use cases, the direct `-f cols:<spec>` format is recommended (see Column Format section above).

**Metrics**: `track_count(key)` increments counters, `track_sum/avg/min/max(key, value)` accumulate statistics, `track_unique(key, value)` counts distinct values. Access via `metrics` map in `--end` scripts or display with `--metrics`.

**Output**: Use `eprint()` for alerts and diagnostics (writes to stderr), `print()` for data output (writes to stdout). Since kelora's processed events go to stdout, `eprint()` prevents interference with the data pipeline.

**Environment**: `get_env(var)` returns environment variable or empty string, `get_env(var, default)` with fallback. Useful for CI/CD pipelines and configuration-driven processing.

## Advanced Features

### Window Analysis
Access recent events with `--window N`. Use `window[0]` (current), `window[1]` (previous), etc. Window helper: `window_values(window, "field")` extracts field values from all events.

```bash
# Detect status changes
kelora --window 2 --exec 'if window[0].status != window[1].status { eprint("Status changed") }' app.log

# Brute force detection (3+ failures from same IP)
kelora --window 5 --filter 'e.event == "login_failed"' \
  --exec 'let ips = window_values(window, "ip"); if ips.len() >= 3 { eprint("Brute force: " + e.ip) }' auth.log
```

### Multi-Stage Processing
Chain filters and execs in any order for complex pipelines:

```bash
# Error analysis: filter → extract → classify → count
kelora -l error \
       --exec 'e.error_class = e.message.extract_re("(\\w+Error)")' \
       --filter 'e.error_class != ""' \
       --exec 'track_count("by_class_" + e.error_class)' \
       --metrics app.log
```

### Output Control
- **Fields**: `-k field1,field2` (include only), `-K field3` (exclude), `-c` (core fields only), `-b` (brief/values only)
- **Levels**: `-l error,warn` (include), `-L debug,trace` (exclude)  
- **Time**: `--since 1h`, `--until 5m`, `--since "2024-01-15 14:00"`
- **Formats**: `-F default|inspect|json|logfmt|levelmap|csv|tsv|csvnh|tsvnh|none` (default is `default`; `inspect` shows typed field breakdown, prefixes each event with `---`, and honors `-v` for longer values), `-J` (json shortcut)

### Performance & Configuration
- **Processing**: `--parallel` for batch files (2-10x faster), `--threads N`, `--batch-size N`
- **Timezones**: `--input-tz Europe/Berlin` (parse), `-z` (display local), `-Z` (display UTC)  
- **Multiline**: Start with presets like `-M stacktrace`, `-M docker`, `-M syslog`, `-M nginx`, or `-M continuation`; run `kelora --help-multiline` for advanced recipes and custom patterns
- **Scripts**: `-E script.rhai` (from file), `--begin 'conf.config = ...'` (initialization), `--end 'print(metrics.total)'` (final reporting)
- **Error Handling**: Default is resilient (skip errors), `--strict` for fail-fast, `--verbose` for details, `--no-emoji` to disable emoji prefixes
- **Verbose Output**: Uses standardized emoji prefixes - 🔹 (blue diamond) for general output like stats and processing messages, ⚠️  (warning) for errors and warnings
- **Config**: `~/.config/kelora/config.ini` for defaults and aliases, `--config-file path/to/config.ini` for custom config, `--show-config` to view

## Complete Examples

### End-to-End Log Analysis Pipeline

```bash
# Real-time nginx monitoring: stdin → filter → transform → metrics → alert
tail -f /var/log/nginx/access.log | \
  kelora -f combined \
    --exec 'let status = e.status.to_int(); e.status_class = if status >= 500 { "error" } else if status >= 400 { "client_error" } else { "ok" }' \
    --filter 'e.status.to_int() >= 400' \
    --exec 'track_count("errors"); track_unique("error_ips", e.ip); track_avg("error_response_time", e.request_time)' \
    --exec 'if e.status.to_int() >= 500 { eprint("🚨 SERVER ERROR: " + e.status + " from " + e.ip + " - " + e.request) }' \
    --metrics
```

### Security Analysis

```bash
# Comprehensive authentication monitoring
kelora -j auth.json \
  --exec 'track_count("total_attempts"); track_unique("attempted_users", e.username)' \
  --filter 'e.auth_result == "failed"' \
  --exec 'track_count("failed_attempts"); track_unique("failed_ips", e.remote_addr)' \
  --window 5 \
  --exec 'let recent_failures = 0;
           for event in window { if event.auth_result == "failed" && event.remote_addr == e.remote_addr { recent_failures += 1; } }
           if recent_failures >= 3 { eprint("🚨 BRUTE FORCE: " + e.remote_addr + " - " + recent_failures + " failures") }' \
  --metrics
```

### Data Transformation

```bash
# Convert and enrich syslog to structured JSON
kelora -f syslog -J /var/log/messages \
  --exec 'e.severity_level = if e.severity <= 3 { "critical" } else if e.severity <= 4 { "error" } else { "info" }' \
  --exec 'e.masked_host = e.host.mask_ip(1)' \
  --exec 'e.processed_at = now_utc().format("%+")' \
  > structured-logs.json
```

### Array Fan-Out Processing

```bash
# Process nested JSON arrays: each user becomes a separate event
echo '{"batch_id": "b123", "users": [{"name": "alice", "role": "admin"}, {"name": "bob", "role": "user"}]}' | \
  kelora -f json --exec 'let base = #{batch_id: e.batch_id, processed: true}; emit_each(e.users, base)' -F json
# Output: {"name": "alice", "role": "admin", "batch_id": "b123", "processed": true}
#         {"name": "bob", "role": "user", "batch_id": "b123", "processed": true}

# Multi-level fan-out: batches → requests → errors (only for failed requests)
echo '{"batches": [{"id": "b1", "requests": [{"url": "/api", "status": 500, "errors": [{"type": "timeout"}, {"type": "db_error"}]}, {"url": "/health", "status": 200, "errors": []}]}]}' | \
  kelora -f json \
    --exec 'emit_each(e.batches)' \
    --exec 'emit_each(e.requests)' \
    --filter 'e.status.to_int() >= 400' \
    --exec 'let ctx = #{url: e.url, status: e.status}; emit_each(e.errors, ctx)'
# Result: type='timeout' url='/api' status=500
#         type='db_error' url='/api' status=500
```

## Learning Kelora (Recommended Path)

### Start Here: The Essentials
1. **Events** - understand that logs become structured objects (`e.field`)
2. **Parsing** - see how different formats create different fields (`-f json`, `-f combined`)
3. **Basic Scripts** - learn to filter (`--filter`) and transform (`--exec`)

### Next: Real-World Usage  
4. **Metrics** - track counts and calculations across events (`track_count`, `--metrics`)
5. **Pipelines** - combine multiple processing steps (multiple `--filter` and `--exec`)
6. **Output Formats** - control how results are displayed (`-F json`, `-k field1,field2`)

### Advanced: Pattern Detection
7. **Windows** - access sequences of events for pattern matching (`--window N`)
8. **Multi-stage Processing** - complex analysis pipelines with initialization (`--begin`, `--end`)

**Why This Order**: Each concept builds naturally on the previous ones. You can't understand windows without understanding events, but you can use events productively without ever learning about windows.

## Help & Documentation

```bash
kelora --help              # Full CLI reference
kelora --help-time         # Timestamp format guide
kelora --help-rhai         # Rhai scripting reference  
kelora --help-functions    # Built-in function reference
kelora --help-multiline    # Multiline strategy guide
kelora --show-config       # Current configuration
```

### Multiline Presets

| Preset | When to use it | Equivalent explicit form |
| --- | --- | --- |
| `stacktrace` | App logs or stack traces that start with ISO or syslog timestamps | `-M timestamp` |
| `docker` | Docker JSON logs with RFC3339 timestamps | `-M timestamp:pattern=^\d{4}-\d{2}-\d{2}T` |
| `syslog` | RFC3164/5424 headers such as `Jan  2` or `<165>1 2024-01-02T...` | `-M timestamp:pattern=^(<\d+>\d\s+\d{4}-\d{2}-\d{2}T|\w{3}\s+\d{1,2})` |
| `nginx` | Access/error logs with bracketed datetimes | `-M timestamp:pattern=^\[[0-9]{2}/[A-Za-z]{3}/[0-9]{4}:` |
| `continuation` | Entries that end with a continuation marker (`\` by default) | `-M backslash` |
| `block` | Sections delimited by `BEGIN`/`END` markers | `-M boundary:start=^BEGIN:end=^END` |

Use `--no-multiline` to disable format defaults. Switch to the advanced recipes in `kelora --help-multiline` when you need custom boundaries.

### Configuration File Example

Create `~/.config/kelora/config.ini`:
```ini
# Set default arguments for all kelora commands  
defaults = --format auto --stats --parallel --input-tz UTC

[aliases]
errors = -l error --since 1h --stats 
warnings = --filter 'e.level == "warn" || e.level == "warning"'
slow-queries = --filter 'e.duration > 1000' --exec 'e.slow = true' --keys timestamp,query,duration
```

Usage:
```bash
kelora app.log                          # Uses defaults: auto format, stats, parallel processing
kelora --config-file custom.ini app.log # Uses custom config  
kelora --no-stats app.log               # Overrides defaults: no stats
kelora -a errors app.log                # Uses 'errors' alias from config
```

## Kelora vs Other Tools (When to Use What)

**Kelora's Purpose**: Transform and analyze structured log events with programmable logic

**Choose Kelora When**: You need to filter, transform, or analyze log data programmatically

**Choose Other Tools When**:
- **Browsing/Exploring** → `lnav`: Purpose is interactive log exploration with syntax highlighting
- **Simple Text Search** → `ripgrep`: Purpose is fast pattern matching across files  
- **Complex JSON** → `jq`: Purpose is sophisticated JSON querying and transformation
- **Visualization** → Grafana: Purpose is creating dashboards and charts
- **Log Collection** → Fluentd: Purpose is shipping logs between systems

**The Independence Principle**: You can pipe Kelora's output to these tools - they complement rather than compete with each other.

### Similar Tools in the Log Processing Space

**Log Processing**:

- [angle-grinder](https://github.com/rcoh/angle-grinder) - Rust-based log processor with query syntax
- [lnav](https://github.com/tstack/lnav) - Advanced log viewer with many formats
- [pq](https://github.com/iximiuz/pq) - Log parser and query tool

**Text Processing**:

- [Miller](https://github.com/johnkerl/miller) - Name-indexed data processor (CSV, JSON, etc.)
- [jq](https://jqlang.github.io/jq/) - JSON processor and query language

## License

[MIT](LICENSE) - See LICENSE file for details.
