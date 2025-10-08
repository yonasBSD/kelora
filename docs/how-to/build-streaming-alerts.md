# Build Streaming Alerts

Monitor logs in real-time and trigger alerts based on conditions, metrics, and thresholds.

## Problem

You need to monitor logs as they're written, detect critical events, and trigger alerts or take action when conditions are met. Traditional batch processing is too slow for real-time alerting.

## Solutions

### Basic Real-Time Monitoring

Monitor logs as they're written using `tail -f`:

```bash
# Monitor JSON logs for errors
> tail -f /var/log/app.log | kelora -j \
    --levels error,critical

# Monitor with custom filter
> tail -f /var/log/app.log | kelora -j \
    --filter 'e.response_time > 1000'

# Monitor multiple log levels
> tail -f /var/log/app.log | kelora -j \
    --levels warn,error,critical \
    --keys timestamp,level,service,message
```

### Alert on Critical Events

Use `eprint()` to write alerts to stderr while suppressing normal output:

```bash
# Alert on critical level
> tail -f /var/log/app.log | kelora -j \
    --exec 'if e.level == "CRITICAL" { eprint("ALERT: " + e.service + " - " + e.message) }' \
    -qq

# Alert on high error rate
> tail -f /var/log/app.log | kelora -j \
    --exec 'track_count("total"); track_count(e.level)' \
    --exec 'if e.level == "ERROR" { eprint("Error in " + e.service) }' \
    -q

# Alert on slow requests
> tail -f /var/log/nginx/access.log | kelora -f combined \
    --filter 'e.get_path("request_time", "0").to_float() > 2.0' \
    --exec 'eprint("SLOW: " + e.path + " took " + e.request_time + "s")' \
    -qq

### Count Incidents While Streaming

```bash
> tail -f examples/simple_logfmt.log | \
    kelora -f logfmt \
      --filter '"duration" in e && e.duration.to_int_or(0) >= 1000' \
      --exec 'track_count("slow_requests")' \
      --metrics
```

Aggregating with `track_count()` keeps a running tally without printing each
event. Combine with `-qq` if you want the counter but not the event stream.
```

### Quiet Modes for Automation

Use graduated quiet levels to control output:

```bash
# Level 1 (-q): Suppress diagnostics, show events
> tail -f app.log | kelora -j -q --filter 'e.level == "ERROR"'

# Level 2 (-qq): Suppress events too, show only script output
> tail -f app.log | kelora -j -qq \
    --exec 'if e.level == "CRITICAL" { eprint("CRITICAL: " + e.message) }'

# Level 3 (-qqq): Complete silence except exit code
> tail -f app.log | kelora -j -qqq --filter 'e.level == "CRITICAL"'
```

**Quiet mode behavior:**

- `-q`: Suppress error summaries, stats, format detection messages
- `-qq`: Additionally suppress event output (automatically enables `-F none`)
- `-qqq`: Additionally suppress `print()` and `eprint()` from Rhai scripts

### Exit Codes for Automation

Use exit codes to detect processing issues:

```bash
# Check for errors (exit code 1 if parsing/runtime errors occurred)
> kelora -f json app.log --filter 'e.level == "ERROR"'
> echo "Exit code: $?"

# Alert based on exit code
> if tail -100 /var/log/app.log | kelora -j -qqq --filter 'e.level == "CRITICAL"'; then
    echo "No critical errors"
  else
    echo "Critical errors detected!" | mail -s "Alert" ops@company.com
  fi

# CI/CD pipeline integration
> kelora -qq -f json logs/*.json --filter 'e.level == "ERROR"' \
    && echo "✓ No errors" \
    || (echo "✗ Errors found" && exit 1)
```

!!! note
    Practicing locally? Swap `tail -f` for a short fixture like
    `examples/simple_logfmt.log` piped through `cat` so you can stop the command
    easily with Ctrl+C.

**Exit codes:**

- `0`: Success (no parsing or runtime errors)
- `1`: Parse errors or Rhai runtime errors occurred
- `2`: Invalid CLI usage or configuration errors
- `130`: Interrupted (Ctrl+C)
- `141`: Broken pipe (normal in Unix pipelines)

### Metrics with End-Stage Alerting

Collect metrics during processing and alert at the end:

