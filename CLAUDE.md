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
./target/release/kelora -f json logs.jsonl --filter "response_time.sub_string(0,2).to_int() > 98"

# Count status codes and track metrics
./target/release/kelora -f json access.log --eval "track_count(status_class(status))" --end "print(tracked)"
```

## Architecture

### Core Components
- **`src/main.rs`** - CLI interface, argument parsing, orchestrates sequential vs parallel processing
- **`src/engine.rs`** - Rhai scripting engine with AST compilation, scope templates, and custom functions
- **`src/event.rs`** - Log event data structure with smart field extraction and metadata tracking
- **`src/parsers.rs`** - Input parsers with trait-based design (currently JSON only)
- **`src/formatters.rs`** - Output formatters with trait-based design (JSON and logfmt text)
- **`src/parallel.rs`** - High-throughput parallel processing with producer-consumer architecture

### Performance Design
Kelora follows a "compile once, evaluate repeatedly" model:
1. **Engine Creation** - Built once at startup with all custom functions registered
2. **AST Compilation** - All user expressions (--filter, --eval, etc.) compiled to ASTs at startup
3. **Scope Templates** - Single scope template cloned and reused for each log line
4. **Variable Injection** - Log fields auto-injected as Rhai variables with fallback to event map
5. **Parallel Architecture** - Producer-consumer model with batching and thread-local state tracking

### Data Flow
**Sequential**: Input → Parser → Event → Filter (Rhai) → Eval (Rhai) → Field Filter → Formatter → Output
**Parallel**: Input → Batching → Worker Threads (Filter/Eval) → State Merging → Output

## Development Guidelines

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
- Test error handling with all four `--on-error` strategies (skip, fail-fast, emit-errors, default-value)
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

## Documentation

- Main design document: `DESIGN.md`
- Rhai syntax and features documented here: https://rhai.rs/book/
- Rhai integration patterns and best practices documented

## Development Roadmap

### 🚀 Completed Features
- ✅ **Core Architecture**: Rhai engine integration with AST compilation and reuse
- ✅ **JSON Input Format**: Full support for JSON log processing
- ✅ **JSON/Text Output**: JSON objects and logfmt-style key=value output
- ✅ **Expression Stages**: `--begin`, `--filter`, `--eval`, `--end` pipeline
- ✅ **Global State Tracking**: `track_count()`, `track_min()`, `track_max()` functions
- ✅ **Error Handling**: Four strategies (skip, fail-fast, emit-errors, default-value)
- ✅ **Field Filtering**: `--keys` for selecting specific output fields
- ✅ **Parallel Processing**: High-throughput batch processing with `--parallel`
- ✅ **Threading**: Configurable worker threads and batch sizes
- ✅ **Order Preservation**: Ordered output by default, `--unordered` for speed

### 📋 TODO: Missing Input Formats
- ❌ **Line Format Parser**: Raw text line processing
- ❌ **CSV Format Parser**: Comma-separated values with header support
- ❌ **Apache Format Parser**: Common Log Format and Combined Log Format

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

### 📋 TODO: Development Tasks
- ❌ **Unit Tests**: Comprehensive test suite for all components
- ✅ **Integration Tests**: Complete test suite covering current CLI interface and functionality
- ❌ **Performance Benchmarks**: Baseline measurements and regression testing
- ❌ **Documentation**: User guide and API documentation

### 📋 TODO: Advanced Features
- ❌ **Multiple File Support**: Process multiple input files
- ❌ **Streaming Timeout Logic**: True timeout-based batching for sparse streams
- ❌ **Memory Management**: Resource limits and cleanup
- ❌ **Configuration Files**: YAML/TOML config file support
