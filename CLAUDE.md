# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Kelora is a command-line log analysis tool written in Rust that uses the Rhai scripting engine for flexible log processing. It processes structured logs (JSON, CSV, etc.) and allows users to filter, transform, and analyze log data using embedded Rhai scripts.

## Key Commands

### Build and Test
```bash
# Build optimized release binary
cargo build --release

# Run performance tests
time ./target/release/kelora -f json <logfile> --filter "expression" --on-error skip > /dev/null

# Run benchmark suite to detect performance regressions
make bench-quick              # Quick benchmarks (10k dataset)
make bench                    # Full benchmark suite (10k + 50k datasets)
make bench-baseline           # Update performance baseline

# Run lint and type checking
cargo clippy
cargo fmt --check

# Run tests
cargo test               # Unit and integration tests
cargo test --lib         # Unit tests only
cargo test --test integration_tests  # Integration tests only
make test-full          # Comprehensive test suite
```

### Example Usage
```bash
# Filter high response times from JSON logs
./target/release/kelora -f jsonl logs.jsonl --filter "response_time.sub_string(0,2).to_int() > 98"

# Parse and filter syslog by severity
./target/release/kelora -f syslog /var/log/syslog --filter 'severity <= 3'

# Count status codes and track metrics
./target/release/kelora -f jsonl access.log --exec "track_count(status_class(status))" --end "print(tracked)"

# Process any log file (default line format)
./target/release/kelora /var/log/syslog --filter 'line.matches("ERROR|WARN")'

# Process gzip compressed log files (automatic decompression)
./target/release/kelora -f jsonl logs.jsonl.gz --filter "status >= 400"

# Process multiple files with different ordering
./target/release/kelora -f jsonl file1.jsonl file2.jsonl file3.jsonl  # CLI order (default)
./target/release/kelora -f jsonl --file-order name *.jsonl            # Alphabetical order
./target/release/kelora -f jsonl --file-order mtime *.jsonl           # Modification time order

# Handle log rotation (mixed compressed/uncompressed, chronological order)
# Matches: app.log app.log.1 app.log.2.gz app.log.3.gz - .gz files auto-decompressed
./target/release/kelora -f jsonl --file-order mtime app.log*
```

## Architecture

### Core Components
- **`src/main.rs`** - CLI interface, argument parsing, orchestrates sequential vs parallel processing
- **`src/pipeline.rs`** - Modular trait-based pipeline architecture with pluggable stages (NEW)
- **`src/engine.rs`** - Rhai scripting engine with AST compilation, scope templates, and custom functions
- **`src/event.rs`** - Log event data structure with smart field extraction and metadata tracking
- **`src/parsers.rs`** - Input parsers implementing EventParser trait (JSON, line, logfmt, syslog)
- **`src/formatters.rs`** - Output formatters implementing pipeline::Formatter trait (JSON and logfmt text)
- **`src/parallel.rs`** - High-throughput parallel processing using unified pipeline architecture

### Performance Design
Kelora follows a "compile once, evaluate repeatedly" model:
1. **Engine Creation** - Built once at startup with all custom functions registered
2. **AST Compilation** - All user expressions (--filter, --exec, etc.) compiled to ASTs at startup
3. **Scope Templates** - Single scope template cloned and reused for each log line
4. **Variable Injection** - Log fields auto-injected as Rhai variables with fallback to event map
5. **Parallel Architecture** - Producer-consumer model with batching and thread-local state tracking

### Data Flow
**Unified Pipeline Architecture**: Both sequential and parallel modes use the same trait-based pipeline:
- **Sequential**: Input → Pipeline.process_line() → Immediate Output  
- **Parallel**: Input → Batching → Worker Pipelines → State Merging → Output

### Processing Pipeline (Trait-Based)
The new modular pipeline processes each line through configurable stages:
1. **LineFilter** (optional): Skip lines before parsing
2. **Chunker**: Handle multi-line records  
3. **EventParser**: Convert line to Event structure
4. **ScriptStages**: Run filters → execs in sequence
5. **EventLimiter** (optional): Implement --take N
6. **Formatter**: Convert event to output format

