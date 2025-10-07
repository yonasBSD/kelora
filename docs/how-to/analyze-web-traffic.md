# Analyze Web Traffic

Parse and analyze Apache/Nginx access logs to find slow requests, errors, and traffic patterns.

## Problem

You have Apache or Nginx access logs and need to find slow requests, 4xx/5xx errors, traffic patterns, or analyze request distribution.

## Solutions

### Basic Combined Log Parsing

Parse Apache/Nginx combined format logs:

```bash
> kelora -f combined /var/log/nginx/access.log --take 5
```

The combined format includes: `ip`, `timestamp`, `request`, `method`, `path`, `protocol`, `status`, `bytes`, `referer`, `user_agent`, and optionally `request_time`.

### Find Server Errors (5xx)

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.status >= 500' \
    --keys ip,timestamp,status,request
```

### Find Client Errors (4xx)

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.status >= 400 && e.status < 500' \
    --keys ip,timestamp,status,request
```

### Find Slow Requests

For Nginx logs with `request_time`:

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.get_path("request_time", "0").to_float() > 1.0' \
    --keys ip,request,request_time,status
```

### Traffic by Status Code

Count requests by status code:

```bash
> kelora -f combined /var/log/nginx/access.log \
    --exec 'track_count(e.status)' \
    --metrics
```

### Top IPs by Request Count

```bash
> kelora -f combined /var/log/nginx/access.log \
    --exec 'track_count(e.ip)' \
    --metrics
```

### Analyze Specific Endpoints

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.path.contains("/api/")' \
    --exec 'track_count(e.path)' \
    --metrics
```

### Find Suspicious Activity

Look for unusual patterns:

```bash
# High request rates from single IP
> kelora -f combined /var/log/nginx/access.log \
    --exec 'track_count(e.ip)' \
    --metrics

# POST requests to unusual paths
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.method == "POST" && !e.path.starts_with("/api/")' \
    --keys ip,timestamp,method,path

# Large response sizes
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.get_path("bytes", "0").to_int() > 10000000' \
    --keys ip,path,bytes,timestamp
```

### Time-Based Analysis

Analyze traffic in specific time windows:

```bash
# Last hour's errors
> kelora -f combined /var/log/nginx/access.log \
    --since "1 hour ago" \
    --filter 'e.status >= 400'

# Traffic during specific time range
> kelora -f combined /var/log/nginx/access.log \
    --since "2024-01-15 09:00:00" \
    --until "2024-01-15 17:00:00" \
    --exec 'track_count(e.status)' \
    --metrics
```

### Response Time Percentiles

Calculate performance metrics for Nginx logs with `request_time`:

```bash
> kelora -f combined /var/log/nginx/access.log \
    --exec 'track_bucket("latency", floor(e.get_path("request_time", "0").to_float() * 1000 / 100) * 100)' \
    --metrics
```

## Real-World Examples

### Daily Error Report

```bash
> kelora -f combined /var/log/nginx/access.log* \
    --filter 'e.status >= 400' \
    --exec 'e.hour = e.timestamp.extract_re(r"(\d{2}):\d{2}:\d{2}", 1)' \
    --exec 'track_count(e.hour)' \
    --exec 'track_count(e.status)' \
    --metrics
```

### API Endpoint Performance

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.path.starts_with("/api/")' \
    --exec 'e.endpoint = e.path.extract_re(r"(/api/[^/]+)", 1)' \
    --exec 'track_count(e.endpoint)' \
    --exec 'track_avg(e.endpoint, e.get_path("request_time", "0").to_float())' \
    --metrics
```

### Bot Detection

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.user_agent.contains("bot") || e.user_agent.contains("crawler")' \
    --exec 'track_count(e.user_agent)' \
    --keys ip,user_agent,path \
    --metrics
```

### Referer Analysis

Find where traffic is coming from:

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.referer != "-" && !e.referer.contains("yourdomain.com")' \
    --exec 'e.domain = e.referer.extract_domain()' \
    --exec 'track_count(e.domain)' \
    --metrics
```

### Failed Authentication Attempts

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.path.contains("/login") && e.status == 401' \
    --exec 'track_count(e.ip)' \
    --keys timestamp,ip,path,status \
    --metrics
```

### Response Size Distribution

```bash
> kelora -f combined /var/log/nginx/access.log \
    --exec 'e.size_kb = floor(e.get_path("bytes", "0").to_int() / 1024)' \
    --exec 'track_bucket("response_size_kb", e.size_kb)' \
    --metrics
```

## Export for Analysis

### Export to CSV

```bash
> kelora -f combined /var/log/nginx/access.log \
    --keys ip,timestamp,status,bytes,request \
    -F csv > access.csv
```

### Export to JSON

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.status >= 400' \
    -F json > errors.json
```

## Performance Tips

**Large Files:**
```bash
# Use parallel processing
> kelora -f combined /var/log/nginx/access.log.* \
    --parallel \
    --filter 'e.status >= 500'

# Limit output
> kelora -f combined access.log --take 1000
```

**Gzipped Archives:**
```bash
# Kelora handles .gz automatically
> kelora -f combined /var/log/nginx/access.log.*.gz \
    --filter 'e.status >= 500'
```

**Multiple Files:**
```bash
# Process all access logs
> kelora -f combined /var/log/nginx/access.log* \
    --exec 'track_count(e.status)' \
    --metrics
```

## Common Patterns

**Find top N IPs by error count:**
```bash
> kelora -f combined access.log \
    --filter 'e.status >= 400' \
    --exec 'track_count(e.ip)' \
    --metrics
```

**Hourly request distribution:**
```bash
> kelora -f combined access.log \
    --exec 'e.hour = e.timestamp.extract_re(r"(\d{2}):\d{2}:\d{2}", 1)' \
    --exec 'track_count(e.hour)' \
    --metrics
```

**Method distribution:**
```bash
> kelora -f combined access.log \
    --exec 'track_count(e.method)' \
    --metrics
```

**Status code summary:**
```bash
> kelora -f combined access.log \
    --exec 'e.status_class = floor(e.status / 100) + "xx"' \
    --exec 'track_count(e.status_class)' \
    --metrics
```

## Troubleshooting

**Timestamp parsing issues:**
```bash
# If timestamps aren't parsed, try explicit format
> kelora -f combined --ts-format "%d/%b/%Y:%H:%M:%S %z" access.log
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
