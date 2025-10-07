# Monitor Application Health

Extract health metrics and track service behavior from JSON application logs.

## Problem

You have JSON logs from microservices and need to monitor health, track errors, measure performance, and understand service behavior.

## Solutions

### Basic Health Check

Monitor overall service health:

```bash
> kelora -f json app.log \
    --exec 'track_count(e.level)' \
    --exec 'track_count(e.service)' \
    --metrics
```

### Error Rate Monitoring

Track error rates over time:

```bash
> kelora -f json app.log \
    --exec 'if e.level == "ERROR" || e.level == "CRITICAL" { track_count("errors") }' \
    --exec 'track_count("total")' \
    --metrics
```

Calculate error percentage from metrics output.

### Service-Specific Health

Monitor individual service health:

```bash
> kelora -f json app.log \
    --filter 'e.service == "database"' \
    --exec 'track_count(e.level)' \
    --exec 'track_avg("duration", e.get_path("duration_ms", 0))' \
    --metrics
```

### Response Time Monitoring

Track performance metrics:

```bash
> kelora -f json app.log \
    --filter 'e.has_path("duration_ms")' \
    --exec 'track_avg("response_time", e.duration_ms)' \
    --exec 'track_min("fastest", e.duration_ms)' \
    --exec 'track_max("slowest", e.duration_ms)' \
    --metrics
```

### Memory Usage Tracking

Monitor memory consumption:

```bash
> kelora -f json app.log \
    --filter 'e.has_path("memory_percent")' \
    --exec 'track_avg("memory", e.memory_percent)' \
    --exec 'track_max("peak_memory", e.memory_percent)' \
    --metrics
```

### Endpoint Performance

Analyze API endpoint health:

```bash
> kelora -f json app.log \
    --filter 'e.has_path("path")' \
    --exec 'track_count(e.path)' \
    --exec 'track_avg(e.path, e.get_path("duration_ms", 0))' \
    --metrics
```

## Real-World Examples

### Service Status Dashboard

Generate a comprehensive health report:

```bash
> kelora -f json app.log \
    --exec 'track_count(e.service)' \
    --exec 'track_count(e.level)' \
    --exec 'if e.level == "ERROR" { track_count(e.service + "_errors") }' \
    --exec 'if e.has_path("duration_ms") { track_avg("avg_duration", e.duration_ms) }' \
    --metrics
```

### Failed Operations

Track operations that fail:

```bash
> kelora -f json app.log \
    --filter 'e.get_path("status", "success") != "success"' \
    --exec 'e.operation = e.get_path("operation", "unknown")' \
    --exec 'track_count(e.operation)' \
    --keys timestamp,service,operation,message \
    --metrics
```

### Database Query Health

Monitor database performance:

```bash
> kelora -f json app.log \
    --filter 'e.service == "database"' \
    --exec 'if e.get_path("duration_ms", 0) > 1000 { e.slow = true }' \
    --exec 'track_count("queries")' \
    --exec 'if e.slow { track_count("slow_queries") }' \
    --exec 'track_avg("query_time", e.get_path("duration_ms", 0))' \
    --metrics
```

### Authentication Failures

Track login and auth issues:

```bash
> kelora -f json app.log \
    --filter 'e.service == "auth"' \
    --filter 'e.message.contains("failed") || e.message.contains("locked")' \
    --exec 'track_count(e.username)' \
    --exec 'track_count(e.get_path("ip", "unknown"))' \
    --keys timestamp,username,ip,message \
    --metrics
```

### Cache Performance

Monitor cache hit rates:

```bash
> kelora -f json app.log \
    --filter 'e.service == "cache"' \
    --exec 'if e.message.contains("hit") { track_count("cache_hits") }' \
    --exec 'if e.message.contains("miss") { track_count("cache_misses") }' \
    --exec 'track_count("cache_total")' \
    --metrics
```

### Service Dependencies

Track which services are interacting:

```bash
> kelora -f json app.log \
    --filter 'e.has_path("downstream_service")' \
    --exec 'e.call = e.service + " -> " + e.downstream_service' \
    --exec 'track_count(e.call)' \
    --metrics
```

### Hourly Health Report

Break down health by time:

```bash
> kelora -f json app.log \
    --exec 'e.hour = e.timestamp.format("%Y-%m-%d %H:00")' \
    --exec 'track_count(e.hour)' \
    --exec 'if e.level == "ERROR" { track_count(e.hour + "_errors") }' \
    --metrics
```

### Resource Exhaustion Detection

Find resource pressure points:

```bash
> kelora -f json app.log \
    --filter 'e.level == "WARN" || e.level == "ERROR"' \
    --filter 'e.message.contains("memory") || e.message.contains("disk") || e.message.contains("connection")' \
    --exec 'track_count(e.service)' \
    --keys timestamp,service,level,message
```

### User Activity Tracking

Monitor user-facing operations:

```bash
> kelora -f json app.log \
    --filter 'e.has_path("user_id")' \
    --exec 'track_unique("active_users", e.user_id)' \
    --exec 'track_count(e.get_path("operation", "unknown"))' \
    --metrics
```

## Time-Based Monitoring

### Last Hour's Health