### Processing Modes
**Sequential Mode (default)**: Real-time streaming output, perfect for monitoring
```bash
kelora --filter 'status >= 400'  # Live log analysis
kubectl logs -f my-app | kelora -f jsonl --filter 'level == "error"'
```

**Parallel Mode**: Batch processing for high-throughput analysis
```bash
kelora --parallel --filter 'status >= 400'  # Large dataset processing
```

## Development Guidelines

### Keeping Parallel and Sequential Modes in Sync

Kelora has both `run_parallel()` and `run_sequential()` functions that must remain synchronized. To prevent divergence:

#### Shared Abstractions (Implemented)
- **`execute_begin_stage()`** - Shared begin stage execution with error handling
- **`execute_end_stage()`** - Shared end stage execution with error handling  
- **Reader helpers** - `create_parallel_reader()`, `create_sequential_reader()` for consistent file handling
- **Common error patterns** - Both modes use identical error handling for pipeline creation and stage execution

#### Testing for Equivalence
- **`test_parallel_sequential_equivalence()`** - Integration test that verifies both modes produce identical results
- Run this test after any changes to processing logic: `cargo test test_parallel_sequential_equivalence`

#### Development Best Practices
1. **Use shared helpers**: Always use the shared helper functions instead of duplicating logic
2. **Update both modes**: When adding new features, ensure both processing modes support them
3. **Test equivalence**: Run the equivalence test to verify changes don't break synchronization
4. **Code reviews**: Check that PR changes maintain mode synchronization

#### Adding New Features
When adding features that affect log processing:
1. Implement the feature in the shared pipeline architecture first
2. Verify both `run_parallel()` and `run_sequential()` use the same pipeline logic
3. Add test cases to `test_parallel_sequential_equivalence()` if needed
4. Document any mode-specific behavior differences

### Performance Considerations
- Pre-compile all Rhai expressions to ASTs at startup
- Reuse scope templates instead of creating from scratch
- Pool frequently allocated data structures (Rhai Maps)
- Avoid string cloning in hot paths
- Use static methods to avoid borrow checker conflicts

### Code Style
- Follow standard Rust conventions
- Use descriptive variable names
- Add inline comments for complex logic
- Prefer `Result<T>` for error handling
- Use `anyhow` for error context

### 🔧 Rhai Optimization Alignment

Kelora is designed to leverage Rhai's built-in optimizations:

| Rhai Optimization | Kelora Usage |
|------------------|--------------|
| Pre-calculated variable offsets | Pre-declare common variables (line, event, meta, tracked) |
| AST compilation and reuse | Compile expressions once at startup |
| Cached function resolution | Use built-in and registered functions consistently |
| Contiguous variable storage | Reuse single scope, update variables in-place |

**Key Design Principle**: Trust Rhai's optimizations rather than implementing custom caching or offset management. Rhai is specifically designed for "compile once, evaluate repeatedly" scenarios.

### Testing
- Test with sample log files in `test_data/` directory or create in `/tmp/` for development
- Use benchmark datasets in `benchmarks/` directory for performance testing
- Verify both correctness and performance impact of changes
- Test error handling with all four `--on-error` strategies (skip, abort, print, stub)
- Use `make test-full` for comprehensive testing including manual test scenarios

### Git Guidelines
- **NEVER use `git add .`** - Always add files explicitly by name
- Use `git add src/main.rs src/parallel.rs Cargo.toml` etc.
- This prevents accidental inclusion of temporary files, editor backups, or unintended changes

## Testing and Performance

### Benchmark Commands
```bash
# Quick benchmarks using built-in test data
make bench-quick              # Uses benchmarks/bench_10k.jsonl

# Full benchmark suite
make bench                    # Uses 10k and 50k datasets

# Performance testing with manual datasets
time ./target/release/kelora -f json large_file.jsonl \
  --filter "status >= 400" --on-error skip > /dev/null
```

### Test Data Location
- **Built-in test data**: `test_data/sample.jsonl`, `test_data/sample.logfmt`
- **Benchmark datasets**: `benchmarks/bench_10k.jsonl`, `benchmarks/bench_50k.jsonl`, etc.
- **Create test samples**: `head -n 1000 large_file.jsonl > /tmp/test_sample.jsonl`