```bash
# Alert if error rate exceeds threshold
> tail -100 /var/log/app.log | kelora -j -q \
    --exec 'track_count("total")' \
    --exec 'if e.level == "ERROR" { track_count("errors") }' \
    --metrics \
    --end '
      let total = metrics.get_path("total", 0);
      let errors = metrics.get_path("errors", 0);
      if total > 0 {
        let error_rate = errors.to_float() / total.to_float() * 100;
        if error_rate > 5.0 {
          eprint("HIGH ERROR RATE: " + error_rate + "%")
        }
      }
    '

# Alert on memory threshold breaches
> tail -f /var/log/app.log | kelora -j -qq \
    --exec 'if e.get_path("memory_percent", 0) > 90 { track_count("high_memory") }' \
    --metrics \
    --end 'if metrics.contains("high_memory") && metrics.high_memory > 10 { eprint("Memory warnings: " + metrics.high_memory) }'
```

### Sliding-Window Anomaly Detection

```bash
> kelora -f syslog examples/simple_syslog.log \
    --filter '"msg" in e && e.msg.contains("Failed login")' \
    --window 5 \
    --exec 'let hits = window_values("msg").filter(|m| m.contains("Failed login"));\
             if hits.len() >= 3 { e.alert = true; }' \
    --filter 'e.alert == true'
```

Use a short sliding window to spot bursts of suspicious activity and feed the
resulting events to downstream tooling (e.g., Slack webhook, SIEM pipeline).

### Write Alerts to File

Use `--allow-fs-writes` to persist alerts:

```bash
# Write critical events to alert file
> tail -f /var/log/app.log | kelora -j --allow-fs-writes \
    --filter 'e.level == "CRITICAL"' \
    --exec 'append_file("/var/log/alerts.log", e.timestamp + " " + e.service + " " + e.message)'

# JSON alert log
> tail -f /var/log/app.log | kelora -j --allow-fs-writes \
    --filter 'e.severity >= 8' \
    --exec 'append_file("/var/log/alerts.json", e.to_json())'

# Separate files by severity
> tail -f /var/log/app.log | kelora -j --allow-fs-writes \
    --exec 'if e.level == "ERROR" { append_file("/tmp/errors.log", e.to_json()) }' \
    --exec 'if e.level == "CRITICAL" { append_file("/tmp/critical.log", e.to_json()) }'
```

## Real-World Examples

### Web Server Error Monitor

```bash
> tail -f /var/log/nginx/access.log | kelora -f combined -qq \
    --filter 'e.status >= 500' \
    --exec 'track_count("5xx_errors")' \
    --exec 'eprint("5xx Error: " + e.status + " " + e.path + " from " + e.ip)' \
    --metrics \
    --end 'if metrics.get_path("5xx_errors", 0) > 10 { eprint("ALERT: " + metrics["5xx_errors"] + " server errors") }'
```

### Database Deadlock Detection

```bash
> tail -f /var/log/postgresql/postgresql.log | kelora -f line -qq \
    --filter 'e.line.contains("deadlock")' \
    --exec 'eprint("DEADLOCK DETECTED: " + e.line)' \
    --allow-fs-writes \
    --exec 'append_file("/var/log/deadlocks.log", now_utc().to_iso() + ": " + e.line)'
```

### Application Health Dashboard

```bash
> tail -f /var/log/app.log | kelora -j -q \
    --exec 'track_count(e.service + "_" + e.level)' \
    --exec 'if e.has_path("duration_ms") { track_avg(e.service + "_latency", e.duration_ms) }' \
    --metrics \
    --end '
      print("=== Service Health ===");
      for (key, value) in metrics {
        if key.contains("ERROR") && value > 5 {
          eprint("⚠️  " + key + ": " + value)
        } else {
          print("✓ " + key + ": " + value)
        }
      }
    '
```

### Failed Login Monitor

```bash
> tail -f /var/log/auth.log | kelora -f syslog -qq \
    --filter 'e.message.contains("Failed password")' \
    --exec 'let ip = e.message.extract_ip(); track_count(ip)' \
    --metrics \
    --end '
      for (ip, count) in metrics {
        if count > 3 {
          eprint("ALERT: " + count + " failed logins from " + ip)
        }
      }
    '
```

### Memory Leak Detection

```bash
> tail -f /var/log/app.log | kelora -j --window 100 -qq \
    --exec 'if e.has_path("memory_mb") {
      let recent_mem = window_numbers("memory_mb");
      if recent_mem.len() >= 10 {
        let trend = recent_mem[-1] - recent_mem[0];
        if trend > 100 {
          eprint("MEMORY LEAK: +" + trend + "MB over last " + recent_mem.len() + " events")
        }
      }
    }'
```

