# Analyze Web Traffic

Parse and analyze Apache/Nginx access logs to find slow requests, errors, and traffic patterns.

## Problem

You have Apache or Nginx access logs and need to find slow requests, 4xx/5xx errors, traffic patterns, or analyze request distribution.

## Solutions

### Basic Combined Log Parsing

Parse Apache/Nginx combined format logs:

=== "Command"

    ```bash
    kelora -f combined examples/simple_combined.log -n 5
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f combined examples/simple_combined.log -n 5
    ```

The combined format includes: `ip`, `timestamp`, `request`, `method`, `path`, `protocol`, `status`, `bytes`, `referer`, `user_agent`, and optionally `request_time`.

### Find Server Errors (5xx)

=== "Command"

    ```bash
    kelora -f combined examples/simple_combined.log \
        --filter 'e.status >= 500' \
        -k ip,timestamp,status,request
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f combined examples/simple_combined.log \
        --filter 'e.status >= 500' \
        -k ip,timestamp,status,request
    ```

### Find Client Errors (4xx)

```bash
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.status >= 400 && e.status < 500' \
    -k ip,timestamp,status,request
```

### Find Slow Requests

For Nginx logs with `request_time`:

```bash
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.get_path("request_time", "0").to_float() > 1.0' \
    -k ip,request,request_time,status
```

### Traffic by Status Code

Count requests by status code:

=== "Command"

    ```bash
    kelora -f combined examples/simple_combined.log \
        -e 'track_count("status_" + e.status)' \
        --metrics
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f combined examples/simple_combined.log \
        -e 'track_count("status_" + e.status)' \
        --metrics
    ```

### Top IPs by Request Count

```bash
kelora -f combined /var/log/nginx/access.log \
    -e 'track_count(e.ip)' \
    --metrics
```

### Analyze Specific Endpoints

```bash
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.path.contains("/api/")' \
    -e 'track_count(e.path)' \
    --metrics
```

### Find Suspicious Activity

Look for unusual patterns:

```bash
# High request rates from single IP
kelora -f combined /var/log/nginx/access.log \
    -e 'track_count(e.ip)' \
    --metrics

# POST requests to unusual paths
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.method == "POST" && !e.path.starts_with("/api/")' \
    -k ip,timestamp,method,path

# Large response sizes
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.get_path("bytes", "0").to_int() > 10000000' \
    -k ip,path,bytes,timestamp
```

### Time-Based Analysis

Analyze traffic in specific time windows:

```bash
# Last hour's errors
kelora -f combined /var/log/nginx/access.log \
    --since "1 hour ago" \
    --filter 'e.status >= 400'

# Traffic during specific time range
kelora -f combined /var/log/nginx/access.log \
    --since "2024-01-15 09:00:00" \
    --until "2024-01-15 17:00:00" \
    -e 'track_count(e.status)' \
    --metrics
```

### Response Time Percentiles

Calculate performance metrics for Nginx logs with `request_time`:

```bash
kelora -f combined /var/log/nginx/access.log \
    -e 'track_bucket("latency", floor(e.get_path("request_time", "0").to_float() * 1000 / 100) * 100)' \
    --metrics
```

## Real-World Examples

### Daily Error Report

```bash
kelora -f combined /var/log/nginx/access.log* \
    --filter 'e.status >= 400' \
    -e 'e.hour = e.timestamp.extract_re(r"(\d{2}):\d{2}:\d{2}", 1)' \
    -e 'track_count(e.hour)' \
    -e 'track_count(e.status)' \
    --metrics
```

### API Endpoint Performance

```bash
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.path.starts_with("/api/")' \
    -e 'e.endpoint = e.path.extract_re(r"(/api/[^/]+)", 1)' \
    -e 'track_count(e.endpoint)' \
    -e 'track_avg(e.endpoint, e.get_path("request_time", "0").to_float())' \
    --metrics
```

### Bot Detection

```bash
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.user_agent.contains("bot") || e.user_agent.contains("crawler")' \
    -e 'track_count(e.user_agent)' \
    -k ip,user_agent,path \
    --metrics
```

### Referer Analysis

Find where traffic is coming from:

```bash
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.referer != "-" && !e.referer.contains("yourdomain.com")' \
    -e 'e.domain = e.referer.extract_domain()' \
    -e 'track_count(e.domain)' \
    --metrics
```

### Failed Authentication Attempts

```bash
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.path.contains("/login") && e.status == 401' \
    -e 'track_count(e.ip)' \
    -k timestamp,ip,path,status \
    --metrics
```

### Response Size Distribution

```bash
kelora -f combined /var/log/nginx/access.log \
    -e 'e.size_kb = floor(e.get_path("bytes", "0").to_int() / 1024)' \
    -e 'track_bucket("response_size_kb", e.size_kb)' \
    --metrics
```

## Export for Analysis

### Export to CSV

```bash
kelora -f combined /var/log/nginx/access.log \
    -k ip,timestamp,status,bytes,request \
    -F csv > access.csv
```

### Export to JSON

```bash
kelora -f combined /var/log/nginx/access.log \
    --filter 'e.status >= 400' \
    -J > errors.json
```

## Performance Tips

**Large Files:**
```bash
# Use parallel processing
kelora -f combined /var/log/nginx/access.log.* \
    --parallel \
    --filter 'e.status >= 500'

# Limit output
kelora -f combined access.log -n 1000
```

**Gzipped Archives:**
```bash
# Kelora handles .gz automatically
kelora -f combined /var/log/nginx/access.log.*.gz \
    --filter 'e.status >= 500'
```

**Multiple Files:**
```bash
# Process all access logs
kelora -f combined /var/log/nginx/access.log* \
    -e 'track_count(e.status)' \
    --metrics
```

## Common Patterns

**Find top N IPs by error count:**
```bash
kelora -f combined access.log \
    --filter 'e.status >= 400' \
    -e 'track_count(e.ip)' \
    --metrics
```

**Hourly request distribution:**
```bash
kelora -f combined access.log \
    -e 'e.hour = e.timestamp.extract_re(r"(\d{2}):\d{2}:\d{2}", 1)' \
    -e 'track_count(e.hour)' \
    --metrics
```

**Method distribution:**
```bash
kelora -f combined access.log \
    -e 'track_count(e.method)' \
    --metrics
```

**Status code summary:**
```bash
kelora -f combined access.log \
    -e 'e.status_class = floor(e.status / 100) + "xx"' \
    -e 'track_count(e.status_class)' \
    --metrics
```

## Troubleshooting

**Timestamp parsing issues:**
```bash
# If auto-detect misses, inspect the stats line:
# Timestamp: auto-detected timestamp — parsed 0 of 100 detected events (0.0%). Hint: Try --ts-field or --ts-format.
# Then supply an explicit format:
kelora -f combined --ts-format "%d/%b/%Y:%H:%M:%S %z" access.log
```

**Missing request_time field:**
```bash
# Apache combined format doesn't include request_time
# Only Nginx with custom log format includes it
# Use safe access with get_path()
e.get_path("request_time", "0")
```

**Large numbers in bytes field:**
```bash
# Convert to MB for readability
e.bytes_mb = e.get_path("bytes", "0").to_float() / 1024 / 1024
```

## See Also

- [Find Errors in Logs](find-errors-in-logs.md) - General error finding techniques
- [Monitor Application Health](monitor-application-health.md) - Application-level monitoring
- [Function Reference](../reference/functions.md) - All available functions
- [Concepts: Pipeline Model](../concepts/pipeline-model.md) - How processing works
