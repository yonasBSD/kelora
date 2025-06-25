# Kelora

A command-line log analysis tool with embedded Rhai scripting for flexible log processing and transformation.

## Features

- **Rhai Scripting**: Filter, transform, and analyze logs with embedded Rhai expressions
- **Parallel Processing**: Batch processing with configurable worker threads for large datasets
- **Global State Tracking**: Track counters, min/max values, and statistics across all log entries
- **Multiple I/O Formats**: JSON input/output, text (logfmt) output
- **Error Strategies**: Skip, fail-fast, emit-errors, or use default values for malformed data
- **Built in Rust**: Memory-efficient processing with good error handling

## Installation

```bash
git clone https://github.com/dloss/kelora.git
cd kelora
cargo build --release
```

## Usage

### Basic Processing
```bash
# Process JSON logs
echo '{"user":"alice","status":404}' | kelora -f json

# Filter with Rhai expressions  
kelora -f json --filter 'status >= 400' logs.jsonl

# Transform data
kelora -f json --eval 'let alert = if status >= 500 { "critical" } else { "warning" };' logs.jsonl

# Text output format
kelora -f json -F text logs.jsonl
```

### Advanced Analysis
```bash
# Multi-stage processing with global tracking
kelora -f json \
  --begin 'print("Starting analysis...")' \
  --filter 'status >= 400' \
  --eval 'track_count(tracked, "errors"); track_max(tracked, "max_response", response_time)' \
  --end 'print(`Found ${tracked["errors"]} errors, max response: ${tracked["max_response"]}ms`)' \
  logs.jsonl

# Parallel processing for larger datasets
kelora --parallel --threads 8 --batch-size 2000 -f json --filter 'level == "error"' large.jsonl
```

### Pipeline Integration
```bash
# Real-time log monitoring
kubectl logs -f my-app | kelora -f json --filter 'level == "error"' --eval 'print("ERROR: " + message);'

# Convert and analyze
cat access.log | kelora -f json --keys user,status,response_time -F text
```

## CLI Reference

### Core Arguments
- `-f, --format json|line|csv|apache` - Input format (default: json)
- `-F, --output-format json|text|csv` - Output format (default: json)
- `--keys field1,field2` - Output only specified fields

### Rhai Stages
- `--begin 'expression'` - Run once before processing
- `--filter 'expression'` - Boolean filter (can repeat)
- `--eval 'expression'` - Transform/process (can repeat)  
- `--end 'expression'` - Run once after processing

### Error Handling
- `--on-error skip|fail-fast|emit-errors|default-value` (default: emit-errors)

### Parallel Processing
- `--parallel` - Enable parallel processing
- `--threads N` - Worker thread count (default: CPU cores)
- `--batch-size N` - Lines per batch (default: 1000)
- `--no-preserve-order` - Faster unordered output

## Rhai Scripting

### Built-in Variables
- `line` - Original log line text
- `event` - Field map for invalid identifiers
- `meta.linenum` - Line number
- `tracked` - Global state map

### Available Functions

#### String Analysis
```rhai
text.contains("pattern")     // String search
text.to_int()               // Parse integer
text.to_float()             // Parse float
```

#### Log Analysis  
```rhai
status.status_class()       // "2xx", "4xx", "5xx", etc.
```

#### Global Tracking
```rhai
track_count(tracked, "errors")              // Increment counter
track_min(tracked, "min_time", response)    // Track minimum
track_max(tracked, "max_time", response)    // Track maximum
```

### Variable Declaration
Use `let` for new variables:
```rhai
let alert_level = if status >= 500 { "critical" } else { "warning" };
```

## Processing Modes

**Sequential (default)**: Real-time streaming output, ideal for monitoring
```bash
kelora --filter 'status >= 400'
```

**Parallel**: Batch processing for larger datasets
```bash  
kelora --parallel --filter 'status >= 400'
```

## Examples

### Error Analysis
```bash
kelora -f json \
  --filter 'status >= 400' \
  --eval 'track_count(tracked, status.status_class())' \
  --end 'print(`4xx: ${tracked["4xx"] ?? 0}, 5xx: ${tracked["5xx"] ?? 0}`)' \
  access.log
```

### Performance Monitoring
```bash
kelora -f json \
  --eval 'track_min(tracked, "min", response_time); track_max(tracked, "max", response_time)' \
  --end 'print(`Response time range: ${tracked["min"]}-${tracked["max"]}ms`)' \
  api.log
```

### Data Enrichment
```bash
kelora -f json \
  --eval 'let risk_score = if ip.contains("10.") { 1 } else { 5 }; let processed_at = "2024-01-01";' \
  -F json logs.jsonl
```

## Development

```bash
# Build and test
cargo build --release
cargo test
cargo clippy

# Performance test
time ./target/release/kelora -f json large.jsonl --filter "status >= 400" --on-error skip > /dev/null
```

## License

MIT License - see [LICENSE](LICENSE) file.