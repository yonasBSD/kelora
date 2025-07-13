# Kelora

Scriptable log processor for the command line. Treats logs as structured events and lets you filter, transform, and analyze them using [Rhai](https://rhai.rs) scripts.

## Quick Start

```bash
# Filter JSON logs with enrichment
kelora -f jsonl app.log --filter 'status >= 500' --exec 'let severity = "critical"'

# Pattern detection with sliding windows  
kelora -f jsonl auth.log --window 3 --filter 'event == "login_failed"' \
  --exec 'if window_values(window, "user").len() >= 2 { print("Brute force detected") }'

# Real-time monitoring with parallelization
kubectl logs -f app | kelora -f jsonl --parallel --levels warn,error

# Time-range analysis with metrics
kelora -f syslog /var/log/messages --since "1 hour ago" \
  --exec 'track_count(facility); track_unique("hosts", host)' --summary
```

## Install

```bash
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```

## Core Features

**Formats**: JSON, syslog, logfmt, CEF, CSV, TSV, raw lines with `.gz` support  
**Processing**: Sliding windows, parallel/batch processing, multiline events  
**Scripting**: Embedded Rhai with 40+ built-in functions for parsing, metrics, time handling  
**Configuration**: Aliases and defaults via config files  
**Error handling**: Quarantine, skip, or abort on malformed input

## Rhai Examples

```rhai
// Filter and enrich
level == "error" && response_time.to_int() > 1000
let severity = if status >= 500 { "critical" } else { "warning" }

// Pattern detection
let ips = window_values(window, "ip"); ips.contains("192.168.1.100")

// Extract and transform  
let user = line.extract_re("user=(\w+)"); 
let masked_ip = event.ip.mask_ip(2)

// Track metrics
track_count("requests"); track_max("peak_latency", duration_ms)
```

## Common Use Cases

```bash
# Error analysis with time grouping
kelora -f jsonl api.log --filter 'level == "error"' \
  --exec 'track_count("errors_" + parse_timestamp(ts).format("%H"))' --summary

# Convert formats with field selection  
kelora -f syslog /var/log/messages -F csv --keys timestamp,host,message

# Quarantine analysis (handle malformed data)
kelora -f jsonl mixed.log --filter 'meta.contains("parse_error")' \
  --exec 'track_unique("error_types", meta.parse_error)' --summary

# Performance testing with output suppression
kelora -f jsonl huge.log --parallel -F null --filter 'status != 200'

# Multiline processing (stack traces)  
kelora -f line app.log --multiline indent --filter 'line.contains("Exception")'

# Configuration aliases
kelora -a slow-requests access.log  # From ~/.config/kelora/config.ini
```

## Key Options

| Flag | Purpose | Example |
|------|---------|---------|
| `-f FORMAT` | Input format | `jsonl`, `syslog`, `csv`, `line` |
| `-F FORMAT` | Output format | `jsonl`, `csv`, `logfmt`, `null` |
| `--filter EXPR` | Include matching events | `level == "error"` |
| `--exec SCRIPT` | Transform events | `let type = "slow"` |
| `--window N` | Sliding window size | `--window 5` |
| `--parallel` | Parallel processing | Higher throughput |
| `--since/--until` | Time filtering | `"2024-01-01"`, `"1 hour ago"` |
| `--keys FIELDS` | Select output fields | `timestamp,level,msg` |
| `--on-error MODE` | Error handling | `quarantine`, `skip`, `abort` |

See `kelora --help` for the complete reference.

## Advanced Features

**Window Analysis**: Detect patterns across event sequences with `--window N`
```bash
kelora -f jsonl app.log --window 2 --exec 'if window[1].status != status { print("Status changed") }'
```

**Timezone Handling**: Parse input in one timezone, display in another  
```bash
kelora -f jsonl app.log --input-tz Europe/Berlin -Z  # Parse as Berlin, display as UTC
```

**Built-in Functions**: 40+ functions for string processing, time parsing, metrics tracking
- String: `extract_re()`, `extract_ip()`, `mask_ip()`  
- Time: `parse_timestamp()`, `parse_duration()`, `now_utc()`
- Data: `parse_json()`, `parse_kv()`, `get_path()`
- Metrics: `track_count()`, `track_max()`, `track_unique()`

**Error Strategies**:
- `quarantine` (default): Process broken lines as events accessible via `meta`
- `skip`: Discard malformed input
- `abort`: Stop on first error

## Help & Documentation

```bash
kelora --help           # CLI reference
kelora --help-time      # Timestamp format guide  
kelora --show-config    # Current configuration
```

## Not a Replacement For

* Log browsing: Use `lnav` 
* Full-text search: Use `ripgrep` 
* Dashboards: Use Grafana/Kibana 
* JSON pipelines: Use `jq`

---

**License**: [MIT](LICENSE)
