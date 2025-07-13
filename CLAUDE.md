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

## Development Guidelines

### Architecture Overview

Kelora is built around a streaming pipeline architecture:

1. **Input Stage**: File reading, decompression, and line preprocessing
2. **Parsing Stage**: Format-specific parsing (JSON, syslog, CEF, etc.)
3. **Processing Stage**: Rhai script execution (filter, exec, transform)
4. **Output Stage**: Formatting and writing results

**Key Design Principles:**
- **Fail Fast**: Invalid data or scripts should error immediately
- **No Magic**: Explicit behavior, predictable outcomes
- **Composable**: Each stage can be configured independently
- **Performance**: Parallel processing and efficient memory usage

### Empty Line Handling

Empty lines are handled differently based on input format:

**Line Format (`-f line`)**:
- Empty lines are processed as events with `line: ""`
- Maintains line-by-line correspondence for debugging
- Use `--filter 'line.len() > 0'` to exclude empty lines if needed

**Structured Formats** (`-f jsonl`, `-f csv`, `-f syslog`, etc.):
- Empty lines are skipped entirely (never reach the parser)
- This prevents noise in structured data processing
- Statistics reflect only non-empty lines that were processed

### Error Handling Patterns

**On-Error Strategies:**
- `quarantine` - Process all lines, isolate broken events, expose via `meta` to Rhai scripts (default)
- `skip` - Skip invalid lines, continue processing
- `abort` - Stop processing on first error

**Error Strategy Selection:**
- Use `quarantine` (default) for analysis and debugging - broken lines become events accessible to Rhai scripts
- Use `skip` for production pipelines where data quality varies and broken lines should be discarded
- Use `abort` for strict validation scenarios where any error should stop processing

### Output Limiting

**--take N Option:**
- Limits output to the first N events from the input stream
- Works with both sequential and parallel processing modes
- Applies after filtering - returns first N events that pass all filters
- Provides early exit behavior in parallel mode for efficient processing
- Examples:
  - `--take 10` - Output first 10 events
  - `--take 100 --filter 'level == "ERROR"'` - First 100 error events
  - `--take 5 --parallel` - First 5 events using parallel processing

### Performance Considerations

**Sequential vs Parallel Processing:**
- Sequential: Maintains order, lower memory usage, simpler debugging
- Parallel: Higher throughput, higher memory usage, may reorder output
- Use `--unordered` with `--parallel` for maximum performance

**Batch Processing:**
- Default batch size: 1000 lines
- Adjust with `--batch-size` for memory vs. throughput tradeoffs
- Use `--batch-timeout` for real-time processing scenarios

**Memory Management:**
- Multi-line mode buffers complete events in memory
- Large batch sizes increase memory usage
- Consider `--stats` overhead for high-volume processing
- Window functionality increases memory usage proportionally to window size

### Testing Approach

**Unit Tests:**
- Test individual parsing functions and Rhai integrations
- Located in `src/` alongside implementation files
- Run with `cargo test --lib`

**Integration Tests:**
- End-to-end CLI testing with sample data
- Located in `tests/integration_tests.rs`
- Run with `cargo test --test integration_tests`

**Performance Tests:**
- Benchmark suites for regression detection
- Run with `make bench` for comprehensive testing
- Use `time` command for quick performance checks

### Timestamp Parsing and Timezone Handling

**Custom Timestamp Formats:**
- Use `--ts-format` to specify custom timestamp formats using chrono format strings
- Use `--help-time` to see comprehensive format reference and examples
- Formats support subseconds, timezones, and various date/time layouts

**Input Timezone Configuration (Parsing Stage):**
- `--input-tz TZ` - Timezone for naive input timestamps (default: UTC)
- Special values: `local` for system local time, `utc` for UTC
- Named timezones: `Europe/Berlin`, `America/New_York`, etc.
- Priority: `--input-tz` > `TZ` environment variable > UTC default
- Only affects naive timestamps (those without explicit timezone info)

**Output Timestamp Formatting (Display Stage):**
- `--pretty-ts field1,field2` - Format specific fields as RFC3339 timestamps
- `-z` - Auto-format all known timestamp fields as local RFC3339
- `-Z` - Auto-format all known timestamp fields as UTC RFC3339
- Only affects default output format (human-readable display)
- No impact on structured outputs (JSON, CSV, etc.) or event data

**Adaptive Parsing:**
- Single consolidated parser handles all timestamp parsing tasks
- Automatically learns and reorders formats for performance
- Supports CLI arguments (--since/--until), event parsing, and Rhai scripts
- Uses input timezone configuration for consistent interpretation

### Rhai Scripting Best Practices

- **Variable Declaration**: Always use "let" when using new Rhai variables (e.g. 'let myfield=line.col("1,2")' or 'let myfield=line.col(1,2)').

### Code Quality Practices

- Always run `cargo clippy` before committing.
- Always run `cargo fmt` before committing anything
- Write unit tests for new functionality
- Document public APIs with examples
- Use descriptive variable names and comments for complex logic