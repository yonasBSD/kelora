# Handle Multiline Stacktraces

Process logs with multiline stack traces, exceptions, and continuation patterns.

## Problem

You have log files where single events span multiple lines (stack traces, JSON arrays, continued messages), and you need to group them into complete events before parsing and analysis.

## Solutions

### Timestamp-Based Multiline

Detect new events by timestamp prefix:

```bash
# Auto-detect timestamps
kelora --multiline timestamp app.log

# With custom timestamp format
kelora --multiline 'timestamp:format=%Y-%m-%d %H:%M:%S' app.log

# Example: Process stack traces as single events
kelora --multiline timestamp examples/multiline_stacktrace.log -n 2
```

When a line starts with a timestamp, it begins a new event. Lines without timestamps are continuation lines that belong to the previous event.

### Indent-Based Multiline

Group lines based on indentation:

```bash
# Lines starting with whitespace continue previous event
kelora --multiline indent app.log

# Works for stack traces with indented frames
kelora --multiline indent examples/multiline_indent.log
```

Common for:

- Python/Java stack traces
- YAML-like logs
- Indented continuation lines

### Regex-Based Multiline

Define custom start/end patterns:

```bash
# Match start pattern only
kelora --multiline 'regex:match=^ERROR' app.log

# Match start and end patterns
kelora --multiline 'regex:match=^BEGIN:end=^END' app.log

# Match exception start
kelora --multiline 'regex:match=^(ERROR|Exception|Traceback)' app.log
```

### Buffer Entire Input

Process entire file as one event:

```bash
# Read all content as single event
kelora --multiline all config.json

# Useful for:
# - Single JSON document spanning multiple lines
# - Small config files
# - Aggregating entire input for summary
```

⚠️ Warning: Loads entire input into memory

## Real-World Examples

### Python Stack Traces

```bash
# Group Python exceptions
kelora --multiline 'regex:match=^Traceback|^Exception|^\d{4}-' app.log \
  -e 'e.error_type = e.line.extract_re(r"^(\w+Error|Exception)", 1)' \
  -e 'track_count(e.error_type)' \
  --metrics
```

### Java Exceptions

```bash
# Group Java stack traces (look for timestamps or exception class names)
kelora --multiline 'regex:match=^\d{4}-\d{2}-\d{2}|^[a-z]+\.\w+\.Exception' app.log \
  --filter 'e.line.contains("Exception")' \
  -e 'e.exception = e.line.extract_re(r"(\w+Exception)", 1)'
```

### HTTP Request/Response Logs

```bash
# Group multi-line HTTP logs
kelora --multiline 'regex:match=^> Request|^< Response' http.log \
  -e 'e.is_request = e.line.starts_with("> Request")' \
  -e 'e.status = e.line.extract_re(r"HTTP/\d\.\d (\d+)", 1)'
```

### JSON Arrays in Logs

```bash
# Process logs with embedded multi-line JSON
kelora --multiline timestamp examples/multiline_json_arrays.log \
  -e 'let json_match = e.line.extract_re(r"\{.*\}", 0);
          if json_match != "" {
            e.data = json_match.parse_json()
          }'
```

### Continuation Lines

```bash
# Lines starting with specific marker continue previous
kelora --multiline 'regex:match=^[^\s]' app.log  # New event if NOT starting with space
```

### Extract Stack Trace Details

```bash
kelora --multiline timestamp app.log \
  --filter 'e.line.contains("Traceback") || e.line.contains("Exception")' \
  -e 'e.file = e.line.extract_re(r"File \"([^\"]+)\"", 1)' \
  -e 'e.line_no = e.line.extract_re(r"line (\d+)", 1)' \
  -e 'e.function = e.line.extract_re(r"in (\w+)", 1)' \
  -e 'track_unique("error_files", e.file)' \
  -k timestamp,file,line_no,function --metrics
```

### Filter Complete Stack Traces

```bash
# Find errors with specific patterns in full trace
kelora --multiline timestamp app.log \
  --filter 'e.line.contains("DatabaseError")' \
  -e 'e.has_timeout = e.line.contains("timeout")' \
  --filter 'e.has_timeout'
```

### Count Exception Types

```bash
kelora --multiline 'regex:match=^\d{4}-|\w+Error:|Exception:' app.log \
  -e 'e.exception_type = e.line.extract_re(r"(\w+(?:Error|Exception))", 1)' \
  --filter 'e.exception_type != ""' \
  -e 'track_count(e.exception_type)' \
  --metrics
```

### Extract Error Context

```bash
# Get timestamp and full stack trace
kelora --multiline timestamp app.log \
  --filter 'e.line.contains("ERROR")' \
  -e 'e.timestamp = e.line.extract_re(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})", 1)' \
  -e 'e.trace_length = e.line.split("\n").len()' \
  --filter 'e.trace_length > 5'  # Only substantial traces
```

## Multiline Strategy Selection

**Use `timestamp` when:**

- Logs have consistent timestamp prefixes
- Each event starts with a timestamp
- Mixed single-line and multi-line events

**Use `indent` when:**

- Stack traces or continuations are indented
- Clear visual structure with whitespace
- Python, YAML, or similar formats

**Use `regex:match` when:**

- Custom event boundaries
- Specific start/end markers
- Complex event detection logic
- Multiple possible start patterns (use `|` for alternation)

**Use `regex:match:end` when:**

- Events have explicit terminators
- Need both start and end markers
- BEGIN/END style blocks

**Use `all` when:**

- Processing single document files
- Need entire input as context
- Small files only (memory limit)

## Tips

**Performance:**

- Multiline buffering increases memory usage
- Use `--batch-size` to limit buffer growth with `--parallel`
- `--batch-timeout` controls flush timing for streaming

**Debugging:**

- Use `--take 5` to inspect first few events
- Add `--stats` to see event counts
- Use `--verbose` to see parsing issues
- Check line count with `--exec 'e.lines = e.line.count("\n")'`

**Pattern Matching:**

- Test regex patterns carefully
- Use `^` for line start anchors
- Common patterns: `^\d{4}-` (timestamp), `^[A-Z]+:` (severity), `^\s+` (indent)
- Combine patterns with `|`: `^(ERROR|WARN|Exception)`

**Edge Cases:**

- Empty lines may break grouping (filter with `--ignore-lines '^$'`)
- Very long events can cause memory issues
- Incomplete events at EOF are emitted as-is
- With `--parallel`, events may be reordered (use `--batch-size` carefully)

**Format After Grouping:**

- `-f line` keeps full multiline text in `line` field
- `-f json` if continuation contains JSON
- Parse after grouping: `--exec 'e.data = e.line.parse_json()'`

## See Also

- [Find Errors in Logs](find-errors-in-logs.md) - Error filtering patterns
- [Build Streaming Alerts](build-streaming-alerts.md) - Real-time multiline processing
- [CLI Reference](../reference/cli-reference.md) - All multiline options
- [Concepts: Multiline Strategies](../concepts/multiline-strategies.md) - Deep dive into multiline modes
- Run `kelora --help-multiline` for quick reference
