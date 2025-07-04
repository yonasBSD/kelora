# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Kelora is a command-line log analysis tool written in Rust that uses the Rhai scripting engine for flexible log processing. It processes structured logs (JSON, CSV, etc.) and allows users to filter, transform, and analyze log data using embedded Rhai scripts.

## Configuration File Support

Kelora supports INI configuration files for setting defaults and defining aliases, similar to klp.

### Configuration File Locations

Kelora searches for configuration files in the following order:
1. `$XDG_CONFIG_HOME/kelora/config.ini` (Unix)
2. `~/.config/kelora/config.ini` (Unix fallback)
3. `~/.kelorarc` (legacy compatibility)
4. `%APPDATA%\kelora\config.ini` (Windows)
5. `%USERPROFILE%\.kelorarc` (Windows legacy)

### Configuration File Format

```ini
[defaults]
input-format = jsonl
output-format = jsonl
on-error = skip
parallel = true
stats = true

[aliases]
errors = --filter 'level == "error"' --stats
json-errors = --format jsonl --filter 'level == "error"' --output-format jsonl
slow-requests = --filter 'response_time.to_int() > 1000' --keys timestamp,method,path,response_time
```

### Configuration Commands

```bash
# Show current configuration and search paths
./target/release/kelora --show-config

# Use an alias from configuration
./target/release/kelora -a errors /path/to/logs

# Ignore configuration file (use CLI defaults only)
./target/release/kelora --ignore-config --filter "level == 'error'"
```

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
# Basic filtering and parsing
./target/release/kelora -f jsonl logs.jsonl --filter "response_time.sub_string(0,2).to_int() > 98"
./target/release/kelora -f syslog /var/log/syslog --filter 'severity <= 3'
./target/release/kelora /var/log/syslog --filter 'line.matches("ERROR|WARN")'

# CEF (Common Event Format) parsing
./target/release/kelora -f cef security.log --filter 'severity.to_int() >= 7'
./target/release/kelora -f cef firewall.cef --filter 'src.extract_ip() != ""' --keys timestamp,host,vendor,product,event,src,dst

# Tracking and metrics
./target/release/kelora -f jsonl access.log --exec "track_count(status_class(status))" --end "print(tracked)"
./target/release/kelora -f jsonl access.log --exec-file transform.rhai

# File processing and compression
./target/release/kelora -f jsonl logs.jsonl.gz --filter "status >= 400"
./target/release/kelora -f jsonl file1.jsonl file2.jsonl file3.jsonl  # CLI order (default)
./target/release/kelora -f jsonl --file-order name *.jsonl            # Alphabetical order
./target/release/kelora -f jsonl --file-order mtime *.jsonl           # Modification time order
./target/release/kelora -f jsonl --file-order mtime app.log*          # Handle log rotation

# Field selection and filtering
./target/release/kelora -f jsonl app.log --core                       # Core fields only
./target/release/kelora -f jsonl app.log --core --keys user,status    # Core + specific fields
./target/release/kelora -f jsonl app.log --levels debug,error,warn    # Filter by log levels
./target/release/kelora -f jsonl app.log --exclude-levels debug,trace # Exclude specific levels

# Line preprocessing and ignoring patterns
./target/release/kelora -f jsonl app.log --ignore-lines "^#.*|^$"     # Skip comments and empty lines
./target/release/kelora -f line /var/log/syslog --ignore-lines "systemd.*"  # Skip systemd messages
./target/release/kelora -f csv data.csv --ignore-lines "^\"?Date"     # Skip CSV header lines

# Multi-line log event processing
# ⚠️  IMPORTANT: Multi-line mode buffers events until complete. In streaming scenarios,
# the last event may not appear until the next event starts.
./target/release/kelora -f line app.log --multiline indent --filter 'line.contains("Exception")'
./target/release/kelora -f syslog /var/log/syslog --multiline timestamp
./target/release/kelora -f line app.log --multiline timestamp:pattern=^\d{4}-\d{2}-\d{2}
./target/release/kelora -f line config.log --multiline backslash
./target/release/kelora -f line debug.log --multiline start:^ERROR
./target/release/kelora -f line sql.log --multiline end:;$

