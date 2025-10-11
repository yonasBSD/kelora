# Batch Process Archives

Process large log archives efficiently using parallel processing, batch tuning, and performance optimization techniques.

## Problem

You have large log archives (compressed or uncompressed) that need processing. Sequential processing is too slow, but you want to maximize throughput while managing memory usage and maintaining reasonable ordering.

## Solutions

### Basic Parallel Processing

Enable parallel processing for faster throughput:

```bash
# Basic parallel mode (auto-detects CPU cores)
> kelora -j large-logs.json --parallel \
    --filter 'e.level == "ERROR"'

# Specify thread count explicitly
> kelora -j logs/*.json --parallel --threads 8 \
    --filter 'e.status >= 500'

# Parallel with metrics
> kelora -j archive.json --parallel \
    -e 'track_count(e.service)' \
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
> kelora -j logs/app.log.gz --parallel \
    --filter 'e.level == "ERROR"'

# Multiple compressed files
> kelora -j logs/*.log.gz --parallel \
    -e 'track_count(e.level)' \
    --metrics

# Mixed compressed and uncompressed
> kelora -j logs/*.log logs/*.log.gz --parallel
```

### Batch Size Tuning

Adjust batch size for memory vs throughput tradeoffs:

```bash
# Large batches = higher throughput, more memory
> kelora -j large.log --parallel --batch-size 5000 \
    --filter 'e.level == "ERROR"'

# Small batches = lower memory, more overhead
> kelora -j large.log --parallel --batch-size 500 \
    --filter 'e.level == "ERROR"'

# Default batch size (1000) - good balance
> kelora -j large.log --parallel

# Very large files with memory constraints
> kelora -j huge.log --parallel --batch-size 100
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
> kelora -j logs/*.json --parallel --unordered \
    --filter 'e.level == "ERROR"'

# With metrics (order doesn't matter)
> kelora -j archive.json --parallel --unordered \
    -e 'track_count(e.status)' \
    --metrics

# Stats only (no event output, order irrelevant)
> kelora -j logs/*.json --parallel --unordered \
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
> kelora -j file1.log file2.log file3.log

# Name order - alphabetically by filename
> kelora -j logs/*.json --file-order name

# Modification time - oldest first
> kelora -j logs/*.json --file-order mtime

# Process newest logs first (reverse mtime)
> kelora -j logs/*.json --file-order mtime --parallel
```

**File order options:**

- `cli`: Command-line order (default)
- `name`: Alphabetical by filename
- `mtime`: Modification time (oldest first)

### Multiple Archive Processing

Process many archives efficiently:

```bash
# Process all archives in directory
> kelora -j /var/log/archives/*.json.gz --parallel \
    -e 'track_count(e.level)' \
    --metrics

# Process with wildcard patterns
> kelora -j logs/2024-*.log.gz --parallel --unordered \
    --filter 'e.status >= 400'

# Recursive with find
> find /var/log -name "*.log.gz" -type f -print0 | \
    xargs -0 kelora -j --parallel --unordered \
    -e 'track_count(e.service)' \
    --metrics
```

## Real-World Examples

### Daily Archive Analysis

```bash
# Analyze yesterday's logs
> kelora -j /var/log/app/app-$(date -d yesterday +%Y-%m-%d).log.gz \
    --parallel \
    -e 'track_count(e.level)' \
    -e 'track_count(e.service)' \
    -e 'if e.level == "ERROR" { track_count("errors_by_service_" + e.service) }' \
    -m \
    > daily_report_$(date -d yesterday +%Y-%m-%d).txt
```

### Monthly Archive Processing

```bash
# Process entire month of logs
> kelora -j /var/log/archives/2024-01-*.log.gz \
    --parallel --threads 16 --unordered \
    -e 'track_count(e.level)' \
    -e 'track_unique("active_users", e.user_id)' \
    -e 'if e.has_path("duration_ms") { track_avg("avg_latency", e.duration_ms) }' \
    -m \
    > monthly_report_2024-01.txt
```

### Error Analysis Across Archives

```bash
# Find all errors in last week of archives
> kelora -j /var/log/archives/2024-01-{15..21}-*.log.gz \
    --parallel --batch-size 2000 \
    --filter 'e.level == "ERROR"' \
    -e 'e.error_type = e.get_path("error.type", "unknown")' \
    -e 'track_count(e.error_type)' \
    -k timestamp,service,error_type,message \
    -m \
    -J > errors_week_03.json
```

