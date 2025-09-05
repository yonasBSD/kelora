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
kelora -f jsonl app.log --filter 'e.status >= 500' --exec 'e.severity = "critical"'

# Count HTTP status codes with metrics
kelora -f combined --exec 'track_count("status_" + e.status)' --metrics access.log

# Real-time monitoring with pattern detection
kubectl logs -f app | kelora -j --parallel --levels warn,error

# Monitor for brute force attacks using sliding windows
kelora -j auth.log --window 3 --filter 'e.event == "login_failed"' \
  --exec 'if window_values(window, "user").len() >= 2 { eprint("🚨 Brute force detected from " + e.ip) }'
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
- **Filters**: Boolean expressions (`e.status >= 500`) that decide which events to keep
- **Execs**: Transform statements (`e.category = "error"`) that modify events  
- **Windows**: Access recent events (`window[1].user`) for pattern detection

## Common Tasks

**Finding Problems**: Filter by criteria with `-l error`, `--filter 'e.status >= 500'`, or time ranges like `--since 1h --levels error,fatal`.

**Understanding Patterns**: Count and measure with `--exec 'track_count("by_status_" + e.status)'`, track averages with `track_avg("response_time", e.duration)`. View results with `--metrics` or `--stats`.

**Detecting Sequences**: Use `--window N` to access recent events. Detect changes with `window[0].status != window[1].status` or patterns like `window_values(window, "user").len() >= 2` for repeated users.

**Transforming Data**: Add/modify fields with `--exec 'e.severity = if e.status >= 500 { "critical" } else { "normal" }'`. Chain transformations with multiple `--exec` statements.

## Input Formats

Each parser creates events with different fields:

|Format        |Fields Created                      |Example                              |
|--------------|------------------------------------|------------------------------------|
|`line`        |`line`                              |Raw log files                       |
|`jsonl`       |All JSON keys                       |`{"level":"info","msg":"started"}`  |
|`logfmt`      |Key-value pairs                     |`level=info msg="started" user=alice`|
|`syslog`      |`timestamp`, `host`, `facility`, `message`|`Jan 15 14:30:45 host app: message`|
|`cef`         |`vendor`, `product`, `severity` + extensions|ArcSight CEF logs               |
|`csv/tsv`     |Column headers as fields            |Structured data files               |
|`combined`    |`ip`, `status`, `request`, `method`, `path`, `request_time`|Apache/NGINX web server logs|

All formats support gzip compression. Use `-f format` to specify (`-j` is a shortcut for `-f jsonl`).

### Prefix Extraction

Extract prefixed text from logs before parsing with `--extract-prefix FIELD`. Useful for Docker Compose logs, service-prefixed logs, and any format with separators:

```bash
# Docker Compose logs: "web_1 | message"
docker compose logs | kelora --extract-prefix service --filter 'e.service == "web_1"'

# Custom separator: "auth-service :: message" 
kelora --extract-prefix service --prefix-sep " :: " --filter 'e.service.contains("auth")' app.log

# Works with any format
kelora -f jsonl --extract-prefix container input.log
```

Prefix extraction runs before parsing, so the extracted prefix becomes a field in the parsed event. Default separator is `|`, configurable with `--prefix-sep`.

## Built-in Functions

**Text Extraction**: `extract_re(pattern)` finds regex matches, `extract_ip()` pulls IP addresses, `parse_kv("=", ";")` converts key-value pairs to fields.

**Safe Conversion**: `to_number()`, `to_bool()` safely convert types, `mask_ip(octets)` anonymizes IPs, `upper()`, `lower()`, `trim()` normalize text.

**Time Operations**: `parse_timestamp(string, format, timezone)` handles custom timestamps, `parse_duration("5m")` converts to seconds, `now_utc()` gets current time.

**Metrics**: `track_count(key)` increments counters, `track_sum/avg/min/max(key, value)` accumulate statistics, `track_unique(key, value)` counts distinct values. Access via `tracked` map in `--end` scripts or display with `--metrics`.

**Output**: Use `eprint()` for alerts and diagnostics (writes to stderr), `print()` for data output (writes to stdout). Since kelora's processed events go to stdout, `eprint()` prevents interference with the data pipeline.

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
- **Formats**: `-F jsonl|logfmt|csv|none` (default is colored logfmt), `-J` (jsonl shortcut)

### Performance & Configuration
- **Processing**: `--parallel` for batch files (2-10x faster), `--threads N`, `--batch-size N`
- **Timezones**: `--input-tz Europe/Berlin` (parse), `-z` (display local), `-Z` (display UTC)  
- **Multiline**: `-M timestamp` (Java stacks), `-M indent` (continuation lines), `-M backslash` (line continuation)
- **Scripts**: `-E script.rhai` (from file), `--begin 'init.config = ...'` (initialization), `--end 'print(tracked.total)'` (final reporting)
- **Error Handling**: Default is resilient (skip errors), `--strict` for fail-fast, `--verbose` for details, `--no-emoji` to disable emoji prefixes
- **Verbose Output**: Uses standardized emoji prefixes - 🔹 (blue diamond) for general output like stats and processing messages, 🔸 (orange diamond) for errors and warnings
- **Config**: `~/.config/kelora/config.ini` for defaults and aliases, `--config-file path/to/config.ini` for custom config, `--show-config` to view

## Complete Examples

### End-to-End Log Analysis Pipeline

```bash
# Real-time nginx monitoring: stdin → filter → transform → metrics → alert
tail -f /var/log/nginx/access.log | \
  kelora -f combined \
    --exec 'e.status_class = if e.status >= 500 { "error" } else if e.status >= 400 { "client_error" } else { "ok" }' \
    --filter 'e.status >= 400' \
    --exec 'track_count("errors"); track_unique("error_ips", e.ip); track_avg("error_response_time", e.request_time)' \
    --exec 'if e.status >= 500 { eprint("🚨 SERVER ERROR: " + e.status + " from " + e.ip + " - " + e.request) }' \
    --metrics
```

### Security Analysis

```bash
# Comprehensive authentication monitoring
kelora -j auth.jsonl \
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
  --exec 'e.processed_at = now_utc()' \
  > structured-logs.jsonl
```

## Learning Kelora (Recommended Path)

### Start Here: The Essentials
1. **Events** - understand that logs become structured objects (`e.field`)
2. **Parsing** - see how different formats create different fields (`-f jsonl`, `-f combined`)
3. **Basic Scripts** - learn to filter (`--filter`) and transform (`--exec`)

### Next: Real-World Usage  
4. **Metrics** - track counts and calculations across events (`track_count`, `--metrics`)
5. **Pipelines** - combine multiple processing steps (multiple `--filter` and `--exec`)
6. **Output Formats** - control how results are displayed (`-F jsonl`, `-k field1,field2`)

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
kelora --show-config       # Current configuration
```

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