## Rhai Scripting Reference

### Variable Injection and Access
Fields are automatically injected as Rhai variables based on input format:
```bash
# JSON input: {"user": "alice", "status": 404}
kelora -f jsonl --filter 'user == "alice" && status >= 400'

# Invalid identifiers use event map
kelora --filter 'event["user-name"] == "admin"'
```

**Always available**: `line` (raw text), `event` (field map), `meta` (metadata), `tracked` (global state)

### Built-in Functions

#### String Methods
```rhai
text.matches("ERROR|WARN")        // Regex match
text.replace("\\d+", "XXX")       // Regex replace  
text.extract("https?://([^/]+)")  // Extract capture group
text.slice("0:5")                 // Python-style slicing: first 5 chars
text.slice("6:")                  // From index 6 to end
text.slice(":-5")                 // All but last 5 chars
text.slice("::2")                 // Every 2nd character
text.slice("-5:-1")               // Last 5 chars except the very last
text.slice("::-1")                // Full reverse
text.to_int()                     // Parse integer
text.to_float()                   // Parse float
```

#### Global Tracking
```rhai
track_count("errors")                   // Increment counter
track_min("min_response_time", ms)      // Track minimum
track_max("max_response_time", ms)      // Track maximum (different key!)

// Access in --end stage (read-only)
tracked["errors"]
```

### String Interpolation
```rhai
print(`User ${user} failed with ${status}`)
alert_msg = `Error at ${meta.linenum}: ${message}`
```

### Error Handling Strategies
Four strategies via `--on-error`:
- `skip`: Continue processing, ignore failed lines
- `abort`: Stop on first error  
- `print`: Print errors to stderr, continue
- `stub`: Use empty/default values for failed lines

### Input/Output Format Status

| Input Format | Status | Available Fields |
|-------------|--------|------------------|
| `jsonl` | ✅ | All JSON keys + `line` |
| `line` | ✅ | `line` only |
| `logfmt` | ✅ | All parsed keys + `line` |
| `syslog` | ✅ | `pri`, `facility`, `severity`, `timestamp`, `host`, `prog`, `pid`, `msgid`, `msg`, `line` |
| `csv` | ❌ | Column headers + `line` |
| `apache` | ✅ | `ip`, `identity`, `user`, `timestamp`, `request`, `method`, `path`, `protocol`, `status`, `bytes`, `referer`, `user_agent`, `line` |

| Output Format | Status | Description |
|--------------|--------|-------------|
| `jsonl` | ✅ | JSON objects |
| `text` | ✅ | Key=value pairs (logfmt style) |
| `csv` | ❌ | Comma-separated values |

## Example Use Cases

### Error Analysis
```bash
kelora -f jsonl \
  --filter 'status >= 400' \
  --exec 'track_count(status.status_class())' \
  --end 'print(`4xx: ${tracked["4xx"] ?? 0}, 5xx: ${tracked["5xx"] ?? 0}`)'
```

### Performance Monitoring  
```bash
kelora -f jsonl \
  --exec 'track_min("min_time", response_time); track_max("max_time", response_time)' \
  --end 'print(`Response time range: ${tracked["min_time"]}-${tracked["max_time"]}ms`)'
```

### Syslog Analysis
```bash
kelora -f syslog \
  --filter 'severity <= 3' \
  --exec 'track_count("errors"); track_unique("hosts", host);' \
  --end 'print(`Errors: ${tracked["errors"]}, Hosts: ${tracked["hosts"].len()}`)'
```

### Apache Log Analysis
```bash
kelora -f apache access.log \
  --filter 'status >= 400' \
  --exec 'track_count("errors"); track_bucket("methods", method)' \
  --end 'print(`Errors: ${tracked["errors"]}, by method: ${tracked["methods"]}`)'
```

### Data Transformation
```bash
kelora -f jsonl \
  --exec 'severity = if level == "ERROR" { "high" } else { "low" }; processed_at = "2024-01-01"' \
  -F jsonl
```