### API Rate Limit Violations

```bash
> tail -f /var/log/api.log | kelora -j -qq \
    --filter 'e.status == 429' \
    --exec 'track_count(e.client_id)' \
    --exec 'eprint("Rate limit hit: " + e.client_id + " on " + e.path)' \
    --metrics \
    --end '
      for (client, count) in metrics {
        if count > 10 {
          eprint("ALERT: Client " + client + " hit rate limits " + count + " times")
        }
      }
    '
```

### Deployment Error Spike Detection

```bash
> tail -f /var/log/app.log | kelora -j --window 50 -qq \
    --exec 'if e.level == "ERROR" {
      let recent_errors = window_values("level").filter(|l| l == "ERROR");
      let error_rate = recent_errors.len().to_float() / 50 * 100;
      if error_rate > 20 {
        eprint("ERROR SPIKE: " + error_rate + "% error rate - possible bad deployment")
      }
    }'
```

### Disk Space Warning

```bash
> tail -f /var/log/syslog | kelora -f syslog -qq \
    --filter 'e.message.contains("disk") && e.message.contains("full")' \
    --exec 'eprint("DISK ALERT: " + e.hostname + " - " + e.message)' \
    --allow-fs-writes \
    --exec 'append_file("/var/log/disk_alerts.log", e.to_json())'
```

### Security Event Aggregation

```bash
> tail -f /var/log/security.log | kelora -j -q \
    --exec 'if e.severity == "high" || e.severity == "critical" { track_count(e.event_type) }' \
    --exec 'if e.severity == "critical" { eprint("SECURITY: " + e.event_type + " from " + e.source_ip) }' \
    --metrics \
    --end '
      let total = 0;
      for (event, count) in metrics { total += count };
      if total > 0 {
        eprint("Security events in last batch: " + total)
      }
    '
```

## Integration Patterns

### Email Alerts

```bash
# Alert via email when critical events occur
> tail -f /var/log/app.log | kelora -j -qqq \
    --filter 'e.level == "CRITICAL"' \
    && echo "No critical events" \
    || echo "Critical events detected" | mail -s "Alert" ops@company.com
```

### Slack/Discord Webhooks

```bash
# Post to Slack when errors exceed threshold
> tail -100 /var/log/app.log | kelora -j -q \
    --exec 'track_count("total"); if e.level == "ERROR" { track_count("errors") }' \
    --metrics \
    --end '
      let errors = metrics.get_path("errors", 0);
      if errors > 10 {
        let msg = "High error count: " + errors;
        print(msg)
      }
    ' | while read msg; do
      curl -X POST -H 'Content-type: application/json' \
        --data '{"text":"'"$msg"'"}' \
        https://hooks.slack.com/services/YOUR/WEBHOOK/URL
    done
```

### PagerDuty Integration

```bash
#!/bin/bash
tail -f /var/log/app.log | kelora -j -qqq \
  --filter 'e.level == "CRITICAL"' \
  --allow-fs-writes \
  --exec 'append_file("/tmp/critical_event.json", e.to_json())'

# Check for new critical events every minute
while true; do
  if [ -s /tmp/critical_event.json ]; then
    curl -X POST https://events.pagerduty.com/v2/enqueue \
      -H 'Content-Type: application/json' \
      -d @/tmp/critical_event.json
    > /tmp/critical_event.json  # Truncate after sending
  fi
  sleep 60
done
```

### Prometheus Metrics Export

```bash
# Generate Prometheus metrics from streaming logs
> tail -f /var/log/app.log | kelora -j --allow-fs-writes -qq \
    --exec 'track_count("http_requests_total")' \
    --exec 'if e.status >= 500 { track_count("http_errors_total") }' \
    --exec 'if e.has_path("duration_ms") { track_avg("http_duration_ms", e.duration_ms) }' \
    --metrics-file /var/lib/prometheus/kelora_metrics.json
```

### Systemd Service

```ini
# /etc/systemd/system/kelora-monitor.service
[Unit]
Description=Kelora Log Monitor
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/tail -f /var/log/app.log | /usr/local/bin/kelora -j -qq \
  --filter 'e.level == "CRITICAL"' \
  --exec 'eprint("CRITICAL: " + e.message)'
Restart=always
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-tier.target
```

### Cron-Based Periodic Alerts

