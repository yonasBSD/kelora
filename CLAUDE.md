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
make bench-update             # Refresh the stored baseline intentionally

# Run formatting, lint, and dependency policy checks
make fmt
make lint
make audit
make deny
make check

# Run tests
make test               # Unit and integration tests
make test-unit          # Binary/unit tests only
make test-integration   # Integration tests only
```

### Error Handling and Automation Examples
```bash
# Verbose error reporting - see each error immediately
./target/release/kelora -f json --verbose suspicious.log

# Multi-level quiet mode for automation
./target/release/kelora -f json -q input.log       # Level 1: suppress diagnostics, show events
./target/release/kelora -f json -qq input.log      # Level 2: suppress events too (-F none)
./target/release/kelora -f json -qqq input.log     # Level 3: suppress script output (print/eprint)

# Test exit code behavior
./target/release/kelora -f json malformed.log; echo "Exit code: $?"

# Parallel processing with verbose errors
./target/release/kelora -f json --parallel --verbose --batch-size 100 large.log

# Automation pipeline examples
if ./target/release/kelora -q -l error logs/*.json; then
    echo "No critical errors found"
else
    echo "Critical errors detected, alerting team..."
    # Send notification, stop deployment, etc.
fi

# Clean automation with complete output suppression
./target/release/kelora -qqq --exec 'track_count("errors")' logs/*.json; echo "Exit code: $?"
```

## Configuration System

Kelora uses a simple, clear configuration precedence system:

**Configuration Precedence (highest to lowest):**
1. **CLI arguments** - Always take highest priority
2. **Project `.kelora.ini`** - Found by walking up directory tree from current working directory
3. **User `kelora.ini`** - Located in user's config directory
4. **Built-in defaults** - Fallback values

### Configuration File Locations

**Project Configuration:**
- Kelora searches for `.kelora.ini` starting from the current working directory
- Walks up the directory tree until it finds `.kelora.ini` or reaches the filesystem root
- This allows project-specific defaults that work anywhere within the project structure

**User Configuration:**
- Unix: `$XDG_CONFIG_HOME/kelora/kelora.ini` (fallback to `~/.config/kelora/kelora.ini`)
- Windows: `%APPDATA%\kelora\kelora.ini`

### Configuration File Format

Configuration files use INI format with two main sections:

```ini
# Global defaults applied to every kelora command
defaults = --format auto --stats --input-tz UTC

[aliases]
# Command aliases for common operations
errors = -l error --stats
json-errors = --format json -l error --output-format json
slow-requests = --filter 'e.response_time.to_int() > 1000' --keys timestamp,method,path,response_time
```

### Configuration Commands

```bash
# View current configuration with precedence information
kelora --show-config

# Show help for all available options
kelora --help
```

### Example Usage

**Project Setup:**
```bash
# Create project-specific defaults in your project root
echo 'defaults = --format json --stats --parallel' > .kelora.ini

# All kelora commands in this project (and subdirectories) will use these defaults
kelora input.log                    # Uses project defaults
cd subproject/logs && kelora *.log  # Still finds and uses project defaults
```

**User Setup:**
```bash
# Set personal defaults for all projects
mkdir -p ~/.config/kelora
echo 'defaults = --input-tz America/New_York --stats' > ~/.config/kelora/kelora.ini
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

### Field Selection and Output Formatting

**Default Formatter Bracket Notation:**
The default formatter uses bracket notation for arrays to maintain consistency with `get_path()` syntax:
- Arrays: `scores[0]=85 scores[1]=92` (not `scores.0=85 scores.1=92`)
- Objects: `user.name=alice user.age=25`
- Mixed: `user.scores[0]=85 user.details.items[1].name=item2`

This ensures that field access patterns in the output match the path syntax used in `get_path()` functions.

**--keys Parameter Design:**
The `--keys` parameter operates only on **top-level field names** in the final processed event, not on nested field paths:

✅ **Supported**: `--keys user,timestamp` (selects top-level fields)  
❌ **Not Supported**: `--keys user.name,user.scores[0]` (nested field paths)

**Rationale**: This design keeps `--keys` simple and predictable, while users can achieve nested field selection through the scripting interface:

```bash
# Extract specific nested fields using get_path() + --keys
kelora -f json \
  --exec 'e.user_name = get_path(e, "user.name", "")' \
  --exec 'e.first_score = get_path(e, "user.scores[0]", 0)' \
  --keys user_name,first_score
```

This approach provides full flexibility while maintaining implementation simplicity.

### Empty Line Handling

Empty lines are handled differently based on input format:

**Line Format (`-f line`)**:
- Empty lines are processed as events with `line: ""`
- Maintains line-by-line correspondence for debugging
- Use `--filter 'e.line.len() > 0'` to exclude empty lines if needed

**Structured Formats** (`-f json`, `-f csv`, `-f syslog`, etc.):
- Empty lines are skipped entirely (never reach the parser)
- This prevents noise in structured data processing
- Statistics reflect only non-empty lines that were processed

### Prefix Extraction

Kelora supports extracting prefixed text from log lines before parsing using `--extract-prefix FIELD` and `--prefix-sep SEPARATOR`:

**Configuration Options:**
- `--extract-prefix FIELD`: Extract text before separator to the specified field name
- `--prefix-sep STRING`: Separator string (default: `|`), can be multiple characters

**How it works:**
1. Extracts text before the first occurrence of the separator
2. Trims whitespace from both the prefix and remaining line
3. Adds the prefix as a field to the parsed event  
4. Passes the remaining line to the selected format parser

**Example Usage:**
```bash
# Docker Compose logs with pipe separator
docker compose logs | kelora --extract-prefix service --filter 'e.service == "web_1"'

# Custom separator for service logs
kelora --extract-prefix service --prefix-sep " :: " --filter 'e.service.contains("auth")' app.log

# Combined with any format parser
kelora -f json --extract-prefix container input.log

# Multi-character separators
kelora --extract-prefix node --prefix-sep " >>> " cluster.log
```

**Input/Output Examples:**
```
web_1    | GET /health 200           → {service: "web_1", line: "GET /health 200"}
db_1     | Connection established    → {service: "db_1", line: "Connection established"}
auth-svc :: User login successful    → {service: "auth-svc", line: "User login successful"}
no-separator-here                    → {line: "no-separator-here"} (no prefix extracted)
 | Empty prefix message              → {line: "Empty prefix message"} (empty prefix ignored)
```

**Integration with Formats:**
Prefix extraction works with any format parser. The prefix becomes a field in the event, and the remaining line is parsed by the specified format:

```bash
# Extract container name, then parse remaining JSON
echo 'web_1 | {"level": "info", "msg": "started"}' | \
  kelora --extract-prefix container -f json

# Output: {"container": "web_1", "level": "info", "msg": "started"}
```

### Combined Log Format (`-f combined`)

Kelora supports parsing web server logs from both Apache and NGINX using a unified combined format:

**Supported Log Formats:**
- **Apache Common Log Format**: `IP - - [timestamp] "request" status bytes`
- **Apache Combined Log Format**: `IP - - [timestamp] "request" status bytes "referer" "user-agent"`
- **NGINX Combined with request time**: `IP - - [timestamp] "request" status bytes "referer" "user-agent" "request_time"`

**Output Fields:**
- `ip` (required): Client IP address or hostname
- `identity` (optional): RFC 1413 identity (usually omitted when `-`)
- `user` (optional): HTTP authenticated username (omitted when `-`)
- `timestamp` (required): Request timestamp in Apache format
- `request` (required): Full HTTP request line
- `method`, `path`, `protocol` (auto-extracted): Components of the request
- `status` (required): HTTP response status code
- `bytes` (optional): Response size in bytes (omitted when `-`, included when `0`)
- `referer` (optional): HTTP referer header (Combined format only, omitted when `-`)
- `user_agent` (optional): HTTP user agent header (Combined format only, omitted when `-`)
- `request_time` (optional): Request processing time in seconds (NGINX only, omitted when `-`)

**Example Usage:**
```bash
# Parse Apache logs
tail -f /var/log/apache2/access.log | kelora -f combined --filter 'e.status >= 400'

# Parse NGINX logs with request time
tail -f /var/log/nginx/access.log | kelora -f combined --filter 'e.request_time > 1.0'

# Auto-detection works for both
cat webserver.log | kelora -f auto --exec 'e.slow_request = e.request_time > 0.5'
```

**Format Examples:**
```
# Apache Common
192.168.1.1 - - [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234

# Apache Combined  
192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234 "http://example.com/" "Mozilla/4.08"

# NGINX with request time
192.168.1.1 - - [25/Dec/1995:10:00:00 +0000] "GET /api/test HTTP/1.1" 200 1234 "-" "curl/7.68.0" "0.123"
```

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
- Use `--verbose` for immediate verbose error output with emoji formatting
- Use `-q`, `-qq`, or `-qqq` for graduated quiet modes (see Multi-Level Quiet Mode section)

**Verbose Error Output (`--verbose`):**
- Prints each error immediately to stderr with format: `⚠️  kelora: line 42: parse error - invalid JSON`
- Works in both sequential and parallel processing modes
- Shows enhanced error summaries with examples when errors occur
- Compatible with all other flags (`--parallel`, `--stats`, etc.)
- Uses standardized emoji prefixes: 🔹 (blue diamond) for general output, ⚠️  (warning) for errors

**Multi-Level Quiet Mode (`-q`, `-qq`, `-qqq`):**
- **Level 1 (-q)**: Suppress kelora diagnostics (error summaries, stats, format detection messages)
- **Level 2 (-qq)**: Additionally suppress event output (automatically enables `-F none`)
- **Level 3 (-qqq)**: Additionally suppress all Rhai script side effects (`print()`, `eprint()` statements)
- Exit codes become the primary indicator of processing success/failure
- Essential for automation and CI/CD pipelines with graduated noise control

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
- Same behavior whether using quiet modes (`-q`, `-qq`, `-qqq`), `--verbose`, or normal output

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
kelora -qq input.log && echo "✓ Clean data" || echo "✗ Has errors"

# CI/CD pipeline usage
if kelora --parallel -q -l error logs/*.json; then
    echo "No errors found in logs"
else
    echo "Error-level events detected, exit code: $?"
    exit 1
fi

# Combined with other Unix tools
kelora -qq suspicious.log || mail -s "Log errors detected" admin@company.com
```

### Output Limiting

**--take N Option:**
- Limits output to the first N events from the input stream
- Works with both sequential and parallel processing modes
- Applies after filtering - returns first N events that pass all filters
- Provides early exit behavior in parallel mode for efficient processing
- Examples:
  - `--take 10` - Output first 10 events
  - `--take 100 -l error` - First 100 error events
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
- **Field Access**: Use direct field access for nested structures:
  - `e.user.name` - Access nested fields directly
  - `e.scores[1]` - Access array elements by index (supports negative indexing: `e.scores[-1]`)
  - `e.headers["user-agent"]` - Access fields with special characters using bracket notation
  - `e.data.items[0].metadata.tags[0]` - Deep nested access through arrays and objects
  - **Safe Field Access Patterns**:
    - `if "field" in e { e.field } else { "default" }` - Check field existence before access
    - `if e.scores.len() > 1 { e.scores[1] } else { 0 }` - Safe array bounds checking
    - `e.get_path("user.role", "guest")` - Safe nested field access with default
  - **Field Existence Checking**:
    - `"field" in e` - Check if top-level field exists
    - `e.has_path("user.role")` - Check nested field existence
    - `e.scores.len() > 0` - Check if array has elements
    - `type_of(e.field) != "()"` - Check if field has a value (not unit type)
- **JSON Array Handling**: JSON arrays are automatically converted to native Rhai arrays, enabling full array functionality:
  - `sorted(e.scores)` - Sort arrays numerically or lexicographically
  - `reversed(e.items)` - Reverse array order
  - `unique(e.tags)` - Remove duplicate elements
  - `dedup(e.values)` - Remove consecutive duplicates
  - `sorted_by(e.users, "age")` - Sort arrays of objects by field
  - Arrays maintain proper JSON types in output formats (e.g., `-F json`)
  - **Array Processing Examples**:
    ```bash
    # Get top 3 highest scores
    kelora -e "e.top_scores = sorted(e.scores)[-3:]"
    
    # Process tags and create summary
    kelora -e "e.unique_tags = unique(e.tags); e.tag_count = e.tags.len()"
    
    # Sort users by score and extract names
    kelora -e "let sorted_users = sorted_by(e.users, 'score'); e.winner = sorted_users[-1].name"
    ```
- **Array Fan-Out Processing**: Use `emit_each()` to convert arrays into individual events:
  - `emit_each(e.items)` - Fan out array elements as separate events (original event suppressed)
  - `emit_each(e.items, base_map)` - Fan out with common fields added to each event
  - Returns count of events emitted for tracking and metrics
  - Supports both strict and resilient error handling modes
  - **Fan-Out Examples**:
    ```bash
    # Basic fan-out: each user becomes separate event
    kelora -f json --exec "emit_each(e.users)"

    # With base fields: add common context to each event
    kelora -f json --exec "let base = #{batch_id: e.batch_id, host: 'server1'}; emit_each(e.items, base)"

    # Multi-level fan-out: batches → items → active items only
    # Input: {"batches": [{"name": "batch1", "items": [{"id": 1, "status": "active"}, {"id": 2, "status": "inactive"}]}]}
    # Pipeline: batches become events → items become events → filter for active → preserve batch context
    kelora -f json --exec "emit_each(e.batches)" \
                   --exec "let batch_ctx = #{batch_name: e.name}; emit_each(e.items, batch_ctx)" \
                   --filter "e.status == 'active'"
    # Result: id=1 status='active' batch_name='batch1'

    # Count and track emitted events
    kelora -f json --exec "e.item_count = emit_each(e.items); track_sum('total_items', e.item_count)"
    ```
- **Field and Event Removal**: Use unit `()` assignments for easy field and event removal:
  - `e.field = ()` - Remove individual fields from events
  - `e = ()` - Remove entire event (clears all fields, event becomes empty)
  - Empty events are filtered out before output and counted as "filtered" in stats
  - Empty events continue through all pipeline stages, allowing later stages to add fields back
  - Consistent behavior across all output formats (default, JSON, CSV, etc.)
  - Examples:
    ```bash
    # Remove sensitive fields
    kelora -e "e.password = (); e.ssn = ()"
    
    # Conditional event removal
    kelora -e "if e.level != 'ERROR' { e = () }"
    
    # Progressive transformation (clear then rebuild)
    kelora -e "let sum = e.a + e.b; e = ()" -e "e.total = sum; e.processed = true"
    ```
- **Common Log Analysis Patterns**:
  ```bash
  # Extract HTTP request details safely
  kelora -f json --exec '
    let method = e.get_path("request.method", "unknown");
    let status = e.get_path("response.status", 0);
    e.summary = method + " " + status
  '
  
  # Process user activity arrays
  kelora -f json --filter 'e.events.len() > 0' --exec '
    e.event_count = e.events.len();
    e.latest_event = e.events[-1];
    e.event_types = unique(e.events.map(|event| event.type))
  '
  
  # Safe nested field extraction with defaults
  kelora -f json --exec '
    e.user_role = e.get_path("user.role", "guest");
    e.permissions = e.get_path("user.permissions", [])
  '

  # Process nested arrays with fan-out and filtering
  kelora -f json --exec 'emit_each(e.requests)' --filter 'e.status >= 400' \
    --exec 'let base = #{alert_time: now_utc(), severity: "high"}; emit_each(e.errors, base)'
  ```
- **Type Conversion Functions**: Two patterns for type conversion:
  - **Strict conversions** (return `()` on error): `to_int(value)`, `to_float(value)`, `to_bool(value)`
  - **Safe conversions** (with defaults): `to_int_or(value, default)`, `to_float_or(value, default)`, `to_bool_or(value, default)`
  - Use strict variants when input should be valid; use `_or` variants for defensive parsing with fallbacks
- **Safety Functions**: Use defensive field access functions for robust scripts:
  - `path_equals(e, "field.subfield", expected)` - Safe nested field comparison
- **Environment Variables**: Access environment variables for CI/CD and configuration:
  - `get_env(var)` - Get environment variable, returns empty string if not found
  - `get_env(var, default)` - Get environment variable with fallback default
  - Examples: `e.branch = get_env("CI_BRANCH", "main")`, `e.build_id = get_env("BUILD_ID")`
- **Side Effects**: Rhai `print()` statements behavior depends on quiet level
  - **Levels 1-2 (-q, -qq)**: `print()` and `eprint()` output preserved (useful for debugging)
  - **Level 3 (-qqq)**: All script side effects suppressed for complete automation silence
  - File operations and tracking functions remain unaffected at all quiet levels
  - Essential for graduated control in automation environments

### Code Quality Practices

- Always run `cargo clippy` before committing.
- Always run `cargo fmt` before committing anything
- Write unit tests for new functionality
- Document public APIs with examples
- Use descriptive variable names and comments for complex logic
- **Emoji Standardization**: Use consistent emoji prefixes in output
  - 🔹 (small blue diamond) for general output: stats, metrics, processing messages, help tips
  - ⚠️  (warning) for errors: error messages, warnings, failures
  - Avoid doubled emoji by ensuring only the final formatting function adds the prefix
  - Use `--no-emoji` flag to disable emoji output when needed

  ## No Backwards Compatiblity

Do not care for backwards compatiblity.
- Always remember that Rhai allows using functions as methods on the first argument
- When adding Rhai functions, always update the --help-functions screen in src/rhai_functions/docs.rs