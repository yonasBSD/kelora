# Batch Process Archives

Process large log archives efficiently using parallel processing, batch tuning, and performance optimization techniques.

## Problem

You have large log archives (compressed or uncompressed) that need processing. Sequential processing is too slow, but you want to maximize throughput while managing memory usage and maintaining reasonable ordering.

## Solutions

### Basic Parallel Processing

Enable parallel processing for faster throughput:

```bash
# Basic parallel mode (auto-detects CPU cores)
> kelora -f json large-logs.json --parallel \
    --filter 'e.level == "ERROR"'

# Specify thread count explicitly
> kelora -f json logs/*.json --parallel --threads 8 \
    --filter 'e.status >= 500'

# Parallel with metrics
> kelora -f json archive.json --parallel \
    --exec 'track_count(e.service)' \
    --metrics
```

**Parallel processing:**

- Automatically uses available CPU cores (`--threads 0`)
- Higher throughput than sequential mode
- May reorder output (use `--unordered` for maximum speed)
- Higher memory usage due to batching

### Process Compressed Archives

Kelora automatically handles gzip compression:

```bash
# Single compressed file
> kelora -f json logs/app.log.gz --parallel \
    --filter 'e.level == "ERROR"'

# Multiple compressed files
> kelora -f json logs/*.log.gz --parallel \
    --exec 'track_count(e.level)' \
    --metrics

# Mixed compressed and uncompressed
> kelora -f json logs/*.log logs/*.log.gz --parallel
```

### Batch Size Tuning

Adjust batch size for memory vs throughput tradeoffs:

```bash
# Large batches = higher throughput, more memory
> kelora -f json large.log --parallel --batch-size 5000 \
    --filter 'e.level == "ERROR"'

# Small batches = lower memory, more overhead
> kelora -f json large.log --parallel --batch-size 500 \
    --filter 'e.level == "ERROR"'

# Default batch size (1000) - good balance
> kelora -f json large.log --parallel

# Very large files with memory constraints
> kelora -f json huge.log --parallel --batch-size 100
```

**Batch size guidelines:**

- Default: 1000 (good balance)
- High memory available: 5000-10000
- Memory constrained: 100-500
- Complex transformations: Lower batch size
- Simple filters: Higher batch size

### Unordered Output for Maximum Speed

Disable output ordering for best performance:

```bash
# Unordered output (fastest)
> kelora -f json logs/*.json --parallel --unordered \
    --filter 'e.level == "ERROR"'

# With metrics (order doesn't matter)
> kelora -f json archive.json --parallel --unordered \
    --exec 'track_count(e.status)' \
    --metrics

# Stats only (no event output, order irrelevant)
> kelora -f json logs/*.json --parallel --unordered \
    --stats-only
```

**When to use `--unordered`:**

- Processing for metrics/stats only
- Order doesn't matter for analysis
- Maximum throughput is priority
- Large-scale batch processing

### File Processing Order

Control which order files are processed:

```bash
# CLI order (default) - as specified on command line
> kelora -f json file1.log file2.log file3.log

# Name order - alphabetically by filename
> kelora -f json logs/*.json --file-order name

# Modification time - oldest first
> kelora -f json logs/*.json --file-order mtime

# Process newest logs first (reverse mtime)
> kelora -f json logs/*.json --file-order mtime --parallel
```

**File order options:**

- `cli`: Command-line order (default)
- `name`: Alphabetical by filename
- `mtime`: Modification time (oldest first)

### Multiple Archive Processing

Process many archives efficiently:

```bash
# Process all archives in directory
> kelora -f json /var/log/archives/*.json.gz --parallel \
    --exec 'track_count(e.level)' \
    --metrics

# Process with wildcard patterns
> kelora -f json logs/2024-*.log.gz --parallel --unordered \
    --filter 'e.status >= 400'

# Recursive with find
> find /var/log -name "*.log.gz" -type f -print0 | \
    xargs -0 kelora -f json --parallel --unordered \
    --exec 'track_count(e.service)' \
    --metrics
```

## Real-World Examples

### Daily Archive Analysis

```bash
# Analyze yesterday's logs
> kelora -f json /var/log/app/app-$(date -d yesterday +%Y-%m-%d).log.gz \
    --parallel \
    --exec 'track_count(e.level)' \
    --exec 'track_count(e.service)' \
    --exec 'if e.level == "ERROR" { track_count("errors_by_service_" + e.service) }' \
    --metrics \
    > daily_report_$(date -d yesterday +%Y-%m-%d).txt
```

### Monthly Archive Processing

```bash
# Process entire month of logs
> kelora -f json /var/log/archives/2024-01-*.log.gz \
    --parallel --threads 16 --unordered \
    --exec 'track_count(e.level)' \
    --exec 'track_unique("active_users", e.user_id)' \
    --exec 'if e.has_path("duration_ms") { track_avg("avg_latency", e.duration_ms) }' \
    --metrics \
    > monthly_report_2024-01.txt
```

### Error Analysis Across Archives

