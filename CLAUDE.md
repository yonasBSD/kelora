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

# Ignore lines matching a regex pattern (applied before parsing for efficiency)
./target/release/kelora -f jsonl app.log --ignore-lines "^#.*|^$"  # Skip comments and empty lines
./target/release/kelora -f line /var/log/syslog --ignore-lines "systemd.*"  # Skip systemd messages

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
./target/release/kelora -f jsonl app.log --core -k user,status  # Short option

# Filter events by log level (case-insensitive)
./target/release/kelora -f jsonl app.log --levels debug,error,warn

# Exclude specific log levels (higher priority than --levels)
./target/release/kelora -f jsonl app.log --exclude-levels debug,trace

# Create custom level field and filter by it
./target/release/kelora -f line app.log --exec 'let level = line.before(":")' --levels ERROR,WARN

# Extract columns using integer syntax (cleaner than string selectors)
./target/release/kelora -f line access.log --exec "let user_name=line.col(1,2)" --filter "user_name != ''"
./target/release/kelora -f csv access.csv --exec "let fields=line.cols(0,2,4)" --filter "fields[1] != ''"

# Select specific fields using short option
./target/release/kelora -f jsonl app.log -k timestamp,level,message,user_id

# Show processing statistics (lines processed, filtered, timing, performance)
./target/release/kelora -f jsonl app.log --filter "status >= 400" --stats

# Statistics work in both sequential and parallel modes
./target/release/kelora -f jsonl large.log --filter "level == 'ERROR'" --parallel --stats

# Statistics are displayed even when interrupted with CTRL-C
seq 1 1000000 | ./target/release/kelora --filter "line.to_int() % 1000 == 0" --stats

# Ignore input lines matching regex patterns (pre-parsing filter for efficiency)
./target/release/kelora -f jsonl app.log --ignore-lines "^#.*|^$"           # Skip comments and empty lines
./target/release/kelora -f line /var/log/syslog --ignore-lines "systemd.*"  # Skip systemd messages
./target/release/kelora -f csv data.csv --ignore-lines "^\"?Date"          # Skip CSV header lines
```

## CLI Help Organization

The `--help` output is organized into logical sections that follow the data processing pipeline:

1. **Input Options**: File handling and input format (`-f`, `--file-order`, `--ignore-lines`)
2. **Processing Options**: Script execution and processing control (`--begin`, `--filter`, `--exec`, `--end`, `--on-error`, `--no-inject`, `--inject-prefix`)
3. **Filtering Options**: Data filtering in the pipeline (`--levels`, `--exclude-levels`, `--keys`/`-k`, `--exclude-keys`/`-K`)
4. **Output Options**: Output formatting (`--output-format`, `--core`, `--brief`)
5. **Performance Options**: Processing optimizations (`--parallel`, `--threads`, `--batch-size`, `--batch-timeout`, `--unordered`)
6. **Display Options**: Visual presentation (`--force-color`, `--no-color`, `--no-emoji`, `--stats`)

### Key Short Options
- `-f` = `--format` (input format)
- `-F` = `--output-format` (output format)  
- `-e` = `--exec` (execute script)
- `-k` = `--keys` (select fields)
- `-K` = `--exclude-keys` (exclude fields)
- `-l` = `--levels` (include log levels)
- `-L` = `--exclude-levels` (exclude log levels)
- `-m` = `--core` (core fields only)
- `-b` = `--brief` (brief output)

## Development Guidelines

### Rhai Scripting Best Practices

- **Variable Declaration**: Always use "let" when using new Rhai variables (e.g. 'let myfield=line.col("1,2")' or 'let myfield=line.col(1,2)').

[... rest of the existing content remains unchanged ...]