# Column extraction and data processing
./target/release/kelora -f line access.log --exec "let user_name=line.col(1,2)" --filter "user_name != ''"
./target/release/kelora -f csv access.csv --exec "let fields=line.cols(0,2,4)" --filter "fields[1] != ''"
./target/release/kelora -f jsonl app.log -k timestamp,level,message,user_id

# Performance monitoring and statistics
./target/release/kelora -f jsonl app.log --exec "track_count('total'); track_bucket('status_codes', status.to_string())" --summary
./target/release/kelora -f jsonl app.log --filter "status >= 400" --stats
./target/release/kelora -f jsonl app.log --exec "track_count('errors')" --summary --stats
./target/release/kelora -f jsonl large.log --filter "level == 'ERROR'" --parallel --stats
./target/release/kelora -f jsonl large.log --exec "track_unique('users', user)" --summary --parallel --batch-size 10
seq 1 1000000 | ./target/release/kelora --filter "line.to_int() % 1000 == 0" --stats

# DateTime and duration processing
./target/release/kelora -f jsonl access.log --exec "let dt = parse_timestamp(timestamp); if dt.hour() >= 9 && dt.hour() <= 17 { print('Business hours') }"
./target/release/kelora -f line app.log --exec "let dt = parse_timestamp(line.before(' '), '%Y/%m/%d-%H:%M:%S'); print(dt.format('%Y-%m-%d %H:%M:%S'))"
./target/release/kelora -f jsonl app.log --filter "parse_timestamp(timestamp) > parse_timestamp('2023-07-04T00:00:00Z')"
./target/release/kelora -f jsonl api.log --exec "let dur = parse_duration(response_time); if dur > duration_from_seconds(5) { print('Slow: ' + dur.as_seconds() + 's') }"
./target/release/kelora -f jsonl access.log --exec "let dt = parse_timestamp(timestamp); track_count('hour_' + dt.format('%H'))" --summary
./target/release/kelora -f jsonl requests.log --exec "let start = parse_timestamp(start_time); let end = parse_timestamp(end_time); let duration = end - start; print('Duration: ' + duration.as_milliseconds() + 'ms')"
./target/release/kelora -f jsonl global.log --exec "let utc_time = parse_timestamp(timestamp).to_utc(); print('UTC: ' + utc_time.format('%Y-%m-%d %H:%M:%S %Z'))"