```bash
# Check last 5 minutes of logs every 5 minutes
# Add to crontab: */5 * * * * /path/to/alert_script.sh

#!/bin/bash
kelora -f json /var/log/app.log \
  --since "5 minutes ago" \
  --levels error,critical \
  -q \
  --exec 'track_count("errors")' \
  --metrics \
  --end '
    let errors = metrics.get_path("errors", 0);
    if errors > 10 {
      eprint("Last 5min: " + errors + " errors")
    }
  ' 2>&1 | grep -q "errors" && {
    echo "High error rate detected" | mail -s "Alert" ops@company.com
  }
```

## Performance and Reliability

### Batch Timeout for Low-Latency

```bash
# Reduce latency in parallel mode with batch timeout
> tail -f /var/log/app.log | kelora -j --parallel \
    --batch-size 100 \
    --batch-timeout 50 \
    --filter 'e.level == "ERROR"'
```

### Handle Log Rotation

```bash
# Use tail -F to follow through log rotation
> tail -F /var/log/app.log | kelora -j --filter 'e.level == "ERROR"'

# Monitor with logrotate awareness
> tail -F /var/log/app.log /var/log/app.log.1 | kelora -j -l error
```

### Reliability with Restart

```bash
# Auto-restart monitoring on failure
> while true; do
    tail -f /var/log/app.log | kelora -j -qq \
      --filter 'e.level == "CRITICAL"' \
      --exec 'eprint("CRITICAL: " + e.message)'
    sleep 5
  done
```

### Buffer Management

```bash
# Prevent memory bloat with take limit on window functions
> tail -f /var/log/app.log | kelora -j --window 100 \
    --exec 'if window_values("level").len() == 100 { eprint("Window full, analyzing...") }'
```

## Tips

**Real-Time Monitoring:**

- Use `tail -f` for active logs, `tail -F` for logs that rotate
- Use `-qq` to suppress event output, showing only alerts
- Use `-qqq` for complete silence when only exit codes matter
- Combine with `--metrics` and `--end` for batch summaries

**Alert Design:**

- Use `eprint()` for alerts (goes to stderr, separate from events)
- Write structured alerts with context: timestamp, service, severity
- Consider alert fatigue - use thresholds and deduplication
- Use `track_unique()` to count distinct values (IPs, users, etc.)

**Performance:**

- Use `--parallel` for high-throughput log streams
- Adjust `--batch-timeout` for latency vs throughput balance
- Lower batch timeout = lower latency, higher CPU usage
- Use `--window` carefully - large windows consume memory

**Automation:**
```bash
# Test alerts without actual monitoring
> echo '{"level":"CRITICAL","message":"test"}' | kelora -j -qq \
    --exec 'if e.level == "CRITICAL" { eprint("Alert triggered") }'

# Validate alert logic with recent logs
> tail -100 /var/log/app.log | kelora -j -q \
    --exec 'if e.level == "CRITICAL" { eprint(e.message) }'
```

**Exit Code Patterns:**
```bash
# Alert only if processing errors occurred (not just filtered events)
kelora -qqq -f json input.log || send_alert "Processing errors detected"

# Combine filtering with exit code checking
kelora -qq -l error input.log && echo "Clean" || echo "Has errors"
```

## Troubleshooting

**Alerts not triggering:**
```bash
# Test filter logic
> echo '{"level":"ERROR","message":"test"}' | kelora -j \
    --filter 'e.level == "ERROR"'

# Verify eprint output
> echo '{"level":"ERROR"}' | kelora -j -qq \
    --exec 'eprint("Test alert: " + e.level)'
```

**High memory usage:**
```bash
# Reduce window size
> tail -f app.log | kelora -j --window 50  # Instead of 1000

# Disable metrics if not needed
> tail -f app.log | kelora -j --filter 'e.level == "ERROR"'  # No --metrics
```

**Missing events:**
```bash
# Check if logs are being written
> tail -f /var/log/app.log | kelora -j --stats

# Verify parsing
> tail -10 /var/log/app.log | kelora -j
```

**Tail behavior:**
```bash
# Tail stops after log rotation - use -F
> tail -F /var/log/app.log | kelora -j  # Follows through rotation

# Start from current position (no history)
> tail -f -n 0 /var/log/app.log | kelora -j  # Only new lines
```

## See Also

- [Monitor Application Health](monitor-application-health.md) - Health metrics and monitoring patterns
- [Find Errors in Logs](find-errors-in-logs.md) - Error detection techniques
- [Function Reference](../reference/functions.md) - Complete function list
- [CLI Reference](../reference/cli-reference.md) - All command-line options
- [Exit Codes Reference](../reference/exit-codes.md) - Exit code documentation
