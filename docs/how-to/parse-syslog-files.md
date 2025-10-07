# Parse Syslog Files

Parse and analyze syslog format files from system logs, application logs, and network devices.

## Problem

You need to parse syslog-formatted logs (RFC 3164/RFC 5424) to extract facility, severity, hostname, process information, and messages for monitoring, troubleshooting, or security analysis.

## Solutions

### Basic Syslog Parsing

Parse standard syslog format:

```bash
# Auto-detect and parse syslog
kelora -f syslog /var/log/syslog --take 5

# Explicit syslog format
kelora -f syslog examples/simple_syslog.log
```

Syslog format includes:
- `priority` - Combined facility/severity number
- `facility` - Facility code (0-23)
- `severity` - Severity level (0-7)
- `timestamp` - Event timestamp
- `hostname` - Source hostname
- `process` - Process name
- `pid` - Process ID
- `message` - Log message

### Filter by Severity

Filter logs by severity level (0=emerg, 7=debug):

```bash
# Critical and below (0-2: emerg, alert, crit)
kelora -f syslog /var/log/syslog --filter 'e.severity <= 2'

# Errors only (severity 3)
kelora -f syslog /var/log/syslog --filter 'e.severity == 3'

# Warning and above (0-4)
kelora -f syslog /var/log/syslog --filter 'e.severity <= 4'
```

Severity levels:
- 0: Emergency (system unusable)
- 1: Alert (immediate action required)
- 2: Critical
- 3: Error
- 4: Warning
- 5: Notice
- 6: Informational
- 7: Debug

### Filter by Facility

Filter by facility type:

```bash
# Kernel messages (facility 0)
kelora -f syslog /var/log/syslog --filter 'e.facility == 0'

# Auth/security messages (facility 4 or 10)
kelora -f syslog /var/log/auth.log --filter 'e.facility == 4 || e.facility == 10'

# System daemons (facility 3)
kelora -f syslog /var/log/syslog --filter 'e.facility == 3'
```

Common facilities:
- 0: Kernel
- 1: User-level
- 2: Mail
- 3: System daemons
- 4: Security/auth
- 10: Security/auth (private)

### Filter by Process

Track specific services or processes:

```bash
# Specific process name
kelora -f syslog /var/log/syslog --filter 'e.process == "sshd"'

# Multiple processes
kelora -f syslog /var/log/syslog \
  --filter 'e.process == "sshd" || e.process == "sudo"'

# Process name pattern
kelora -f syslog /var/log/syslog \
  --filter 'e.process.contains("systemd")'
```

### Monitor Authentication

Track authentication events:

```bash
# Failed SSH logins
kelora -f syslog /var/log/auth.log \
  --filter 'e.process == "sshd" && e.message.contains("Failed password")'

# Sudo usage
kelora -f syslog /var/log/auth.log \
  --filter 'e.process == "sudo"' \
  --keys timestamp,hostname,message

# Track unique users attempting auth
kelora -f syslog /var/log/auth.log \
  --filter 'e.message.contains("Failed")' \
  --exec 'e.user = e.message.extract_re(r"for ([^ ]+)", 1)' \
  --exec 'track_unique("failed_users", e.user)' \
  --metrics
```

### Extract Message Details

Parse structured information from messages:

```bash
# Extract IP addresses from messages
kelora -f syslog /var/log/syslog \
  --exec 'e.ip = e.message.extract_ip()' \
  --filter 'e.ip != ""'

# Extract error codes
kelora -f syslog /var/log/syslog \
  --exec 'e.error_code = e.message.extract_re(r"error[: ](\d+)", 1)' \
  --filter 'e.error_code != ""'

# Parse key-value pairs in message
kelora -f syslog /var/log/syslog \
  --exec 'e.details = e.message.parse_kv(" ", "=")' \
  --exec 'e.status = e.get_path("details.status", "")'
```

### Aggregate by Hostname

Track activity across multiple hosts:

```bash
# Count messages per host
kelora -f syslog /var/log/syslog \
  --exec 'track_count(e.hostname)' \
  --metrics

# Track errors per host
kelora -f syslog /var/log/syslog \
  --filter 'e.severity <= 3' \
  --exec 'track_count(e.hostname)' \
  --metrics

# Find most active hosts
kelora -f syslog /var/log/syslog \
  --exec 'track_count(e.hostname)' \
  --exec 'track_unique("processes", e.hostname + ":" + e.process)' \
  --metrics
```

