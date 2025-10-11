# Monitor Application Health

Extract health metrics and track service behavior from JSON application logs.

## Problem

You have JSON logs from microservices and need to monitor health, track errors, measure performance, and understand service behavior.

## Solutions

### Basic Health Check

Monitor overall service health:

```bash
> kelora -j app.log \
    -e 'track_count(e.level)' \
    -e 'track_count(e.service)' \
    --metrics
```

### Error Rate Monitoring

Track error rates over time:

```bash
> kelora -j app.log \
    -e 'if e.level == "ERROR" || e.level == "CRITICAL" { track_count("errors") }' \
    -e 'track_count("total")' \
    --metrics
```

Calculate error percentage from metrics output.

### Service-Specific Health

Monitor individual service health:

```bash
> kelora -j app.log \
    --filter 'e.service == "database"' \
    -e 'track_count(e.level)' \
    -e 'track_avg("duration", e.get_path("duration_ms", 0))' \
    --metrics
```

### Response Time Monitoring

Track performance metrics:

```bash
> kelora -j app.log \
    --filter 'e.has_path("duration_ms")' \
    -e 'track_avg("response_time", e.duration_ms)' \
    -e 'track_min("fastest", e.duration_ms)' \
    -e 'track_max("slowest", e.duration_ms)' \
    --metrics
```

### Memory Usage Tracking

Monitor memory consumption:

```bash
> kelora -j app.log \
    --filter 'e.has_path("memory_percent")' \
    -e 'track_avg("memory", e.memory_percent)' \
    -e 'track_max("peak_memory", e.memory_percent)' \
    --metrics
```

### Endpoint Performance

Analyze API endpoint health:

```bash
> kelora -j app.log \
    --filter 'e.has_path("path")' \
    -e 'track_count(e.path)' \
    -e 'track_avg(e.path, e.get_path("duration_ms", 0))' \
    --metrics
```

## Real-World Examples

### Service Status Dashboard

Generate a comprehensive health report:

```bash
> kelora -j app.log \
    -e 'track_count(e.service)' \
    -e 'track_count(e.level)' \
    -e 'if e.level == "ERROR" { track_count(e.service + "_errors") }' \
    -e 'if e.has_path("duration_ms") { track_avg("avg_duration", e.duration_ms) }' \
    --metrics
```

### Failed Operations

Track operations that fail:

```bash
> kelora -j app.log \
    --filter 'e.get_path("status", "success") != "success"' \
    -e 'e.operation = e.get_path("operation", "unknown")' \
    -e 'track_count(e.operation)' \
    -k timestamp,service,operation,message \
    --metrics
```

### Database Query Health

Monitor database performance:

```bash
> kelora -j app.log \
    --filter 'e.service == "database"' \
    -e 'if e.get_path("duration_ms", 0) > 1000 { e.slow = true }' \
    -e 'track_count("queries")' \
    -e 'if e.slow { track_count("slow_queries") }' \
    -e 'track_avg("query_time", e.get_path("duration_ms", 0))' \
    --metrics
```

### Authentication Failures

Track login and auth issues:

```bash
> kelora -j app.log \
    --filter 'e.service == "auth"' \
    --filter 'e.message.contains("failed") || e.message.contains("locked")' \
    -e 'track_count(e.username)' \
    -e 'track_count(e.get_path("ip", "unknown"))' \
    -k timestamp,username,ip,message \
    --metrics
```

### Cache Performance

Monitor cache hit rates:

```bash
> kelora -j app.log \
    --filter 'e.service == "cache"' \
    -e 'if e.message.contains("hit") { track_count("cache_hits") }' \
    -e 'if e.message.contains("miss") { track_count("cache_misses") }' \
    -e 'track_count("cache_total")' \
    --metrics
```

### Service Dependencies

Track which services are interacting:

```bash
> kelora -j app.log \
    --filter 'e.has_path("downstream_service")' \
    -e 'e.call = e.service + " -> " + e.downstream_service' \
    -e 'track_count(e.call)' \
    --metrics
```

### Hourly Health Report

Break down health by time:

```bash
> kelora -j app.log \
    -e 'e.hour = e.timestamp.format("%Y-%m-%d %H:00")' \
    -e 'track_count(e.hour)' \
    -e 'if e.level == "ERROR" { track_count(e.hour + "_errors") }' \
    --metrics
```

### Resource Exhaustion Detection

Find resource pressure points:

```bash
> kelora -j app.log \
    --filter 'e.level == "WARN" || e.level == "ERROR"' \
    --filter 'e.message.contains("memory") || e.message.contains("disk") || e.message.contains("connection")' \
    -e 'track_count(e.service)' \
    -k timestamp,service,level,message
```