# Real-time and streaming scenarios
kubectl logs app | ./target/release/kelora -f jsonl --filter 'level == "error"' -F text
tail -f /var/log/app.log | ./target/release/kelora -f jsonl --filter 'status >= 400'
```

## CLI Help Organization

The `--help` output is organized into logical sections that follow the data processing pipeline:

1. **Input Options**: File handling and input format (`-f`, `--file-order`, `--ignore-lines`)
2. **Processing Options**: Script execution and processing control (`--begin`, `--filter`, `--exec`, `--exec-file`, `--end`, `--on-error`, `--no-inject`, `--inject-prefix`)
3. **Filtering Options**: Data filtering in the pipeline (`--levels`, `--exclude-levels`, `--keys`/`-k`, `--exclude-keys`/`-K`)
4. **Output Options**: Output formatting (`--output-format`, `--core`, `--brief`)
5. **Performance Options**: Processing optimizations (`--parallel`, `--threads`, `--batch-size`, `--batch-timeout`, `--unordered`)
6. **Display Options**: Visual presentation (`--force-color`, `--no-color`, `--no-emoji`, `--summary`, `--stats`)

### Key Short Options
- `-f` = `--format` (input format)
- `-F` = `--output-format` (output format)  
- `-e` = `--exec` (execute script)
- `-E` = `--exec-file` (execute script from file)
- `-k` = `--keys` (select fields)
- `-K` = `--exclude-keys` (exclude fields)
- `-l` = `--levels` (include log levels)
- `-L` = `--exclude-levels` (exclude log levels)
- `-m` = `--core` (core fields only)
- `-b` = `--brief` (brief output)

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

### Error Handling Patterns

**On-Error Strategies:**
- `skip` - Skip invalid lines, continue processing
- `print` - Print error and original line, continue processing (default)
- `abort` - Stop processing on first error
- `stub` - Insert placeholder event for invalid lines

**Error Strategy Selection:**
- Use `skip` for production pipelines where data quality varies
- Use `print` for debugging and log analysis
- Use `abort` for strict validation scenarios
- Use `stub` when maintaining line count is important

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

### Rhai Scripting Best Practices

- **Variable Declaration**: Always use "let" when using new Rhai variables (e.g. 'let myfield=line.col("1,2")' or 'let myfield=line.col(1,2)').

### Built-in Rhai Functions

#### Data Parsing Functions
- `parse_json(string)` - Parse JSON string into Map
- `parse_kv(string)` - Parse key=value pairs separated by whitespace
- `parse_kv(string, separator)` - Parse key=value pairs with custom separator  
- `parse_kv(string, separator, kv_separator)` - Parse pairs with custom separators

Examples:
```rhai
// Parse JSON
let data = parse_json('{"level": "error", "code": 500}');
let level = data.level;  // "error"

// Parse logfmt-style key=value pairs
let kv = parse_kv("level=info method=GET status=200");
let method = kv.method;  // "GET"

// Parse with custom separators
let data = parse_kv("key1:value1,key2:value2", ",", ":");
let key1 = data.key1;  // "value1"
```

#### String Processing Functions

**Basic String Methods:**
- `lower()` - Convert string to lowercase
- `upper()` - Convert string to uppercase
- `is_digit()` - Check if string contains only ASCII digits
- `count(pattern)` - Count non-overlapping occurrences of substring
- `strip()` - Remove whitespace from both ends
- `strip(chars)` - Remove specific characters from both ends
- `join(array)` - Join array elements with separator string

**Advanced Regex Methods:**
- `extract_re(pattern)` - Extract first capture group (or full match if no groups)
- `extract_re(pattern, group)` - Extract specific capture group by index
- `extract_all_re(pattern)` - Extract all matches with capture groups as arrays
- `extract_all_re(pattern, group)` - Extract specific group from all matches
- `split_re(pattern)` - Split string using regex pattern
- `replace_re(pattern, replacement)` - Replace using regex with capture group support

**Regex Group Indexing:**
- Group 0: Full match (entire matched portion)
- Group 1+: Capture groups (content within parentheses)
- Out-of-bounds groups return empty strings/arrays

Examples:
```rhai
// Basic string operations
let text = "  Hello World  ";
let clean = text.strip();              // "Hello World"
let upper = clean.upper();             // "HELLO WORLD"
let count = upper.count("L");          // 3
let parts = ["a", "b", "c"];
let joined = ",".join(parts);          // "a,b,c"

// Simple regex extraction
let log = "user=alice status=200 level=info";
let user = log.extract_re("user=(\\w+)");              // "alice"
let status = log.extract_re("status=(\\d+)");          // "200"

// Multi-group regex extraction
let pattern = "user=(\\w+).*status=(\\d+)";
let full_match = log.extract_re(pattern, 0);   // "user=alice status=200"
let username = log.extract_re(pattern, 1);     // "alice" 
let status_code = log.extract_re(pattern, 2);  // "200"

// Extract all matches
let logs = "user=alice status=200 user=bob status=404 user=charlie status=500";
let users = logs.extract_all_re("user=(\\w+)", 1);     // ["alice", "bob", "charlie"]
let statuses = logs.extract_all_re("status=(\\d+)", 1); // ["200", "404", "500"]