### Performance Audit

```bash
# Extract slow requests from archives
> kelora -f combined /var/log/nginx/access.log.*.gz \
    --parallel --unordered --threads 12 \
    --filter 'e.get_path("request_time", "0").to_float() > 1.0' \
    -e 'e.latency = e.get_path("request_time", "0").to_float()' \
    -e 'track_bucket("latency_buckets", floor(e.latency))' \
    -e 'track_avg("avg_latency", e.latency)' \
    -k timestamp,ip,path,request_time,status \
    --metrics
```

### Security Audit Across Archives

```bash
# Find suspicious activity in multiple archives
> kelora -j /var/log/security/*.log.gz \
    --parallel --batch-size 5000 \
    --filter 'e.severity == "high" || e.severity == "critical"' \
    -e 'track_count(e.event_type)' \
    -e 'track_unique("affected_ips", e.source_ip)' \
    -e 'e.hour = e.timestamp.format("%Y-%m-%d %H:00")' \
    -e 'track_count(e.hour)' \
    -m \
    -J > security_audit.json
```

### Database Query Analysis

```bash
# Analyze slow queries from database archives
> kelora -j /var/log/postgres/postgres-*.log.gz \
    --parallel --unordered \
    --filter 'e.get_path("duration_ms", 0) > 1000' \
    -e 'e.query_hash = e.query.hash("xxh3")' \
    -e 'e.table = e.query.extract_re(r"FROM\\s+(\\w+)", 1)' \
    -e 'track_count(e.table)' \
    -e 'track_avg(e.table + "_latency", e.duration_ms)' \
    -k timestamp,user,table,duration_ms,query_hash \
    --metrics
```

### User Activity Aggregation

```bash
# Aggregate user activity from month of archives
> kelora -j /var/log/app/2024-01-*.log.gz \
    --parallel --threads 16 --batch-size 5000 --unordered \
    --filter 'e.has_path("user_id")' \
    -e 'track_unique("daily_active_users", e.user_id)' \
    -e 'track_count(e.action)' \
    -e 'if e.action == "purchase" { track_sum("revenue", e.get_path("amount", 0)) }' \
    --metrics
```

### Multi-Year Archive Search

```bash
# Search for specific pattern across years
> kelora -j /archives/app-{2022,2023,2024}-*.log.gz \
    --parallel --threads 24 --unordered \
    --filter 'e.message.contains("memory leak") || e.message.contains("out of memory")' \
    -e 'e.year = e.timestamp.format("%Y")' \
    -e 'track_count(e.year)' \
    -k timestamp,service,level,message \
    --metrics
```

### Access Log Consolidation

```bash
# Consolidate web access logs
> kelora -f combined /var/log/nginx/access.log.*.gz \
    --parallel --batch-size 10000 --unordered \
    -e 'track_count(e.status)' \
    -e 'track_count(e.method)' \
    -e 'track_unique("unique_ips", e.ip)' \
    -e 'if e.has_path("bytes") { track_sum("total_bytes", e.get_path("bytes", "0").to_int()) }' \
    -m \
    --metrics-file nginx_consolidated_metrics.json
```

### Time-Range Archive Processing

```bash
# Process archives within specific time range
> kelora -j /var/log/archives/*.log.gz \
    --parallel --batch-size 2000 \
    --since "2024-01-15 00:00:00" \
    --until "2024-01-20 23:59:59" \
    --filter 'e.level == "ERROR"' \
    -e 'track_count(e.service)' \
    --metrics
```

## Performance Optimization

### CPU-Bound Workloads

```bash
# Max out CPU utilization
> kelora -j huge.log \
    --parallel --threads 0 \
    --batch-size 5000 \
    --unordered \
    -e 'track_count(e.level)'
```

### Memory-Constrained Environments

```bash
# Minimize memory usage
> kelora -j huge.log \
    --parallel --threads 4 \
    --batch-size 200 \
    --filter 'e.level == "ERROR"'

# Sequential processing (lowest memory)
> kelora -j huge.log \
    --filter 'e.level == "ERROR"'
```

### I/O-Bound Workloads

