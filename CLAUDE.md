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
time ./target/release/kelora -f json <logfile> --filter "expression" > /dev/null

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

### Error Handling and Automation Examples
```bash
# Verbose error reporting - see each error immediately
./target/release/kelora -f jsonl --verbose suspicious.log

# Quiet mode for automation - exit codes indicate success/failure
./target/release/kelora -f jsonl --quiet input.log && echo "✓ Processing succeeded"

# Test exit code behavior
./target/release/kelora -f jsonl malformed.log; echo "Exit code: $?"

# Parallel processing with verbose errors
./target/release/kelora -f jsonl --parallel --verbose --batch-size 100 large.log

# Automation pipeline example
if ./target/release/kelora --quiet --filter 'e.level == "ERROR"' logs/*.json; then
    echo "No critical errors found"
else
    echo "Critical errors detected, alerting team..."
    # Send notification, stop deployment, etc.
fi
```

## Development Guidelines

### Architecture Overview

Kelora is built around a streaming pipeline architecture:

1. **Input Stage**: File reading, decompression, and line preprocessing
2. **Parsing Stage**: Format-specific parsing (JSON, syslog, CEF, etc.)
3. **Processing Stage**: Rhai script execution (filter, exec, transform)
4. **Output Stage**: Formatting and writing results

**Key Design Principles:**
- **Resiliency**: Robust error recovery with context-specific handling
- **No Magic**: Explicit behavior, predictable outcomes
- **Composable**: Each stage can be configured independently
- **Performance**: Parallel processing and efficient memory usage

### Empty Line Handling

Empty lines are handled differently based on input format:

**Line Format (`-f line`)**:
- Empty lines are processed as events with `line: ""`
- Maintains line-by-line correspondence for debugging
- Use `--filter 'e.line.len() > 0'` to exclude empty lines if needed

**Structured Formats** (`-f jsonl`, `-f csv`, `-f syslog`, etc.):
- Empty lines are skipped entirely (never reach the parser)
- This prevents noise in structured data processing
- Statistics reflect only non-empty lines that were processed

### Resiliency Model

**Processing Modes:**
- **Resilient Mode (default)**: Skip errors, continue processing, show error summary
- **Strict Mode (`--strict`)**: Fail-fast on any error, show each error immediately

**Context-Specific Error Handling:**

**Input Parsing:**
- Resilient: Skip unparseable lines automatically, continue processing
- Strict: Abort on first parsing error

**Filtering (`--filter` expressions):**
- Resilient: Filter errors evaluate to false (event is skipped)
- Strict: Filter errors abort processing

**Transformations (`--exec` expressions):**
- Resilient: Atomic execution with rollback - failed transformations return original event unchanged
- Strict: Transformation errors abort processing

**Error Reporting:**
- Resilient mode: Shows error summary at end of processing
- Strict mode: Shows each error immediately before aborting
- Use `--error-report-file` to write detailed error logs to file
- Use `--verbose` for immediate verbose error output with emoji formatting
- Use `--quiet` to suppress all kelora output while preserving script side effects

**Verbose Error Output (`--verbose`):**
- Prints each error immediately to stderr with format: `🧱 kelora: line 42: parse error - invalid JSON`
- Works in both sequential and parallel processing modes
- Shows enhanced error summaries with examples when errors occur
- Compatible with all other flags (`--parallel`, `--stats`, etc.)

**Quiet Mode (`--quiet`):**
- Suppresses all kelora output: events, error messages, stats, summaries
- Automatically enables `-F hide` output format (not `-F null`)
- Preserves all Rhai script side effects (`print()` statements, file operations, etc.)
- Exit codes become the primary indicator of processing success/failure
- Essential for automation and CI/CD pipelines

### Exit Codes

Kelora uses standard Unix exit codes to indicate processing results:

**Exit Code 0 (Success):**
- No parsing errors or Rhai runtime errors occurred
- Processing completed successfully
- Filtering events is considered normal behavior (not an error)

**Exit Code 1 (General Error):**
- Parse errors occurred (invalid JSON, malformed syslog, etc.)
- Rhai runtime errors occurred (filter errors, exec errors, script failures)
- Applies to both strict (`--strict`) and resilient (default) modes
- Same behavior whether using `--quiet`, `--verbose`, or normal output

**Exit Code 2 (Invalid Usage):**
- CLI argument errors, invalid flags, missing required parameters
- Configuration file errors, invalid format specifications
- File not found errors, permission issues

**Signal Exit Codes (130+):**
- 130: Interrupted by SIGINT (Ctrl+C)
- 141: Broken pipe (SIGPIPE) - normal in Unix pipelines
- 143: Terminated by SIGTERM

**Automation Examples:**
```bash
# Detect data quality issues in scripts
kelora --quiet input.log && echo "✓ Clean data" || echo "✗ Has errors"

# CI/CD pipeline usage
if kelora --parallel --quiet --filter 'e.level == "ERROR"' logs/*.json; then
    echo "No errors found in logs"
else
    echo "Error-level events detected, exit code: $?"
    exit 1
fi

# Combined with other Unix tools
kelora --quiet suspicious.log || mail -s "Log errors detected" admin@company.com
```

### Output Limiting

**--take N Option:**
- Limits output to the first N events from the input stream
- Works with both sequential and parallel processing modes
- Applies after filtering - returns first N events that pass all filters
- Provides early exit behavior in parallel mode for efficient processing
- Examples:
  - `--take 10` - Output first 10 events
  - `--take 100 --filter 'e.level == "ERROR"'` - First 100 error events
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

- **Event Variable**: Use `e` to access the current event (renamed from `event` for brevity)
- **Variable Declaration**: Always use "let" when using new Rhai variables (e.g. 'let myfield=e.col("1,2")' or 'let myfield=e.col(1,2)')
- **Field and Event Removal**: Use unit `()` assignments for easy field and event removal:
  - `e.field = ()` - Remove individual fields from events
  - `e = ()` - Remove entire event (clears all fields, event becomes empty)
  - Empty events are filtered out before output and counted as "filtered" in stats
  - Empty events continue through all pipeline stages, allowing later stages to add fields back
  - Consistent behavior across all output formats (default, JSONL, CSV, etc.)
  - Examples:
    ```bash
    # Remove sensitive fields
    kelora -e "e.password = (); e.ssn = ()"
    
    # Conditional event removal
    kelora -e "if e.level != 'ERROR' { e = () }"
    
    # Progressive transformation (clear then rebuild)
    kelora -e "let sum = e.a + e.b; e = ()" -e "e.total = sum; e.processed = true"
    ```
- **Safety Functions**: Use defensive field access functions for robust scripts:
  - `get_path(e, "field.subfield", default)` - Safe nested field access with fallback
  - `has_path(e, "field.subfield")` - Check if nested path exists
  - `path_equals(e, "field.subfield", expected)` - Safe nested field comparison
  - `to_number(value, default)` - Safe number conversion with fallback
  - `to_bool(value, default)` - Safe boolean conversion with fallback
- **Side Effects**: Rhai `print()` statements and file operations are preserved in `--quiet` mode
  - Use `print()` for debugging output that should remain visible even when kelora output is suppressed
  - `--quiet` uses `-F hide` (not `-F null`) to maintain script behavior consistency
  - Essential for debugging scripts in automation environments

### Code Quality Practices

- Always run `cargo clippy` before committing.
- Always run `cargo fmt` before committing anything
- Write unit tests for new functionality
- Document public APIs with examples
- Use descriptive variable names and comments for complex logic