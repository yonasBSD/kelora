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

# Output only core fields (timestamp, level, message)
./target/release/kelora -f jsonl app.log --core

# Output core fields plus specific additional fields
./target/release/kelora -f jsonl app.log --core --keys user,status

# Filter events by log level (case-insensitive)
./target/release/kelora -f jsonl app.log --levels debug,error,warn

# Exclude specific log levels (higher priority than --levels)
./target/release/kelora -f jsonl app.log --exclude-levels debug,trace

# Create custom level field and filter by it
./target/release/kelora -f line app.log --exec 'let level = line.before(":")' --levels ERROR,WARN

# Extract columns using integer syntax (cleaner than string selectors)
./target/release/kelora -f line access.log --exec "let user_name=line.col(1,2)" --filter "user_name != ''"
./target/release/kelora -f csv access.csv --exec "let fields=line.cols(0,2,4)" --filter "fields[1] != ''"

# Show processing statistics (lines processed, filtered, timing, performance)
./target/release/kelora -f jsonl app.log --filter "status >= 400" --stats

# Statistics work in both sequential and parallel modes
./target/release/kelora -f jsonl large.log --filter "level == 'ERROR'" --parallel --stats

# Statistics are displayed even when interrupted with CTRL-C
seq 1 1000000 | ./target/release/kelora --filter "line.to_int() % 1000 == 0" --stats
```

## Development Guidelines

### Rhai Scripting Best Practices

- **Variable Declaration**: Always use "let" when using new Rhai variables (e.g. 'let myfield=line.col("1,2")' or 'let myfield=line.col(1,2)').

[... rest of the existing content remains unchanged ...]