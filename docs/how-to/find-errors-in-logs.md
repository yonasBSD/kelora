# Find Errors in Logs

Quickly find and analyze error-level events across different log formats.

## Problem

You need to find error-level events in your logs, possibly across multiple files or formats, and understand what's happening around them.

## Solutions

### Basic Error Filtering

Filter by log level using `--levels`:

=== "Command"

    ```bash
    # JSON logs - filter for errors and critical
    kelora -j examples/simple_json.jsonl -l error,critical
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    # JSON logs - filter for errors and critical
    kelora -j examples/simple_json.jsonl -l error,critical
    ```

Other level filtering examples:

```bash
# Logfmt logs
kelora -f logfmt service.log -l error,critical

# Multiple levels (error, warn, critical)
kelora -j app.log -l warn,error
```

### Errors from Specific Time Range

Combine level filtering with time bounds:

```bash
# Errors from the last hour
kelora -j app.log -l error --since "1 hour ago"

# Errors in specific date range
kelora -j app.log -l error \
  --since "2024-01-15 09:00:00" \
  --until "2024-01-15 17:00:00"

# Today's errors only
kelora -j app.log -l error --since "today"
```

### Errors from Specific Services

Use `--filter` for fine-grained control:

```bash
# Errors from database service only
kelora -j app.log -l error \
  --filter 'e.service == "database"'

# Errors matching specific pattern
kelora -j app.log -l error \
  --filter 'e.message.contains("timeout")'

# Errors with high severity
kelora -j app.log -l error \
  --filter 'e.get_path("severity", 0) >= 8'
```

### Context Lines

Show surrounding events for context (like `grep -A/-B/-C`):

```bash
# Show 2 lines after each error
kelora -j app.log -l error --after-context 2

# Show 1 line before each error
kelora -j app.log -l error --before-context 1

# Show 2 lines before and after each error
kelora -j app.log -l error \
  --before-context 2 --after-context 2
```

### Extract Key Fields

Focus on relevant information:

```bash
# Show only timestamp, service, and message
kelora -j app.log -l error \
  -k timestamp,service,message

# Include error code if present
kelora -j app.log -l error \
  -e 'e.error_code = e.get_path("error.code", "unknown")' \
  -k timestamp,service,error_code,message
```

### Multiple Files

Search across many log files:

```bash
# All logs in directory
kelora -j logs/*.jsonl -l error

# Recursive search with find
find /var/log -name "*.log" -exec kelora -f auto {} -l error \;

# Gzipped archives
kelora -j logs/2024-01-*.log.gz -l error
```

### Extract Error Patterns

Identify error codes and patterns:

```bash
# Extract error codes using regex
kelora -j app.log -l error \
  -e 'e.error_code = e.message.extract_re(r"ERR-(\d+)", 1)' \
  -k timestamp,error_code,message

# Count error types
kelora -j app.log -l error \
  -e 'track_count(e.get_path("error.type", "unknown"))' \
  --metrics
```

### Output to Different Format

Export errors for further analysis:

```bash
# JSON output
kelora -f logfmt app.log -l error -J > errors.json

# CSV for spreadsheets
kelora -j app.log -l error \
  -k timestamp,service,message -F csv > errors.csv
```

## Real-World Examples

### Find Database Errors

```bash
kelora -j db.log -l error \
  --filter 'e.message.contains("deadlock") || e.message.contains("constraint")' \
  -k timestamp,query,error_message
```

### API Errors with Status Codes

```bash
kelora -f combined /var/log/nginx/access.log \
  --filter 'e.status >= 500' \
  -k ip,timestamp,status,request,user_agent
```

### Application Crashes

```bash
kelora -j app.log -l error,critical \
  --filter 'e.message.contains("panic") || e.message.contains("fatal")' \
  --before-context 5 --after-context 2
```

### Errors by Hour

```bash
kelora -j app.log -l error \
  -e 'e.hour = e.timestamp.format("%Y-%m-%d %H:00")' \
  -e 'track_count(e.hour)' \
  --metrics
```

## Tips

**Performance:**

- Use `--levels` instead of `--filter` when possible (faster)
- Add `--parallel` for large files
- Use `--take 100` to limit output when exploring

**Debugging:**

- Use `--verbose` to see parsing errors
- Use `--stats` to see processing summary
- Use `-F json | jq` for complex JSON analysis

**Automation:**
```bash
# Alert on errors (exit code 0 = no errors, 1 = has errors)
if kelora -q -f json app.log -l error --since "5 minutes ago"; then
    echo "No errors found"
else
    echo "Errors detected!" | mail -s "Alert" admin@example.com
fi
```

## See Also

- [Monitor Application Health](monitor-application-health.md) - Extract health metrics
- [Analyze Web Traffic](analyze-web-traffic.md) - Web server error analysis
- [Function Reference](../reference/functions.md) - All available functions
- [CLI Reference](../reference/cli-reference.md) - Complete flag documentation
