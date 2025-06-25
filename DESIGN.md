# Kelora Design Document

## Overview

**Kelora** is a command-line log analysis tool that embeds the Rhai scripting language for flexible log processing. Users write Rhai expressions to filter, transform, and analyze log data in real-time.

## Core Concept

Process log lines through a pipeline of Rhai expressions:
```
Input → Parse → Begin → Filter → Eval → End → Output
```

Each log line becomes available as variables in Rhai scripts, enabling powerful one-liner transformations.

## Command Line Interface

```bash
# Basic filtering
kelora --filter 'status >= 400'

# Multi-stage processing
kelora --begin 'print("Starting analysis...")' \
       --filter 'status >= 400' \
       --eval 'alert_level = "high"; track_count(tracked, "errors")' \
       --end 'print(`Found ${tracked["errors"]} errors`)'

# Input/output formats
kelora -f apache --filter 'ip.is_private_ip()' -F json
kelora -f csv --eval 'total = price * quantity'
```

### CLI Arguments

| Flag | Purpose | Example |
|------|---------|---------|
| `-f, --format` | Input format (line, csv, json, apache) | `-f apache` |
| `-F, --output-format` | Output format (json, csv, text) | `-F json` |
| `--begin` | Run once before processing | `--begin 'print("Starting...")'` |
| `--filter` | Boolean filter (can repeat) | `--filter 'status >= 400'` |
| `--eval` | Transform/process (can repeat) | `--eval 'alert = "high"'` |
| `--end` | Run once after processing | `--end 'print("Done")'` |
| `--no-inject-fields` | Disable field auto-injection | Access via `event["field"]` only |
| `--inject-prefix` | Prefix for injected variables | `--inject-prefix "log_"` |
| `--on-error` | Error handling (skip, fail-fast, emit-errors, default-value) | `--on-error skip` |
| `--keys` | Output only specific fields | `--keys "ip,status,timestamp"` |

## Processing Modes

Kelora supports different processing modes that affect performance and output behavior:

### Default Mode (Recommended)
```bash
kelora --filter 'status >= 400'  # Default behavior
```
- **Parallel processing** with **ordered output**
- Events processed concurrently but output maintains input order
- `tracked` state only available in `--end` stage
- Best balance of speed and correctness
- Use for most log analysis tasks

## Data Model

### Event Structure
Each log line becomes an Event with:
- **Fields**: Key-value pairs (injected as Rhai variables)
- **Metadata**: Line number, filename, etc. (available as `meta.linenum`, `meta.filename`)
- **Original line**: Raw text (available as `line` variable)

### Variable Injection
Fields are automatically injected as Rhai variables based on input format:
```bash
# JSON input: {"user": "alice", "status": 404}
kelora -f json --filter 'user == "alice" && status >= 400'

# Apache logs: predefined fields (ip, method, path, status, etc.)
kelora -f apache --filter 'status >= 400 && ip.is_private_ip()'

# CSV with headers: column names become variables
kelora -f csv --filter 'price.to_float() > 100'

# Raw lines: only "line" field available
kelora -f line --filter 'line.contains("ERROR")'

# Invalid identifiers use event map
kelora --filter 'event["user-name"] == "admin"'
```

**Important**: Fields like `status`, `user`, `ip` are only available if the input format provides them. Use `event["field"]` to access any field or when field names have invalid characters.

## Rhai Scripting Features

### Built-in Functions

#### Column Parsing
```rhai
line.cols(0)              // First column
line.cols(-1)             // Last column
line.cols("1:3")          // Columns 1-2 (slice)
line.cols("2:")           // From column 2 to end
line.cols(0, 2, 4)        // Multiple columns
```

#### String Methods
```rhai
text.matches("ERROR|WARN")        // Regex match
text.replace("\\d+", "XXX")       // Regex replace  
text.extract("https?://([^/]+)")  // Extract capture group
text.extract_pattern("email")     // Built-in patterns
text.to_int()                     // Parse integer
text.to_float()                   // Parse float
text.to_ts()                      // Parse timestamp
```

#### Log Analysis
```rhai
status.status_class()             // "4xx", "5xx", etc.
level.normalize_level()           // "DEBUG", "INFO", etc.
ip.is_private_ip()               // Boolean
url.domain()                     // Extract domain
user_agent.is_bot()              // Detect bots
```

#### Global Tracking
```rhai
track_count(tracked, "errors")           // Increment counter
track_min(tracked, "response_time", ms)  // Track minimum
track_max(tracked, "response_time", ms)  // Track maximum
track_unique(tracked, "ips", ip)         // Collect unique values
track_bucket(tracked, "status", code)    // Count by value

// Access in --end stage
tracked["errors"]                        // Read-only access
```