```bash
> kelora -f json app.log \
    --since "1 hour ago" \
    --exec 'track_count(e.level)' \
    --exec 'track_count(e.service)' \
    --metrics
```

### Compare Time Periods

```bash
# Morning traffic
> kelora -f json app.log \
    --since "2024-01-15 06:00:00" \
    --until "2024-01-15 12:00:00" \
    --exec 'track_count(e.level)' \
    --metrics

# Afternoon traffic
> kelora -f json app.log \
    --since "2024-01-15 12:00:00" \
    --until "2024-01-15 18:00:00" \
    --exec 'track_count(e.level)' \
    --metrics
```

### Real-Time Monitoring

=== "Linux/macOS"

    ```bash
    > tail -f /var/log/app.log | kelora -j \
        --exec 'track_count(e.level)' \
        --exec 'if e.level == "ERROR" { eprint("⚠️  ERROR in " + e.service) }' \
        --metrics
    ```

=== "Windows"

    ```powershell
    > Get-Content -Wait app.log | kelora -j \
        --exec 'track_count(e.level)' \
        --exec 'if e.level == "ERROR" { eprint("⚠️  ERROR in " + e.service) }' \
        --metrics
    ```

## Alerting Patterns

### Critical Error Detection

```bash
> kelora -f json app.log \
    --filter 'e.level == "CRITICAL"' \
    --exec 'eprint("🚨 CRITICAL: " + e.service + " - " + e.message)' \
    -qq
```

The `-qq` flag suppresses event output, showing only alerts.

### Threshold Alerts

```bash
> kelora -f json app.log \
    --exec 'if e.get_path("memory_percent", 0) > 90 { eprint("⚠️  High memory: " + e.service + " at " + e.memory_percent + "%") }' \
    --exec 'if e.get_path("duration_ms", 0) > 5000 { eprint("⚠️  Slow request: " + e.get_path("path", "unknown") + " took " + e.duration_ms + "ms") }' \
    -qq
```

### Service Down Detection

```bash
> kelora -f json app.log \
    --filter 'e.message.contains("unavailable") || e.message.contains("timeout") || e.message.contains("unreachable")' \
    --exec 'eprint("🔴 Service issue: " + e.service + " - " + e.message)' \
    --keys timestamp,service,message
```

## Export for Monitoring Tools

### Prometheus-Style Metrics

```bash
> kelora -f json app.log \
    --exec 'track_count("http_requests_total")' \
    --exec 'if e.status >= 500 { track_count("http_requests_errors") }' \
    --exec 'track_avg("http_request_duration_ms", e.get_path("duration_ms", 0))' \
    --metrics
```

### JSON Export for Dashboards

```bash
> kelora -f json app.log \
    --filter 'e.level == "ERROR"' \
    --exec 'e.error_type = e.get_path("error.type", "unknown")' \
    --keys timestamp,service,error_type,message \
    -F json > errors.json
```

### CSV for Spreadsheets

```bash
> kelora -f json app.log \
    --exec 'e.hour = e.timestamp.format("%Y-%m-%d %H:00")' \
    --keys hour,service,level,message \
    -F csv > health_report.csv
```

## Performance Tips

**Large Log Files:**
```bash
> kelora -f json large-app.log.gz \
    --parallel \
    --exec 'track_count(e.service)' \
    --metrics
```

**Sampling for Quick Analysis:**
```bash
> kelora -f json app.log \
    --exec 'if e.user_id.bucket() % 10 == 0 { track_count("sampled") }' \
    --metrics
```

**Focus on Recent Events:**
```bash
> kelora -f json app.log \
    --since "30 minutes ago" \
    --exec 'track_count(e.level)' \
    --metrics
```

## Common Patterns

**Service health summary:**
```bash
> kelora -f json app.log \
    --exec 'track_count(e.service + "_" + e.level)' \
    --metrics
```

**Error rate calculation:**
```bash
> kelora -f json app.log \
    --exec 'track_count("total")' \
    --exec 'if e.level == "ERROR" { track_count("errors") }' \
    --metrics
# Calculate: errors / total * 100
```

**Unique users:**
```bash
> kelora -f json app.log \
    --exec 'track_unique("users", e.user_id)' \
    --metrics
```

**Service call patterns:**
```bash
> kelora -f json app.log \
    --filter 'e.has_path("operation")' \
    --exec 'track_count(e.service + "::" + e.operation)' \
    --metrics
```

## Troubleshooting

**Missing fields:**
```bash
# Use safe access with defaults
e.get_path("nested.field", "default_value")
```

**Inconsistent log formats:**
```bash
# Check if field exists before using
if e.has_path("duration_ms") {
    track_avg("duration", e.duration_ms)
}
```

**Large numbers:**
```bash
# Convert to human-readable
e.duration_s = e.duration_ms / 1000
e.memory_mb = e.memory_bytes / 1024 / 1024
```

## See Also

- [Find Errors in Logs](find-errors-in-logs.md) - Error detection techniques
- [Analyze Web Traffic](analyze-web-traffic.md) - Web server monitoring
- [Function Reference](../reference/functions.md) - All available functions
- [Metrics and Tracking Tutorial](../tutorials/metrics-and-tracking.md) - Deep dive into metrics