// Advanced text processing
let csv = "one,two;three:four";
let fields = csv.split_re("[,;:]");     // ["one", "two", "three", "four"]

let emails = "Contact alice@example.com or bob@test.org";
let masked = emails.replace_re("(\\w+)@(\\w+\\.\\w+)", "[$1 at $2]");
// Result: "Contact [alice at example.com] or [bob at test.org]"
```

**Network/IP Methods:**
- `extract_ip()` - Extract first IP address from text
- `extract_ips()` - Extract all IP addresses from text as array
- `mask_ip()` - Mask IP address (default: last octet)
- `mask_ip(octets)` - Mask specified number of octets (1-4)
- `is_private_ip()` - Check if IP is in private ranges
- `extract_url()` - Extract first URL from text
- `extract_domain()` - Extract domain from URLs or emails

Examples:
```rhai
// IP extraction and analysis
let log = "Connection from 192.168.1.100 to 10.0.0.1";
let first_ip = log.extract_ip();           // "192.168.1.100"
let all_ips = log.extract_ips();           // ["192.168.1.100", "10.0.0.1"]

// IP privacy masking (configurable)
let ip = "192.168.1.100";
let masked1 = ip.mask_ip();                // "192.168.1.X" (default: 1 octet)
let masked2 = ip.mask_ip(2);               // "192.168.X.X" (2 octets)
let masked3 = ip.mask_ip(3);               // "192.X.X.X" (3 octets)

// Private IP detection
let private1 = "192.168.1.1".is_private_ip();    // true (RFC 1918)
let private2 = "10.0.0.1".is_private_ip();       // true (RFC 1918)
let private3 = "172.16.0.1".is_private_ip();     // true (RFC 1918)
let public = "8.8.8.8".is_private_ip();          // false (public)

// URL and domain extraction
let text = "Visit https://api.example.com/v1/users for API docs";
let url = text.extract_url();              // "https://api.example.com/v1/users"
let domain = text.extract_domain();        // "api.example.com"

// Email domain extraction (URLs take priority)
let contact = "Email support@test.org for help";
let email_domain = contact.extract_domain(); // "test.org"
```

**Nested Data Access Functions:**
- `get_path(map, path)` - Extract value from nested path with null default
- `get_path(map, path, default)` - Extract value from nested path with custom default
- `get_path(json_string, path)` - Parse JSON string then extract value with null default
- `get_path(json_string, path, default)` - Parse JSON string then extract value with custom default

**Path Syntax:**
- Use dot notation for object keys: `"user.name"`
- Use bracket notation for array indices: `"scores[0]"`
- Combine both for complex paths: `"user.details.items[1].name"`
- Negative array indexing supported: `"scores[-1]"` (last element)

Examples:
```rhai
// Direct map access
let user_map = parse_json('{"name": "alice", "scores": [10, 20, 30]}');
let name = get_path(user_map, "name");           // "alice"
let first_score = get_path(user_map, "scores[0]"); // 10
let last_score = get_path(user_map, "scores[-1]"); // 30

// JSON string parsing (automatic)
let json_str = '{"user": {"name": "bob", "details": {"age": 25, "items": [{"id": 1, "name": "item1"}]}}}';
let user_name = get_path(json_str, "user.name");                    // "bob"
let user_age = get_path(json_str, "user.details.age");             // 25
let item_name = get_path(json_str, "user.details.items[0].name");  // "item1"

// With default values
let missing = get_path(json_str, "user.nonexistent", "default");   // "default"
let missing_num = get_path(json_str, "user.invalid[99]", 0);       // 0