```bash
# Find all errors in last week of archives
> kelora -f json /var/log/archives/2024-01-{15..21}-*.log.gz \
    --parallel --batch-size 2000 \
    --filter 'e.level == "ERROR"' \
    --exec 'e.error_type = e.get_path("error.type", "unknown")' \
    --exec 'track_count(e.error_type)' \
    --keys timestamp,service,error_type,message \
    --metrics \
    -F json > errors_week_03.json
```

### Performance Audit

```bash
# Extract slow requests from archives
> kelora -f combined /var/log/nginx/access.log.*.gz \
    --parallel --unordered --threads 12 \
    --filter 'e.get_path("request_time", "0").to_float() > 1.0' \
    --exec 'e.latency = e.get_path("request_time", "0").to_float()' \
    --exec 'track_bucket("latency_buckets", floor(e.latency))' \
    --exec 'track_avg("avg_latency", e.latency)' \
    --keys timestamp,ip,path,request_time,status \
    --metrics
```

### Security Audit Across Archives

```bash
# Find suspicious activity in multiple archives
> kelora -f json /var/log/security/*.log.gz \
    --parallel --batch-size 5000 \
    --filter 'e.severity == "high" || e.severity == "critical"' \
    --exec 'track_count(e.event_type)' \
    --exec 'track_unique("affected_ips", e.source_ip)' \
    --exec 'e.hour = e.timestamp.format("%Y-%m-%d %H:00")' \
    --exec 'track_count(e.hour)' \
    --metrics \
    -F json > security_audit.json
```

### Database Query Analysis

```bash
# Analyze slow queries from database archives
> kelora -f json /var/log/postgres/postgres-*.log.gz \
    --parallel --unordered \
    --filter 'e.get_path("duration_ms", 0) > 1000' \
    --exec 'e.query_hash = e.query.hash("xxh3")' \
    --exec 'e.table = e.query.extract_re(r"FROM\\s+(\\w+)", 1)' \
    --exec 'track_count(e.table)' \
    --exec 'track_avg(e.table + "_latency", e.duration_ms)' \
    --keys timestamp,user,table,duration_ms,query_hash \
    --metrics
```

### User Activity Aggregation

```bash
# Aggregate user activity from month of archives
> kelora -f json /var/log/app/2024-01-*.log.gz \
    --parallel --threads 16 --batch-size 5000 --unordered \
    --filter 'e.has_path("user_id")' \
    --exec 'track_unique("daily_active_users", e.user_id)' \
    --exec 'track_count(e.action)' \
    --exec 'if e.action == "purchase" { track_sum("revenue", e.get_path("amount", 0)) }' \
    --metrics
```

### Multi-Year Archive Search

```bash
# Search for specific pattern across years
> kelora -f json /archives/app-{2022,2023,2024}-*.log.gz \
    --parallel --threads 24 --unordered \
    --filter 'e.message.contains("memory leak") || e.message.contains("out of memory")' \
    --exec 'e.year = e.timestamp.format("%Y")' \
    --exec 'track_count(e.year)' \
    --keys timestamp,service,level,message \
    --metrics
```

### Access Log Consolidation

```bash
# Consolidate web access logs
> kelora -f combined /var/log/nginx/access.log.*.gz \
    --parallel --batch-size 10000 --unordered \
    --exec 'track_count(e.status)' \
    --exec 'track_count(e.method)' \
    --exec 'track_unique("unique_ips", e.ip)' \
    --exec 'if e.has_path("bytes") { track_sum("total_bytes", e.get_path("bytes", "0").to_int()) }' \
    --metrics \
    --metrics-file nginx_consolidated_metrics.json
```

### Time-Range Archive Processing

```bash
# Process archives within specific time range
> kelora -f json /var/log/archives/*.log.gz \
    --parallel --batch-size 2000 \
    --since "2024-01-15 00:00:00" \
    --until "2024-01-20 23:59:59" \
    --filter 'e.level == "ERROR"' \
    --exec 'track_count(e.service)' \
    --metrics
```

## Performance Optimization

### CPU-Bound Workloads

```bash
# Max out CPU utilization
> kelora -f json huge.log \
    --parallel --threads 0 \
    --batch-size 5000 \
    --unordered \
    --exec 'track_count(e.level)'
```

### Memory-Constrained Environments

```bash
# Minimize memory usage
> kelora -f json huge.log \
    --parallel --threads 4 \
    --batch-size 200 \
    --filter 'e.level == "ERROR"'

# Sequential processing (lowest memory)
> kelora -f json huge.log \
    --filter 'e.level == "ERROR"'
```

### I/O-Bound Workloads

```bash
# More threads than cores for I/O-heavy tasks
> kelora -f json /nfs/logs/*.json.gz \
    --parallel --threads 32 \
    --batch-size 1000 \
    --unordered
```

### Balanced Processing

```bash
# Good default for most workloads
> kelora -f json logs/*.json.gz \
    --parallel \
    --batch-size 1000 \
    --exec 'track_count(e.service)' \
    --metrics
```

