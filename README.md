# Kelora

Kelora is a fast, scriptable CLI log processor built for real-world logs and clean data pipelines. It reads raw or structured logs (JSON, syslog, logfmt, CSV, etc.), and uses Rhai scripts to filter, enrich, and analyze them — all from the terminal.

---

## 🔧 Quick Start

> **⚠️ Breaking Change**: Naive timestamps are now interpreted as UTC by default (was local time). Use `--input-tz local` for the old behavior.

```bash
# Filter warnings and errors from a structured JSON log
kelora -f jsonl app.log --levels warn,error

# System logs (RHEL/Fedora: /var/log/messages; Ubuntu: /var/log/syslog)
kelora -f syslog /var/log/messages --levels warn,error
# OR
kelora -f syslog /var/log/syslog --levels warn,error

# Extract slow HTTP requests
kelora -f jsonl access.log \
  --filter 'response_time.to_int() > 1000' \
  --keys timestamp,method,path,response_time

# Filter logs by time range with timezone-aware display
kelora -f jsonl app.log \
  --since "2024-01-01T00:00:00Z" \
  --until "2024-01-01T23:59:59Z" -z

# Add a derived status class and export as CSV
kelora -f jsonl app.log \
  --exec 'let class = status_class(status)' \
  --keys timestamp,status,class -F csv

# Process compressed logs and filter server errors
kelora -f jsonl logs/app.log.1.gz \
  --filter 'status.to_int() >= 500'

# Detect repeated login failures using a sliding window
kelora -f jsonl auth.log --window 5 \
  --filter 'event_type == "login_failed"' \
  --exec 'let failures = window_values(window, "username").len(); if failures >= 3 { print("Suspicious activity") }'

# Real-time log triage from Kubernetes
kubectl logs -f app | kelora -f jsonl --levels warn,error
```

---

## 💡 Key Features

- **Rhai scripting**: powerful and readable one-liners
- **Window analysis**: look at past N events with `--window`
- **Flexible formats**: JSON, logfmt, syslog, CEF, CSV, TSV, raw lines
- **Real-time capable**: stream from `tail`, `kubectl logs`, stdin
- **Compressed input**: automatic `.gz` decompression
- **Multi-file support**: process logs in CLI, alphabetical, or mtime order
- **Parallel mode**: scale up with `--parallel`, `--threads`, `--unordered`

---

## ✍️ Rhai Snippets

```rhai
// Filtering
level == "error" && user != "system"

// Enrichment
let sev = if status >= 500 { "crit" } else { "info" };

// Global metrics
track_count("errors");
track_max("max_duration", duration_ms);

// Window-based logic (with --window N)
let recent = window_values(window, "status");
let changed = window.len() > 1 && window[1].status != status;

// Regex, strings, JSON
let user = line.extract_re("user=(\w+)");
let ip = line.extract_ip();
let data = parse_json(line);
```

Built-in objects:
- `line` – raw line
- `event` – parsed fields
- `meta.linenum` – line number
- `tracked` – global state
- `window` – recent events (if `--window` is used)

---

## 🏗️ Common Patterns

```bash
# Filter and extract fields
kelora -f jsonl logs.jsonl --filter 'status >= 500' --keys ts,level,msg

# Format output
kelora -f jsonl app.log -F csv --keys user,status,duration_ms
kelora -f jsonl app.log -F logfmt --core --brief

# Use aliases (from config)
kelora -a errors logs.jsonl
```

---

## 📦 Install

```bash
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```

---

## 📚 Help & Options

```bash
kelora --help         # Show CLI help
kelora --help-time    # Show timestamp format reference
kelora --show-config  # Show config file and aliases
```

### Example flags:

| Flag           | Purpose                                      |
| -------------- | -------------------------------------------- |
| `-f`           | Input format (`jsonl`, `line`, `syslog`, …) |
| `-F`           | Output format (`jsonl`, `csv`, `logfmt`, …) |
| `--filter`     | Rhai expression to include events            |
| `--exec`       | Rhai script to transform events              |
| `--window N`   | Enable N+1 sliding event window              |
| `--summary`    | Show tracked key/value table                 |
| `--stats`      | Show line counts and performance stats       |
| `--on-error`   | How to handle bad lines (`print`, `skip`, …) |
| `--ts-format`  | Custom timestamp format (chrono syntax)     |
| `--input-tz`   | Timezone for naive timestamps (default: UTC) |
| `--pretty-ts`  | Format specific fields as RFC3339 timestamps |
| `-z`           | Format all timestamps as local RFC3339       |
| `-Z`           | Format all timestamps as UTC RFC3339         |
| `--since`      | Filter events after timestamp               |
| `--until`      | Filter events before timestamp              |

