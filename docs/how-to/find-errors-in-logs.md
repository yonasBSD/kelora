# Find Errors in Logs

Quickly find and analyze error-level events across different log formats.

## Problem

You need to find error-level events in your logs, possibly across multiple files or formats, and understand what's happening around them.

## Solutions

### Basic Error Filtering

Filter by log level using `--levels`:

```bash
# JSON logs
kelora -f json app.log --levels error

# Logfmt logs
kelora -f logfmt service.log --levels error,critical

# Multiple levels (error, warn, critical)
kelora -f json app.log --levels warn,error
```

### Errors from Specific Time Range

Combine level filtering with time bounds:

```bash
# Errors from the last hour
kelora -f json app.log --levels error --since "1 hour ago"

# Errors in specific date range
kelora -f json app.log --levels error \
  --since "2024-01-15 09:00:00" \
  --until "2024-01-15 17:00:00"

# Today's errors only
kelora -f json app.log --levels error --since "today"
```

### Errors from Specific Services

Use `--filter` for fine-grained control:

```bash
# Errors from database service only
kelora -f json app.log --levels error \
  --filter 'e.service == "database"'

# Errors matching specific pattern
kelora -f json app.log --levels error \
  --filter 'e.message.contains("timeout")'

# Errors with high severity
kelora -f json app.log --levels error \
  --filter 'e.get_path("severity", 0) >= 8'
```

### Context Lines

Show surrounding events for context (like `grep -A/-B/-C`):

```bash
# Show 2 lines after each error
kelora -f json app.log --levels error --after-context 2

# Show 1 line before each error
kelora -f json app.log --levels error --before-context 1

# Show 2 lines before and after each error
kelora -f json app.log --levels error \
  --before-context 2 --after-context 2
```

### Extract Key Fields

Focus on relevant information:

```bash
# Show only timestamp, service, and message
kelora -f json app.log --levels error \
  --keys timestamp,service,message

# Include error code if present
kelora -f json app.log --levels error \
  --exec 'e.error_code = e.get_path("error.code", "unknown")' \
  --keys timestamp,service,error_code,message
```

### Multiple Files

Search across many log files:

```bash
# All logs in directory
kelora -f json logs/*.jsonl --levels error

# Recursive search with find
find /var/log -name "*.log" -exec kelora -f auto {} --levels error \;

# Gzipped archives
kelora -f json logs/2024-01-*.log.gz --levels error
```

### Extract Error Patterns

Identify error codes and patterns:

```bash
# Extract error codes using regex
kelora -f json app.log --levels error \
  --exec 'e.error_code = e.message.extract_re(r"ERR-(\d+)", 1)' \
  --keys timestamp,error_code,message

# Count error types
kelora -f json app.log --levels error \
  --exec 'track_count(e.get_path("error.type", "unknown"))' \
  --metrics
```

### Output to Different Format

Export errors for further analysis:

```bash
# JSON output
kelora -f logfmt app.log --levels error -F json > errors.json

# CSV for spreadsheets
kelora -f json app.log --levels error \
  --keys timestamp,service,message -F csv > errors.csv
```

## Real-World Examples

### Find Database Errors

```bash
kelora -f json db.log --levels error \
  --filter 'e.message.contains("deadlock") || e.message.contains("constraint")' \
  --keys timestamp,query,error_message
```

### API Errors with Status Codes

```bash
kelora -f combined /var/log/nginx/access.log \
  --filter 'e.status >= 500' \
  --keys ip,timestamp,status,request,user_agent
```

### Application Crashes

```bash
kelora -f json app.log --levels error,critical \
  --filter 'e.message.contains("panic") || e.message.contains("fatal")' \
  --before-context 5 --after-context 2
```

### Errors by Hour

```bash
kelora -f json app.log --levels error \
  --exec 'e.hour = e.timestamp.format("%Y-%m-%d %H:00")' \
  --exec 'track_count(e.hour)' \
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
if kelora -q -f json app.log --levels error --since "5 minutes ago"; then
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