## Monitoring and Validation

### Processing Statistics

```bash
# Show detailed processing stats
> kelora -f json large.log.gz --parallel --stats \
    --filter 'e.level == "ERROR"'

# Stats only (no event output)
> kelora -f json logs/*.json.gz --parallel --unordered \
    --stats-only

# Combine with metrics
> kelora -f json archive.log.gz --parallel \
    --exec 'track_count(e.level)' \
    --stats --metrics
```

### Validate Processing

```bash
# Count events in archive
> kelora -f json archive.log.gz --parallel -qq \
    --exec 'track_count("total")' \
    --metrics

# Verify no parsing errors (exit code 0)
> kelora -f json archive.log.gz --parallel -qqq && echo "✓ Clean" || echo "✗ Errors"

# Sample output for verification
> kelora -f json archive.log.gz --parallel --take 100 | less
```

### Performance Benchmarking

```bash
# Benchmark sequential vs parallel
> time kelora -f json large.log --filter 'e.level == "ERROR"' > /dev/null
> time kelora -f json large.log --parallel --filter 'e.level == "ERROR"' > /dev/null

# Benchmark batch sizes
> for size in 100 1000 5000 10000; do
    echo "Batch size: $size"
    time kelora -f json large.log --parallel --batch-size $size -qq
  done

# Compare thread counts
> for threads in 2 4 8 16; do
    echo "Threads: $threads"
    time kelora -f json large.log --parallel --threads $threads -qq
  done
```

## Tips

**Performance:**

- Use `--parallel` for files > 100MB
- Use `--unordered` when order doesn't matter (20-30% faster)
- Increase `--batch-size` for simple operations (5000-10000)
- Decrease `--batch-size` for complex transformations (200-500)
- Use `--threads 0` to auto-detect CPU cores
- I/O-bound tasks benefit from more threads than cores

**Memory Management:**

- Default batch size (1000) uses ~10-50MB per thread
- Large batch sizes can use significant memory with many threads
- Reduce batch size if you see OOM errors
- Sequential mode uses minimal memory (single-threaded)
- Window functions increase memory proportionally

**File Processing:**

- `.gz` files are automatically decompressed
- Use `--file-order mtime` to process chronologically
- Use `--file-order name` for predictable ordering
- Combine with shell globbing for flexible file selection

**Automation:**
```bash
# Scheduled archive processing
0 2 * * * kelora -f json /var/log/archives/$(date -d yesterday +\%Y-\%m-\%d)*.gz \
  --parallel --unordered --metrics > /reports/daily_$(date -d yesterday +\%Y-\%m-\%d).txt

# Parallel archive creation
find /var/log -name "*.log" -mtime +7 | \
  parallel "gzip {}" "kelora -f json {}.gz --parallel -qq --stats"
```

**Export:**
```bash
# Export to CSV for analysis
> kelora -f json logs/*.gz --parallel --unordered \
    --keys timestamp,level,service,message \
    -F csv > consolidated.csv

# Export to JSON
> kelora -f json logs/*.gz --parallel \
    --filter 'e.level == "ERROR"' \
    -F json > errors.json

# Persist metrics
> kelora -f json logs/*.gz --parallel --unordered \
    --exec 'track_count(e.service)' \
    --metrics --metrics-file daily_metrics.json
```

## Troubleshooting

**Out of memory errors:**
```bash
# Reduce batch size
> kelora -f json huge.log --parallel --batch-size 200

# Reduce thread count
> kelora -f json huge.log --parallel --threads 2

# Use sequential mode
> kelora -f json huge.log
```

**Slow processing:**
```bash
# Enable parallel mode
> kelora -f json large.log --parallel

# Increase batch size
> kelora -f json large.log --parallel --batch-size 5000

# Enable unordered output
> kelora -f json large.log --parallel --unordered

# Increase threads for I/O-bound
> kelora -f json /nfs/logs/*.gz --parallel --threads 32
```

**Inconsistent output:**
```bash
# Use ordered output (default in parallel mode)
> kelora -f json logs/*.json --parallel

# Or disable parallel if order is critical
> kelora -f json logs/*.json
```

**Missing events:**
```bash
# Check for parsing errors
> kelora -f json archive.log.gz --parallel --verbose

# Validate with stats
> kelora -f json archive.log.gz --parallel --stats

# Sample to verify
> kelora -f json archive.log.gz --parallel --take 1000
```

**Progress monitoring:**
```bash
# Use pv (pipe viewer) for progress
> pv large.log.gz | gunzip | kelora -f json --parallel

# Process with stats
> kelora -f json large.log.gz --parallel --stats
```

## See Also

- [Performance Model](../concepts/performance-model.md) - Optimization guide
- [Build Streaming Alerts](build-streaming-alerts.md) - Real-time processing
- [CLI Reference](../reference/cli-reference.md) - All command-line options
- [Execution Modes](../concepts/performance-model.md#execution-modes) - How parallel mode works
