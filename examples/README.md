# Kelora Examples

This directory contains sample files for testing Kelora with different log formats, edge cases, and real-world scenarios. Use these files to experiment with filters, transformations, and parsing strategies before processing your own logs.

For detailed guides and tutorials, see the [documentation](https://kelora.dev).

## Quick Start

New to Kelora? Try these first:

```bash
# Basic filtering and transformation
kelora examples/quickstart.log --filter 'e.line.contains("ERROR")'

# JSON log analysis
kelora -f json examples/simple_json.jsonl --filter 'e.level == "ERROR"' --exec '#{level: e.level, message: e.message}'

# Web access log parsing
kelora examples/web_access.log --filter 'e.status >= 400'

# Using Rhai helper functions
kelora --include examples/helpers.rhai examples/api_logs.jsonl --exec 'if is_problem(e) { e } else { () }'
```

Then run `kelora --help-examples` for common patterns and usage recipes.

## File Organization

Examples follow a naming convention for easy discovery:

### Basic Format Examples (`simple_*`)

Start here to understand Kelora's format auto-detection:

- `simple_json.jsonl` - Structured JSON logs
- `simple_csv.csv` - Comma-separated values with headers
- `simple_tsv.tsv` - Tab-separated values
- `simple_logfmt.log` - Logfmt key=value format
- `simple_syslog.log` - Standard syslog messages
- `simple_combined.log` - Apache combined log format
- `simple_cef.log` - Common Event Format
- `simple_line.log` - Unstructured text logs

### Error Handling & Edge Cases (`errors_*`)

Test Kelora's robustness with malformed or unusual input:

- `errors_json_mixed.jsonl` - Mixed valid/invalid JSON
- `errors_json_types.jsonl` - Type handling edge cases
- `errors_csv_ragged.csv` - Rows with varying column counts
- `errors_empty_lines.log` - Empty lines and whitespace
- `errors_unicode.log` - Unicode handling
- `errors_filter_runtime.jsonl` - Filter expression errors
- `errors_exec_transform.jsonl` - Transformation errors

### Multiline Handling (`multiline_*`)

Different strategies for parsing multi-line log entries:

- `multiline_stacktrace.log` - Stack traces and exceptions
- `multiline_continuation.log` - Line continuation patterns
- `multiline_indent.log` - Indentation-based grouping
- `multiline_boundary.log` - Delimiter-based boundaries
- `multiline_json_arrays.log` - JSON arrays spanning lines

See `kelora --help-multiline` for detailed multiline strategies.

### Real-World Scenarios

Production-like log files for testing realistic use cases:

- `api_logs.jsonl` - API gateway requests with nested metadata
- `web_access.log` - Web server access logs
- `security_audit.jsonl` - Security audit events
- `k8s_security.jsonl` - Kubernetes security logs
- `auth_burst.jsonl` - Authentication burst patterns
- `payments_latency.jsonl` - Payment processing latency
- `email_logs.log` - Email delivery logs
- `duration_logs.jsonl` - Performance timing analysis
- `uptime_windows.jsonl` - Service uptime windows
- `incident_story.log` - Simulated incident timeline
- And many more...

### Power-User Technique Examples

Examples for advanced features from the [Power-User Techniques](https://kelora.dev/how-to/power-user-techniques/) guide:

- `production-errors.jsonl` - Pattern normalization with `normalized()`
- `user-activity.jsonl` - Deterministic sampling with `bucket()`
- `deeply-nested.jsonl` - Structure flattening with `flattened()`
- `auth-logs.jsonl` - JWT parsing with `parse_jwt()`
- `error-logs.jsonl` - Fuzzy matching with `edit_distance()`
- `user-data.jsonl` - Multi-algorithm hashing
- `analytics.jsonl` - Privacy-preserving pseudonymization
- `user-events.jsonl` - Stateful processing with `state` map

### Specialized Formats

- `cols_fixed.log`, `cols_mixed.log` - Fixed-width columns
- `csv_typed.csv` - CSV with type inference
- `prefix_docker.log` - Docker container logs with prefixes
- `prefix_custom.log` - Custom prefix patterns
- `custom_timestamps.log` - Non-standard timestamp formats
- `timezones_mixed.log` - Mixed timezone handling
- `kv_pairs.log` - Key-value pair extraction
- `regex_apache_style.log` - Custom regex parsing
- `regex_custom_format.log` - User-defined patterns
- `fan_out_batches.jsonl` - Flattening nested arrays
- `json_nested_deep.jsonl` - Deep object nesting
- `json_arrays.jsonl` - Array handling
- `window_metrics.jsonl` - Time window aggregation
- `sampling_hash.jsonl.gz` - Deterministic sampling (compressed)
- `web_access_large.log.gz` - Large file processing (compressed)

### Stress Tests (`nightmare_*`)

Complex scenarios for testing performance and correctness:

- `nightmare_mixed_formats.log` - Multiple formats in one file
- `nightmare_deeply_nested_transform.jsonl` - Complex nested transformations

## Rhai Helper Scripts

Reusable Rhai functions that you can include in your pipelines with `--include`:

### `helpers.rhai`

Common utility functions for log analysis:

```bash
kelora --include examples/helpers.rhai examples/api_logs.jsonl \
  --exec 'if is_problem(e) { e } else { () }'
```

Functions:
- `is_problem(event)` - Check if event is an error or slow
- `classify_severity(level, value)` - Categorize severity
- `extract_domain(text)` - Extract domain from URL/email
- `mask_sensitive(value)` - Mask sensitive data

### `enrich_events.rhai`

Example of event enrichment and transformation patterns.

## Finding the Right Example

- **By format**: Look for `simple_<format>.*` files
- **By use case**: Browse real-world scenario files (api_logs, web_access, etc.)
- **By feature**: Use prefixes (multiline_, errors_, etc.)
- **By complexity**: Start with `simple_*`, progress to real-world, test with `nightmare_*`

Use `grep`, `ls`, or your editor's file search to quickly locate examples.

## Using Examples

All examples work with Kelora's CLI:

```bash
# Auto-detect format
kelora examples/simple_json.jsonl

# Specify format explicitly
kelora -f json examples/api_logs.jsonl

# Chain operations
kelora examples/web_access.log --filter 'e.status >= 400' --exec '#{ip: e.client_ip, path: e.path}'

# Include helper scripts
kelora --include examples/helpers.rhai examples/api_logs.jsonl --exec 'if is_problem(e) { e } else { () }'

# Compressed files work too
kelora examples/web_access_large.log.gz
```

## Next Steps

- [Documentation](https://kelora.dev) - How-to guides and tutorials
- `kelora --help` - Complete CLI reference
- `kelora --help-functions` - All 150+ built-in Rhai functions
- `kelora --help-examples` - Common usage patterns
- `kelora --help-rhai` - Rhai scripting guide
