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

# Parse CEF (Common Event Format) logs with syslog prefix
./target/release/kelora -f cef security.log --filter 'severity.to_int() >= 7'

# Parse CEF logs and extract specific fields
./target/release/kelora -f cef firewall.cef --filter 'src.extract_ip() != ""' --keys timestamp,host,vendor,product,event,src,dst

# Count status codes and track metrics
./target/release/kelora -f jsonl access.log --exec "track_count(status_class(status))" --end "print(tracked)"

# Execute script from file
./target/release/kelora -f jsonl access.log --exec-file transform.rhai

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

# Multi-line log event processing
# ⚠️  IMPORTANT: Multi-line mode buffers events until complete. In streaming scenarios,
# the last event may not appear until the next event starts.

# Group Java/Python stack traces by indentation
./target/release/kelora -f line app.log --multiline indent --filter 'line.contains("Exception")'

# Group syslog entries by timestamp (handles continuation lines)
./target/release/kelora -f syslog /var/log/syslog --multiline timestamp

# Group log entries starting with timestamp pattern
./target/release/kelora -f line app.log --multiline timestamp:pattern=^\d{4}-\d{2}-\d{2}

# Group lines that end with backslash continuation
./target/release/kelora -f line config.log --multiline backslash

# Group events starting with ERROR keyword
./target/release/kelora -f line debug.log --multiline start:^ERROR

# Group events ending with semicolon
./target/release/kelora -f line sql.log --multiline end:;$

# For immediate output without buffering (streaming-friendly), omit --multiline entirely
./target/release/kelora -f line app.log

# Extract columns using integer syntax (cleaner than string selectors)
./target/release/kelora -f line access.log --exec "let user_name=line.col(1,2)" --filter "user_name != ''"
./target/release/kelora -f csv access.csv --exec "let fields=line.cols(0,2,4)" --filter "fields[1] != ''"

# Select specific fields using short option
./target/release/kelora -f jsonl app.log -k timestamp,level,message,user_id

# Show summary of tracked values in a table format
./target/release/kelora -f jsonl app.log --exec "track_count('total'); track_bucket('status_codes', status.to_string())" --summary

# Show processing statistics (lines processed, filtered, timing, performance)
./target/release/kelora -f jsonl app.log --filter "status >= 400" --stats

# Combined summary and statistics output (summary appears first)
./target/release/kelora -f jsonl app.log --exec "track_count('errors')" --summary --stats

# Statistics work in both sequential and parallel modes
./target/release/kelora -f jsonl large.log --filter "level == 'ERROR'" --parallel --stats

# Summary also works in parallel mode with different batch sizes
./target/release/kelora -f jsonl large.log --exec "track_unique('users', user)" --summary --parallel --batch-size 10

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