// Common use case with log events
let user_data = get_path(user, "profile.settings.theme", "light"); // Extract theme with fallback
let error_count = get_path(metrics, "errors.count", 0);            // Extract error count with 0 default
```

#### DateTime and Duration Functions

**Timestamp Parsing:**
- `parse_timestamp(s)` - Parse timestamp with automatic format detection
- `parse_timestamp(s, format)` - Parse with explicit format string
- `parse_timestamp(s, format, timezone)` - Parse with format and timezone

**Current Time:**
- `now_utc()` - Get current UTC time
- `now_local()` - Get current local time

**Duration Creation:**
- `duration_from_seconds(n)` - Create duration from seconds
- `duration_from_minutes(n)` - Create duration from minutes  
- `duration_from_hours(n)` - Create duration from hours
- `duration_from_days(n)` - Create duration from days
- `duration_from_milliseconds(n)` - Create duration from milliseconds
- `duration_from_nanoseconds(n)` - Create duration from nanoseconds
- `parse_duration(s)` - Parse human-readable duration ("1h 30m", "2d", etc.)

**DateTime Methods:**
- `dt.year()`, `dt.month()`, `dt.day()` - Get date components
- `dt.hour()`, `dt.minute()`, `dt.second()` - Get time components
- `dt.timestamp_nanos()` - Get Unix timestamp in nanoseconds
- `dt.timezone_name()` - Get timezone name
- `dt.format(fmt)` - Format using strftime format
- `dt.to_utc()` - Convert to UTC timezone
- `dt.to_local()` - Convert to local timezone
- `dt.to_timezone(tz)` - Convert to specific timezone

**Duration Methods:**
- `dur.as_seconds()`, `dur.as_minutes()`, `dur.as_hours()`, `dur.as_days()` - Convert to different units
- `dur.as_milliseconds()`, `dur.as_nanoseconds()` - High precision conversions

**Arithmetic and Comparison:**
- `dt + dur`, `dt - dur` - Add/subtract duration from datetime
- `dt1 - dt2` - Get duration between datetimes (always positive)
- `dur1 + dur2`, `dur1 - dur2` - Duration arithmetic (always positive results)
- `dur * n`, `dur / n` - Duration multiplication/division
- All comparison operators work on both datetime and duration types

Examples:
```rhai
// Parse various timestamp formats (automatic detection)
let dt1 = parse_timestamp("2023-07-04T12:34:56Z");           // ISO 8601
let dt2 = parse_timestamp("04/Jul/2023:12:34:56 +0000");     // Apache logs
let dt3 = parse_timestamp("2023-07-04 12:34:56");           // Common format

// Parse with explicit format and timezone
let dt4 = parse_timestamp("2023/07/04 12:34:56", "%Y/%m/%d %H:%M:%S", "UTC");

// Duration parsing and creation
let dur1 = parse_duration("1h 30m");                        // 90 minutes
let dur2 = parse_duration("2d 3h 45m");                     // Complex duration
let dur3 = duration_from_hours(2);                          // 2 hours

// DateTime component access
let year = dt1.year();                                       // 2023
let formatted = dt1.format("%Y-%m-%d %H:%M:%S");           // "2023-07-04 12:34:56"

// Timezone conversions
let utc_time = dt1.to_utc();
let pst_time = dt1.to_timezone("America/Los_Angeles");

// Duration operations
let minutes = dur1.as_minutes();                            // 90
let seconds = dur1.as_seconds();                            // 5400

// Arithmetic operations
let end_time = dt1 + dur1;                                  // Add 1h 30m to datetime
let duration_between = dt2 - dt1;                          // Duration between times
let double_dur = dur1 * 2;                                 // 3 hours
let half_dur = dur1 / 2;                                   // 45 minutes

// Time-based log analysis
let request_time = parse_timestamp(timestamp);
let process_duration = parse_duration(elapsed);
let end_time = request_time + process_duration;

// Filter by time ranges
if request_time > parse_timestamp("2023-07-04T00:00:00Z") {
    print("Recent request");
}

// Performance analysis
if process_duration > duration_from_seconds(5) {
    print("Slow request: " + process_duration.as_seconds() + "s");
}

// Time bucketing for analysis
let hour_bucket = request_time.format("%Y-%m-%d %H:00:00");
track_count("requests_by_hour_" + hour_bucket);
```