### Time-Based Analysis

Filter and analyze by time:

```bash
# Last hour's errors
kelora -f syslog /var/log/syslog \
  --since "1 hour ago" \
  --filter 'e.severity <= 3'

# Events in specific time range
kelora -f syslog /var/log/syslog \
  --since "2024-01-15 09:00" \
  --until "2024-01-15 17:00"

# Group errors by hour
kelora -f syslog /var/log/syslog \
  --filter 'e.severity <= 3' \
  --exec 'e.hour = to_datetime(e.timestamp).format("%Y-%m-%d %H:00")' \
  --exec 'track_count(e.hour)' \
  --metrics
```

### Convert to JSON

Export syslog to JSON for further processing:

```bash
# Convert to JSON
kelora -f syslog /var/log/syslog -F json > syslog.json

# Convert with selected fields
kelora -f syslog /var/log/syslog \
  --keys timestamp,hostname,process,severity,message \
  -F json > syslog.json

# Add enrichment before export
kelora -f syslog /var/log/syslog \
  --exec 'e.severity_name = switch e.severity {
    0 => "EMERG", 1 => "ALERT", 2 => "CRIT",
    3 => "ERROR", 4 => "WARN", 5 => "NOTICE",
    6 => "INFO", _ => "DEBUG"
  }' \
  -F json
```

## Real-World Examples

### Security Monitoring

```bash
# Monitor failed SSH attempts from external IPs
kelora -f syslog /var/log/auth.log \
  --filter 'e.process == "sshd" && e.message.contains("Failed")' \
  --exec 'e.ip = e.message.extract_ip()' \
  --exec 'e.external = !e.ip.is_private_ip()' \
  --filter 'e.external' \
  --exec 'track_count(e.ip)' \
  --keys timestamp,ip,message --metrics
```

### Service Health Check

```bash
# Track service starts/stops
kelora -f syslog /var/log/syslog \
  --filter 'e.message.contains("Started") || e.message.contains("Stopped")' \
  --exec 'e.action = if e.message.contains("Started") { "start" } else { "stop" }' \
  --exec 'track_count(e.process + ":" + e.action)' \
  --metrics
```

### Disk Space Warnings

```bash
# Track disk space warnings
kelora -f syslog /var/log/syslog \
  --filter 'e.message.contains("disk") && e.message.contains("full")' \
  --exec 'e.disk = e.message.extract_re(r"(/[^ ]+)", 1)' \
  --keys timestamp,hostname,disk,message
```

### Network Device Logs

```bash
# Parse router/switch logs
kelora -f syslog network.log \
  --filter 'e.facility == 16' \
  --exec 'e.interface = e.message.extract_re(r"interface ([^ ]+)", 1)' \
  --exec 'track_count(e.interface)' \
  --metrics
```

## Tips

**Severity Filtering:**
- Use numeric comparison for severity ranges
- Lower numbers = higher severity (0 is most critical)
- Filter `<= 3` for error-level and above
- Filter `>= 6` for debug/info only

**Facility Codes:**
- Different systems use different facilities
- Check your syslog.conf for facility mappings
- Security logs often use facility 4 or 10
- Custom applications typically use 16-23

**Performance:**
- Add `--parallel` for large syslog files
- Use `--since`/`--until` to reduce processing
- Filter by severity early in pipeline

**Message Parsing:**
- Message format varies by application
- Use `extract_re()` for pattern extraction
- Use `parse_kv()` for structured messages
- Consider `--filter` before expensive parsing

**Hostname Handling:**
- Hostname may be IP or FQDN
- Normalize with `.to_lower()` for consistency
- Use `extract_domain()` for FQDN analysis

## See Also

- [Find Errors in Logs](find-errors-in-logs.md) - General error filtering patterns
- [Build Streaming Alerts](build-streaming-alerts.md) - Real-time syslog monitoring
- [Monitor Application Health](monitor-application-health.md) - Service monitoring patterns
- [Function Reference](../reference/functions.md) - String extraction functions