### User Activity Tracking

Monitor user-facing operations:

```bash
> kelora -j app.log \
    --filter 'e.has_path("user_id")' \
    -e 'track_unique("active_users", e.user_id)' \
    -e 'track_count(e.get_path("operation", "unknown"))' \
    --metrics
```

## Time-Based Monitoring

### Last Hour's Health

```bash
> kelora -j app.log \
    --since "1 hour ago" \
    -e 'track_count(e.level)' \
    -e 'track_count(e.service)' \
    --metrics
```

### Compare Time Periods

```bash
# Morning traffic
> kelora -j app.log \
    --since "2024-01-15 06:00:00" \
    --until "2024-01-15 12:00:00" \
    -e 'track_count(e.level)' \
    --metrics

# Afternoon traffic
> kelora -j app.log \
    --since "2024-01-15 12:00:00" \
    --until "2024-01-15 18:00:00" \
    -e 'track_count(e.level)' \
    --metrics
```

### Real-Time Monitoring

=== "Linux/macOS"

    ```bash
    > tail -f /var/log/app.log | kelora -j \
        -e 'track_count(e.level)' \
        -e 'if e.level == "ERROR" { eprint("⚠️  ERROR in " + e.service) }' \
        --metrics
    ```

=== "Windows"

    ```powershell
    > Get-Content -Wait app.log | kelora -j \
        -e 'track_count(e.level)' \
        -e 'if e.level == "ERROR" { eprint("⚠️  ERROR in " + e.service) }' \
        --metrics
    ```

## Alerting Patterns

### Critical Error Detection

```bash
> kelora -j app.log \
    --filter 'e.level == "CRITICAL"' \
    -e 'eprint("🚨 CRITICAL: " + e.service + " - " + e.message)' \
    -qq
```

The `-qq` flag suppresses event output, showing only alerts.

### Threshold Alerts

```bash
> kelora -j app.log \
    -e 'if e.get_path("memory_percent", 0) > 90 { eprint("⚠️  High memory: " + e.service + " at " + e.memory_percent + "%") }' \
    -e 'if e.get_path("duration_ms", 0) > 5000 { eprint("⚠️  Slow request: " + e.get_path("path", "unknown") + " took " + e.duration_ms + "ms") }' \
    -qq
```

### Service Down Detection

```bash
> kelora -j app.log \
    --filter 'e.message.contains("unavailable") || e.message.contains("timeout") || e.message.contains("unreachable")' \
    -e 'eprint("🔴 Service issue: " + e.service + " - " + e.message)' \
    -k timestamp,service,message
```

## Export for Monitoring Tools

### Prometheus-Style Metrics

```bash
> kelora -j app.log \
    -e 'track_count("http_requests_total")' \
    -e 'if e.status >= 500 { track_count("http_requests_errors") }' \
    -e 'track_avg("http_request_duration_ms", e.get_path("duration_ms", 0))' \
    --metrics
```

### JSON Export for Dashboards

```bash
> kelora -j app.log \
    --filter 'e.level == "ERROR"' \
    -e 'e.error_type = e.get_path("error.type", "unknown")' \
    -k timestamp,service,error_type,message \
    -J > errors.json
```

### CSV for Spreadsheets

```bash
> kelora -j app.log \
    -e 'e.hour = e.timestamp.format("%Y-%m-%d %H:00")' \
    -k hour,service,level,message \
    -F csv > health_report.csv
```

## Performance Tips

**Large Log Files:**
```bash
> kelora -j large-app.log.gz \
    --parallel \
    -e 'track_count(e.service)' \
    --metrics
```

**Sampling for Quick Analysis:**
```bash
> kelora -j app.log \
    -e 'if e.user_id.bucket() % 10 == 0 { track_count("sampled") }' \
    --metrics
```

**Focus on Recent Events:**
```bash
> kelora -j app.log \
    --since "30 minutes ago" \
    -e 'track_count(e.level)' \
    --metrics
```

## Common Patterns

**Service health summary:**
```bash
> kelora -j app.log \
    -e 'track_count(e.service + "_" + e.level)' \
    --metrics
```

**Error rate calculation:**
```bash
> kelora -j app.log \
    -e 'track_count("total")' \
    -e 'if e.level == "ERROR" { track_count("errors") }' \
    --metrics
# Calculate: errors / total * 100
```

**Unique users:**
```bash
> kelora -j app.log \
    -e 'track_unique("users", e.user_id)' \
    --metrics
```

**Service call patterns:**
```bash
> kelora -j app.log \
    --filter 'e.has_path("operation")' \
    -e 'track_count(e.service + "::" + e.operation)' \
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