### String Interpolation
```rhai
print(`User ${user} failed with ${status}`)
alert_msg = `Error at ${meta.linenum}: ${message}`
```

### Pattern Examples
```rhai
// Apache log processing
ip = line.cols(0)
status = line.cols(8).to_int()
if status >= 400 { track_count(tracked, "errors") }

// JSON log enhancement  
severity = if level == "ERROR" { "high" } else { "low" }
response_time_ms = response_time * 1000

// CSV analysis
total_score = math_score + english_score
grade = if total_score >= 90 { "A" } else { "B" }
```

## Architecture

### Core Components

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Input Parser  │──→ │  Rhai Engine     │──→ │ Output Formatter│
│                 │    │                  │    │                 │
│ - Line          │    │ - Field Injection│    │ - JSON          │
│ - CSV           │    │ - AST Compilation│    │ - CSV           │
│ - JSON          │    │ - Stage Execution│    │ - Text          │
│ - Apache        │    │ - Error Handling │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

### Processing Pipeline

1. **Parse**: Convert input line to Event structure
2. **Inject**: Make fields available as Rhai variables
3. **Execute Stages**: Run begin → filters → evals → end
4. **Format**: Convert result to output format

### Input Formats

| Format | Description | Available Fields | Example Fields |
|--------|-------------|------------------|----------------|
| `line` | Raw text | `line` only | `line` |
| `csv` | Comma-separated | Column headers + `line` | `name`, `age`, `status`, `line` |
| `json` | JSON objects | All JSON keys + `line` | `user`, `timestamp`, `level`, `line` |
| `apache` | Apache/Nginx logs | Parsed fields + `line` | `ip`, `method`, `path`, `status`, `bytes`, `line` |

**Field Availability**: Only fields provided by the input format can be used as direct variables. Always available: `line` (raw text), `event` (field map), `meta` (metadata), `tracked` (global state).

### Output Formats

| Format | Description |
|--------|-------------|
| `json` | JSON objects |
| `csv` | Comma-separated values |
| `text` | Original lines or key=value pairs |

## Implementation Architecture

### Key Design Decisions

1. **Rhai Engine Reuse**: Compile expressions once, evaluate per line
2. **Variable Pre-declaration**: Optimize Rhai performance with known field names
3. **Error Strategies**: Configurable handling (skip, fail-fast, emit, default)
4. **Stage Ordering**: Filters and evals can be interleaved as specified
5. **Memory Management**: Reuse Scope objects, avoid per-line allocations

### Critical Implementation Details

#### Engine Setup
- Register custom functions for log analysis
- Pre-declare common variables (`line`, `event`, `meta`, `tracked`)
- Set resource limits for safety

#### Stage Execution
- **Begin**: Execute once before any events
- **Filter**: Boolean expressions, skip event if false
- **Eval**: Modify variables, side-channel output via `print()`
- **End**: Execute once after all events

#### Error Handling
Four strategies via `--on-error`:
- `skip`: Continue processing, ignore failed lines
- `fail-fast`: Stop on first error
- `emit-errors`: Print errors to stderr, continue
- `default-value`: Use empty/default values for failed lines

#### Global State
- Track statistics across all events
- Write-only during processing (`track_*()` functions)
- Read-only access in end stage (`tracked["key"]`)

## Example Use Cases

### Error Analysis
```bash
# Find and categorize errors
kelora -f apache \
  --filter 'status >= 400' \
  --eval 'track_bucket(tracked, "error_type", status.status_class())' \
  --end 'print(`4xx errors: ${tracked["error_type#4xx"] ?? 0}
5xx errors: ${tracked["error_type#5xx"] ?? 0}`)'
```

### Performance Monitoring
```bash
# Track response times
kelora -f json \
  --eval 'track_min(tracked, "min_time", response_time)
          track_max(tracked, "max_time", response_time)' \
  --end 'print(`Response time range: ${tracked["min_time"]} - ${tracked["max_time"]}ms`)'
```

### Data Transformation
```bash
# Enrich and transform
kelora -f csv \
  --eval 'risk_score = calculate_risk(ip, user_agent)
          alert_level = if risk_score > 8.0 { "high" } else { "normal" }
          debug_info = null' \
  -F json
```

## Development Notes

### Technology Stack
- **Rust**: Core implementation
- **Rhai**: Embedded scripting engine
- **clap**: CLI argument parsing
- **serde**: JSON serialization
- **regex**: Pattern matching
- **chrono**: Timestamp handling

### Testing Strategy
- Unit tests for each component (parsers, formatters, engine)
- Integration tests for CLI workflows
- Error handling verification
- Performance benchmarks

This design balances simplicity with power, providing a clean CLI interface while enabling complex log analysis through Rhai scripting.