### Compressed Log Processing
```bash
# Process single gzip file (automatic decompression)
kelora -f jsonl app.log.1.gz --filter 'status >= 400'

# Process log rotation sequence
for log in app.log.*.gz; do
  kelora -f jsonl "$log" --filter 'level == "ERROR"'
done

# Handle mixed compressed/uncompressed log rotation with chronological order
# Matches: app.log app.log.1 app.log.2.gz app.log.3.gz (.gz files auto-decompressed)
kelora -f jsonl --file-order mtime app.log*

# ZIP files require manual extraction
unzip logs.zip && kelora -f jsonl extracted_file.log
```

## Documentation

- Rhai syntax and features: https://rhai.rs/book/
- Rhai integration patterns and best practices documented in source code

## Development Roadmap

### 🚀 Completed Features
- ✅ **Core Architecture**: Rhai engine integration with AST compilation and reuse
- ✅ **JSONL Input Format**: Full support for JSON Lines log processing
- ✅ **Logfmt Input Format**: Key=value pair parsing with type conversion
- ✅ **Syslog Input Format**: RFC3164/RFC5424 parsing with priority, facility, severity extraction
- ✅ **JSONL/Text Output**: JSON objects and logfmt-style key=value output
- ✅ **Expression Stages**: `--begin`, `--filter`, `--exec`, `--end` pipeline
- ✅ **Global State Tracking**: `track_count()`, `track_min()`, `track_max()` functions
- ✅ **Error Handling**: Four strategies (skip, abort, print, stub)
- ✅ **Field Filtering**: `--keys` for selecting specific output fields
- ✅ **Parallel Processing**: High-throughput batch processing with `--parallel`
- ✅ **Threading**: Configurable worker threads and batch sizes
- ✅ **Order Preservation**: Ordered output by default, `--unordered` for speed
- ✅ **Apache Format Parser**: Common Log Format and Combined Log Format with method/path/protocol extraction
- ✅ **Automatic Gzip Decompression**: Streaming decompression of `.gz` files detected by extension
- ✅ **Multiple Input Files**: Process multiple files with configurable ordering (CLI order, alphabetical, modification time)
- ✅ **Mixed File Support**: Handle compressed and uncompressed files together automatically

### 📋 TODO: Missing Input Formats
- ❌ **CSV Format Parser**: Comma-separated values with header support

### 📋 TODO: Missing Output Formats  
- ❌ **CSV Output Formatter**: Comma-separated values output

### 📋 TODO: Missing Rhai Functions
These functions are documented in DESIGN.md but not yet implemented:

#### Column Parsing Functions
```rhai
line.cols(0)              // First column
line.cols(-1)             // Last column  
line.cols("1:3")          // Columns 1-2 (slice)
line.cols("2:")           // From column 2 to end
line.cols(0, 2, 4)        // Multiple columns
```

#### String Analysis Functions
```rhai
text.matches("ERROR|WARN")        // Regex match
text.replace("\\d+", "XXX")       // Regex replace
text.extract("https?://([^/]+)")  // Extract capture group
text.extract_pattern("email")     // Built-in patterns
text.to_ts()                      // Parse timestamp
```

#### Advanced Tracking Functions
```rhai
track_unique(tracked, "ips", ip)         // Collect unique values
track_bucket(tracked, "status", code)    // Count by value
```

### 📋 TODO: Missing CLI Options
- ❌ **`--no-inject`**: Disable field auto-injection
- ❌ **`--inject-prefix`**: Prefix for injected variables

### ✅ Implemented CLI Options
- ✅ **`--file-order`**: File processing order (none, name, mtime)
- ✅ **Automatic decompression**: `.gz` files are automatically decompressed based on extension (ZIP files not supported, use manual extraction)

### 📋 TODO: Development Tasks
- ❌ **Unit Tests**: Comprehensive test suite for all components
- ✅ **Integration Tests**: Complete test suite covering current CLI interface and functionality
- ✅ **Performance Benchmarks**: Baseline measurements and regression testing with comprehensive benchmark suite
- ❌ **Documentation**: User guide and API documentation

### 📋 TODO: Advanced Features
- ❌ **Streaming Timeout Logic**: True timeout-based batching for sparse streams
- ❌ **Memory Management**: Resource limits and cleanup
- ❌ **Configuration Files**: YAML/TOML config file support