## 🌍 Timezone Handling

Kelora provides separate controls for parsing input timestamps and formatting output timestamps:

### Input Timezone (Parsing)
```bash
# Parse naive timestamps as UTC (default)
kelora -f jsonl app.log --input-tz UTC

# Parse naive timestamps as local system time
kelora -f jsonl app.log --input-tz local

# Parse naive timestamps as specific timezone
kelora -f jsonl app.log --input-tz Europe/Berlin
```

### Output Timestamp Formatting (Display Only)
```bash
# Format all known timestamp fields as local RFC3339
kelora -f jsonl app.log -z

# Format all known timestamp fields as UTC RFC3339
kelora -f jsonl app.log -Z

# Format specific fields as RFC3339 timestamps (local time)
kelora -f jsonl app.log --pretty-ts created_at,updated_at

# Combine: parse as Berlin time, display as UTC
kelora -f jsonl app.log --input-tz Europe/Berlin -Z
```

**Important**: Timestamp formatting only affects the default output format for human-readable display. It does not modify event data or structured outputs (JSON, CSV, etc.).

## 📖 Configuration

Kelora supports configuration files for setting defaults and aliases.

### Configuration File Locations

1. `$XDG_CONFIG_HOME/kelora/config.ini` (Unix)
2. `~/.config/kelora/config.ini` (Unix fallback)
3. `~/.kelorarc` (legacy compatibility)
4. `%APPDATA%\kelora\config.ini` (Windows)
5. `%USERPROFILE%\.kelorarc` (Windows legacy)

### Configuration Example

```ini
[defaults]
input-format = jsonl
output-format = jsonl
on-error = skip
parallel = true
stats = true

[aliases]
errors = --filter 'level == "error"' --stats
json-errors = --format jsonl --filter 'level == "error"' --output-format jsonl
slow-requests = --filter 'response_time.to_int() > 1000' --keys timestamp,method,path,response_time
```

### Configuration Commands

```bash
# Show current configuration and search paths
kelora --show-config

# Use an alias from configuration
kelora -a errors /path/to/logs

# Ignore configuration file (use CLI defaults only)
kelora --ignore-config --filter "level == 'error'"
```

## 🔧 Input & Output Formats

### Input Formats (`-f FORMAT`)

| Format | Description | Example |
|--------|-------------|---------|
| `jsonl` | JSON lines (default) | `{"level": "info", "msg": "..."}` |
| `line` | Raw text lines | Any text file |
| `syslog` | Syslog format | `/var/log/messages` |
| `logfmt` | Key=value pairs | `level=info method=GET status=200` |
| `csv` | Comma-separated values | `date,user,action` |
| `tsv` | Tab-separated values | `date	user	action` |
| `cef` | Common Event Format | Security logs |
| `cols` | Whitespace-separated columns | Like AWK processing |

### Output Formats (`-F FORMAT`)

| Format | Description | Use Case |
|--------|-------------|----------|
| `default` | Logfmt-style output | Normal log analysis |
| `jsonl` | JSON lines | Structured data processing |
| `logfmt` | Strict logfmt | Standardized output |
| `csv` | Comma-separated values | Data analysis |
| `tsv` | Tab-separated values | Data analysis |
| `hide` | Hide events, show side effects | Analytics with debug |
| `null` | Suppress all output | Performance testing |

## 🪟 Window Analysis

The `--window N` option enables sliding window analysis for pattern detection:

```bash
# Detect consecutive errors
kelora -f jsonl app.log --window 2 --filter 'level == "error"' \
  --exec 'if window.len() > 1 && window[1].level == "error" { print("Consecutive errors!") }'

# Monitor response time trends
kelora -f jsonl api.log --window 5 \
  --exec 'let times = window_numbers(window, "response_time"); 
          if times.len() >= 3 { 
            let avg = times.iter().sum() / times.len(); 
            print("Avg response time: " + avg + "ms") 
          }'
```