```bash
# More threads than cores for I/O-heavy tasks
> kelora -j /nfs/logs/*.json.gz \
    --parallel --threads 32 \
    --batch-size 1000 \
    --unordered
```

### Balanced Processing

```bash
# Good default for most workloads
> kelora -j logs/*.json.gz \
    --parallel \
    --batch-size 1000 \
    -e 'track_count(e.service)' \
    --metrics
```

## Monitoring and Validation

### Processing Statistics

```bash
# Show detailed processing stats
> kelora -j large.log.gz --parallel -s \
    --filter 'e.level == "ERROR"'

# Stats only (no event output)
> kelora -j logs/*.json.gz --parallel --unordered \
    --stats-only

# Combine with metrics
> kelora -j archive.log.gz --parallel \
    -e 'track_count(e.level)' \
    -s --metrics
```

### Validate Processing

```bash
# Count events in archive
> kelora -j archive.log.gz --parallel -qq \
    -e 'track_count("total")' \
    --metrics

# Verify no parsing errors (exit code 0)
> kelora -j archive.log.gz --parallel -qqq && echo "✓ Clean" || echo "✗ Errors"

# Sample output for verification
> kelora -j archive.log.gz --parallel -n 100 | less
```

### Performance Benchmarking

```bash
# Benchmark sequential vs parallel
> time kelora -j large.log --filter 'e.level == "ERROR"' > /dev/null
> time kelora -j large.log --parallel --filter 'e.level == "ERROR"' > /dev/null

# Benchmark batch sizes
> for size in 100 1000 5000 10000; do
    echo "Batch size: $size"
    time kelora -j large.log --parallel --batch-size $size -qq
  done

# Compare thread counts
> for threads in 2 4 8 16; do
    echo "Threads: $threads"
    time kelora -j large.log --parallel --threads $threads -qq
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
0 2 * * * kelora -j /var/log/archives/$(date -d yesterday +\%Y-\%m-\%d)*.gz \
  --parallel --unordered -m > /reports/daily_$(date -d yesterday +\%Y-\%m-\%d).txt

# Parallel archive creation
find /var/log -name "*.log" -mtime +7 | \
  parallel "gzip {}" "kelora -j {}.gz --parallel -qq --stats"
```

**Export:**
```bash
# Export to CSV for analysis
> kelora -j logs/*.gz --parallel --unordered \
    -k timestamp,level,service,message \
    -F csv > consolidated.csv

# Export to JSON
> kelora -j logs/*.gz --parallel \
    --filter 'e.level == "ERROR"' \
    -J > errors.json

# Persist metrics
> kelora -j logs/*.gz --parallel --unordered \
    -e 'track_count(e.service)' \
    -m --metrics-file daily_metrics.json
```

## Troubleshooting

**Out of memory errors:**
```bash
# Reduce batch size
> kelora -j huge.log --parallel --batch-size 200

# Reduce thread count
> kelora -j huge.log --parallel --threads 2

# Use sequential mode
> kelora -j huge.log
```

**Slow processing:**
```bash
# Enable parallel mode
> kelora -j large.log --parallel

# Increase batch size
> kelora -j large.log --parallel --batch-size 5000

# Enable unordered output
> kelora -j large.log --parallel --unordered

# Increase threads for I/O-bound
> kelora -j /nfs/logs/*.gz --parallel --threads 32
```

**Inconsistent output:**
```bash
# Use ordered output (default in parallel mode)
> kelora -j logs/*.json --parallel

# Or disable parallel if order is critical
> kelora -j logs/*.json
```

**Missing events:**
```bash
# Check for parsing errors
> kelora -j archive.log.gz --parallel --verbose

# Validate with stats
> kelora -j archive.log.gz --parallel --stats

# Sample to verify
> kelora -j archive.log.gz --parallel -n 1000
```

**Progress monitoring:**
```bash
# Use pv (pipe viewer) for progress
> pv large.log.gz | gunzip | kelora -j --parallel

# Process with stats
> kelora -j large.log.gz --parallel --stats
```

## See Also

- [Performance Model](../concepts/performance-model.md) - Optimization guide
- [Build Streaming Alerts](build-streaming-alerts.md) - Real-time processing
- [CLI Reference](../reference/cli-reference.md) - All command-line options
- [Execution Modes](../concepts/performance-model.md#execution-modes) - How parallel mode works
