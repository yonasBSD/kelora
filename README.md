# Kelora

Scriptable log processor for the command line. Treats logs as structured events and lets you filter, transform, and analyze them using [Rhai](https://rhai.rs) scripts.

## Quick Start

Scripts use `e` to access the current log event - `e.status`, `e.level`, etc. are the actual fields from your logs.

```bash
# Filter JSON logs with enrichment
kelora -f jsonl app.log --filter 'e.status >= 500' --exec 'e.severity = "critical"'

# Pattern detection with sliding windows  
kelora -f jsonl auth.log --window 3 --filter 'e.event == "login_failed"' \
  --exec 'if window_values(window, "user").len() >= 2 { print("Brute force detected") }'

# Real-time monitoring with parallelization
kubectl logs -f app | kelora -f jsonl --parallel --levels warn,error

# Time-range analysis with metrics
kelora -f syslog /var/log/messages --since "1 hour ago" \
  --exec 'track_count(e.facility); track_unique("hosts", e.host)' --stats

# Docker container monitoring
docker compose logs --timestamps | kelora -f docker --filter 'e.msg.contains("error")' \
  --exec 'e.service = e.src ?? "unknown"; print(`${e.service}: ${e.msg}`)'
```

## Install

```bash
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```

## Core Features

**Formats**: JSON, syslog, logfmt, CEF, CSV, TSV, Docker logs, raw lines with `.gz` support  
**Processing**: Sliding windows, parallel/batch processing, multiline events  
**Scripting**: Embedded Rhai with 40+ built-in functions for parsing, metrics, time handling  
**Configuration**: Aliases and defaults via config files  
**Error handling**: Resilient mode with robust error recovery, strict mode for fail-fast behavior

## Rhai Examples

```rhai
// Filter and enrich
e.level == "error" && e.response_time.to_int() > 1000
e.severity = if e.status >= 500 { "critical" } else { "warning" }

// Pattern detection
let ips = window_values(window, "ip"); ips.contains("192.168.1.100")

// Extract and transform  
let user = e.line.extract_re("user=(\w+)"); 
e.masked_ip = e.ip.mask_ip(2)

// Track metrics and use safety functions
track_count("requests"); track_max("peak_latency", get_path(e, "duration_ms", 0))
```

## Working with Events

The `e` variable represents the current event. Access fields directly (`e.level`) or use bracket notation for invalid identifiers (`e["content-type"]`). Add fields by assignment (`e.severity = "critical"`).

```rhai
// Field access and modification
e.level == "error"                        // Direct access
e["user-agent"] = "kelora/1.0"           // Invalid identifiers need brackets
e.processed = now_utc()                   // Add new fields

// Field and event removal with unit ()
e.password = ()                           // Remove individual fields
e = ()                                    // Remove entire event (clears all fields)

// Method vs function style (both work, methods chain better)
e.ip.mask_ip(2)                          // Method style
mask_ip(e.ip, 2)                         // Function style (avoids conflicts)

// Safety functions with fallbacks
get_path(e, "user.profile.id", "unknown") // Safe nested access
to_number(e.port, 80)                     // Safe conversion with default
```

## Common Use Cases

```bash
# Error analysis with time grouping
kelora -f jsonl api.log --filter 'e.level == "error"' \
  --exec 'track_count("errors_" + parse_timestamp(e.ts).format("%H"))' --stats

# Convert formats with field selection  
kelora -f syslog /var/log/messages -F csv --keys timestamp,host,message

# Docker container log analysis
docker compose logs --timestamps | kelora -f docker \
  --filter 'e.src == "web" && e.msg.contains("500")' --exec 'print(`Error in ${e.src}: ${e.msg}`)'

# Strict mode for validation (fail-fast on errors)
kelora -f jsonl mixed.log --strict --filter 'e.level == "error"'

# Performance testing with output suppression
kelora -f jsonl huge.log --parallel -F null --filter 'e.status != 200'

# Multiline processing (stack traces)  
kelora -f line app.log --multiline indent --filter 'e.line.contains("Exception")'

# Configuration aliases
kelora -a slow-requests access.log  # From ~/.config/kelora/config.ini
```

## Key Options

| Flag | Purpose | Example |
|------|---------|---------|
| `-f FORMAT` | Input format | `jsonl`, `syslog`, `csv`, `docker`, `line` |
| `-F FORMAT` | Output format | `jsonl`, `csv`, `logfmt`, `null` |
| `--filter EXPR` | Include matching events | `e.level == "error"` |
| `--exec SCRIPT` | Transform events | `e.type = "slow"` |
| `--window N` | Sliding window size | `--window 5` |
| `--parallel` | Parallel processing | Higher throughput |
| `--since/--until` | Time filtering | `"2024-01-01"`, `"1 hour ago"` |
| `--keys FIELDS` | Select output fields | `timestamp,level,msg` |
| `--strict` | Fail-fast error handling | Abort on first error |
| `--verbose` | Detailed error information | Show error details |

See `kelora --help` for the complete reference.

## Advanced Features

**Window Analysis**: Detect patterns across event sequences with `--window N`
```bash
kelora -f jsonl app.log --window 2 --exec 'if window[1].status != e.status { print("Status changed") }'
```

**Timezone Handling**: Parse input in one timezone, display in another  
```bash
kelora -f jsonl app.log --input-tz Europe/Berlin -Z  # Parse as Berlin, display as UTC
```

**Built-in Functions**: 40+ functions for string processing, time parsing, metrics tracking
- String: `extract_re()`, `extract_ip()`, `mask_ip()`  
- Time: `parse_timestamp()`, `parse_duration()`, `now_utc()`
- Data: `parse_json()`, `parse_kv()`, `get_path()`, `has_path()`, `path_equals()`
- Safety: `to_number()`, `to_bool()` for robust type conversion
- Metrics: `track_count()`, `track_max()`, `track_unique()`

**Error Handling Modes**:
- **Resilient** (default): Skip errors, continue processing, show summary at end
- **Strict** (`--strict`): Fail-fast on any error with immediate error display
- Context-specific: Parsing errors skip lines, filter errors skip events, exec errors roll back changes

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