### Window Functions

- `window_values(window, field)` - Extract field values from all window events
- `window_numbers(window, field)` - Extract numeric values from window events
- `window[0]` - Current event
- `window[1]` - Previous event
- `window[N]` - Event N steps back

## 🔍 Built-in Functions

### String Processing
- `extract_re(pattern)` - Extract with regex
- `extract_ip()` - Extract IP addresses
- `extract_url()` - Extract URLs
- `mask_ip(octets)` - Mask IP addresses
- `is_private_ip()` - Check if IP is private

### Data Parsing
- `parse_json(string)` - Parse JSON
- `parse_kv(string)` - Parse key=value pairs
- `parse_timestamp(string)` - Parse timestamps (auto-detect format)
- `get_path(data, "path.to.field")` - Extract nested values

### Array Operations
- `sorted(array)` - Sort array (returns new array)
- `reversed(array)` - Reverse array (returns new array)
- `array.sorted_by(field)` - Sort objects by field

### DateTime & Duration
- `parse_timestamp(s)` - Parse with auto-detection
- `parse_duration(s)` - Parse durations like "1h 30m"
- `now_utc()` - Current UTC time
- `duration_from_seconds(n)` - Create duration

### Tracking & Metrics
- `track_count(key)` - Count occurrences
- `track_max(key, value)` - Track maximum value
- `track_min(key, value)` - Track minimum value
- `track_sum(key, value)` - Sum values
- `track_unique(key, value)` - Count unique values

## 📊 Performance & Parallelization

```bash
# Parallel processing for large files
kelora -f jsonl large.log --parallel --threads 8 --filter 'level == "error"'

# Unordered processing for maximum speed
kelora -f jsonl huge.log --parallel --unordered --filter 'status >= 500'

# Batch processing configuration
kelora -f jsonl stream.log --batch-size 5000 --batch-timeout 100ms
```

### Performance Tips

- Use `--parallel` for large files
- Add `--unordered` for maximum throughput
- Use `-F null` for performance testing
- Adjust `--batch-size` based on memory constraints
- Use `--on-error skip` for production pipelines

## 🔄 Multi-line Processing

For logs with multi-line events:

```bash
# Stack traces (indented continuation)
kelora -f line app.log --multiline indent --filter 'line.contains("Exception")'

# SQL statements (semicolon-terminated)
kelora -f line sql.log --multiline end:';$'

# Timestamp-based grouping
kelora -f line app.log --multiline timestamp --filter 'line.contains("ERROR")'
```

## 📝 Common Examples

### Error Analysis
```bash
# Count errors by hour
kelora -f jsonl app.log --filter 'level == "error"' \
  --exec 'let dt = parse_timestamp(timestamp); track_count("hour_" + dt.format("%H"))' \
  --summary

# Find slow requests with context
kelora -f jsonl api.log --window 2 --filter 'response_time.to_int() > 1000' \
  --exec 'let prev = window.len() > 1 ? window[1].response_time : "none"; 
          print("Slow request, prev: " + prev + "ms")'
```

### Log Transformation
```bash
# Convert syslog to JSON
kelora -f syslog /var/log/messages -F jsonl --keys timestamp,level,message

# Extract and mask IP addresses
kelora -f line access.log \
  --exec 'let ip = line.extract_ip(); let masked = ip.mask_ip(2)' \
  --keys timestamp,masked_ip,request
```

### Real-time Monitoring
```bash
# Monitor Kubernetes pods
kubectl logs -f deployment/app | kelora -f jsonl --levels warn,error

# System log monitoring
tail -f /var/log/syslog | kelora -f syslog --filter 'level <= 3' --stats
```

---

## 🕵️ Not a Replacement For

| Task                | Use Instead                          |
| ------------------- | ------------------------------------- |
| Log browsing        | `lnav`                                |
| Full-text search    | `ripgrep`                             |
| Dashboards          | `Grafana`, `Kibana`                   |
| Log ingestion       | `fluentbit`, `vector`, `loki`         |
| JSON-only pipelines | `jq`                                  |
| Regex pipelines     | `angle-grinder`                       |

---

## 📄 License

MIT — see [LICENSE](LICENSE)
