# Integrate Kelora with External Tools

Kelora thrives in Unix pipelines—combine it with specialized tools to filter, transform, store, visualize, and explore log data.

## When This Guide Helps

- You want to pre-filter logs before Kelora parses them.
- Kelora's output needs further analysis (SQL, advanced JSON queries, statistical aggregation).
- You need interactive exploration, visualization, or long-term storage.
- Your workflow requires format conversion or integration with existing tooling.

## Before You Start

- Kelora installed (see [Install](../index.md#install)).
- Basic familiarity with Kelora's input formats (`-j`, `-f logfmt`, `-f csv`, etc.) and output formats (`-F json`, `-F csv`, `-F tsv`).
- Access to the external tools you want to use (installation instructions vary by tool).

## Quick Reference

### Upstream Tools (Input Side)

Tools that feed data **into** Kelora:

| Tool | Category | Use For |
|------|----------|---------|
| **grep/ripgrep** | Pre-filter | Fast text filtering before parsing |
| **tail/journalctl** | Streaming | Live log monitoring |
| **kubectl** | Container logs | Stream Kubernetes pod logs |
| **docker/docker compose** | Container logs | Stream Docker container logs |
| **find + xargs** | Discovery | Locate log archives for batch processing |
| **jc** | Converter | Convert command output to JSON for Kelora |

### Downstream Tools (Output Side)

Tools that process data **from** Kelora:

| Tool | Category | Use For | Kelora Output |
|------|----------|---------|---------------|
| **jq** | JSON query | Advanced JSON transformations | `-J` |
| **SQLite/DuckDB** | Database | SQL queries, aggregations, storage | `-F csv`, `-J` |
| **sort** | Sorting | Order by timestamp, level, or other fields | `-F tsv`, `-F csv` |
| **column** | Formatter | Pretty-print TSV as aligned tables | `-F tsv` |
| **miller** | Data processing | Multi-format analytics and reshaping | `-F csv`, `-J` |
| **qsv** | CSV analytics | Statistical analysis, joins, validation | `-F csv` |
| **lnav** | Interactive viewer | Explore and search logs interactively | `-J` |
| **visidata** | Spreadsheet | Terminal-based data analysis | `-F csv`, `-F tsv` |
| **ov** | Pager | Feature-rich viewing and search | (any) |
| **csvlens** | CSV viewer | Interactive CSV exploration | `-F csv` |
| **rare** | Visualization | Real-time histograms and charts | `-J`, `-F tsv` |
| **angle-grinder** | Alternative processor | Different query syntax | `-J` |

---

## Upstream Tools (Pre-Processing)

### grep / ripgrep — Fast Pre-Filtering

Search for text patterns before parsing to reduce Kelora's workload. Grep can be faster than parsing every line when you only need a small subset. Use when scanning huge files for specific keywords.

```bash
# Pre-filter for "ERROR" lines before parsing
grep -i "ERROR" /var/log/app.log | kelora -f logfmt

# Faster with ripgrep, case-insensitive, show context
rg -i "timeout" /var/log/app.log -A 2 | kelora -j
```

Kelora's `--filter` is more powerful for structured field checks, but `grep` excels at raw text scanning across massive files.

---

### find + xargs — Archive Discovery

Locate log files across directories and process them in parallel. Use when you have scattered archives that need systematic batch processing. The `find` command provides powerful file selection (by date, size, name), and `-print0 | xargs -0` handles paths with spaces safely.

```bash
# Find all compressed JSON logs from April 2024
find /archives -name "app-2024-04-*.jsonl.gz" -print0 | \
  xargs -0 kelora -j --parallel -l error --stats

# Recursive search for all .log files modified in last 7 days
find /var/log -name "*.log" -mtime -7 -print0 | \
  xargs -0 kelora -f auto --filter 'e.level == "ERROR"' -F json
```

See [Process Archives at Scale](batch-process-archives.md) for parallel processing strategies.

---

### tail / journalctl — Live Streaming

Stream live log data into Kelora for real-time monitoring and alerting during deployments or incidents. Use `tail -F` (capital F) to survive log rotation. Combine with Kelora's `-qq` for quiet monitoring.

```bash
# Monitor live logs for critical errors
tail -F /var/log/app.log | kelora -j -l critical -qq

# Stream systemd journal logs for a specific service
journalctl -u myapp.service -f --output=json | \
  kelora -j --filter 'e.priority <= 3' -F json
```

See [Design Streaming Alerts](build-streaming-alerts.md) for complete alert workflows.

---

### kubectl — Kubernetes Pod Logs

Stream logs from Kubernetes pods into Kelora for real-time monitoring and analysis. Essential for debugging containerized applications and tracking issues across pod restarts. Use `-f` for live streaming, `--tail` to limit history, and `--all-containers` for multi-container pods.

```bash
# Stream logs from a specific pod
kubectl logs -f pod-name | kelora -j -l error,warn

# Follow logs from a deployment with label selector
kubectl logs -f -l app=myapp --all-containers=true | \
  kelora -j --filter 'e.level == "ERROR"' -F json

# Stream logs from previous container instance (after crash)
kubectl logs -f pod-name --previous | \
  kelora -j --window 50 -e 'track_count("error_type|" + e.error)'

# Multi-pod log aggregation
kubectl logs -f -l app=myapp --prefix=true | \
  kelora -f auto -e 'e.pod = e.message.split(" ")[0]' -J
```

See [Design Streaming Alerts](build-streaming-alerts.md) for alerting patterns that work well with kubectl streaming.

---

### docker / docker compose — Container Logs

Stream Docker container logs into Kelora for local development and single-host deployments. Use `docker logs -f` or `docker compose logs -f` for live streaming. Combine `--tail` to limit history and `--timestamps` for temporal analysis.

```bash
# Stream logs from a single container
docker logs -f container-name | kelora -j -l error,critical

# Follow logs from all compose services
docker compose logs -f | kelora -f logfmt --filter 'e.service == "api"'

# Stream with timestamps for correlation
docker logs -f --timestamps myapp | \
  kelora -f auto -e 'track_count(e.level)' --metrics

# Multiple containers with filtering
docker compose logs -f api worker | \
  kelora -j --filter 'e.response_time > 1000' -k timestamp,service,response_time
```

---

### jc — Command Output Converter

Converts output from 100+ CLI tools (ls, ps, netstat, etc.) into JSON for Kelora to parse. Turns unstructured command output into queryable structured data.

```bash
# Parse directory listings as structured data
jc ls -la | kelora -j --filter 'e.size > 1000000' -k filename,size

# Analyze running processes
jc ps aux | kelora -j --filter 'e.pcpu > 50.0' -k user,pid,command
```

[jc documentation](https://kellyjonbrazil.github.io/jc/)

---

## Downstream Tools (Post-Processing)

### jq — Advanced JSON Manipulation

Powerful JSON querying, reshaping, and computation beyond Kelora's built-in capabilities. Use for complex JSON restructuring, recursive descent, or computations that are awkward in Rhai. Kelora focuses on log-specific operations; jq excels at generic JSON transformation. Use Kelora for filtering, extraction, and log-specific functions. Use jq for complex JSON reshaping and output formatting.

```bash
# Kelora extracts errors, jq reshapes into nested structure
kelora -j examples/simple_json.jsonl -l error -J | \
  jq 'group_by(.service) | map({service: .[0].service, count: length})'

# Combine Kelora filtering with jq's advanced path queries
kelora -j logs/app.jsonl --filter 'e.status >= 500' -J | \
  jq -r '.request.headers | to_entries[] | "\(.key): \(.value)"'
```

[jq manual](https://jqlang.github.io/jq/)

---

### qsv — CSV Analytics Powerhouse

High-performance CSV toolkit with statistics, joins, validation, SQL queries, and more. Adds SQL-like operations, statistical analysis, and data validation to CSV workflows. Use when Kelora's CSV output needs further analysis, validation, or joining with other datasets.

```bash
# Kelora extracts to CSV, qsv generates statistics
kelora -j logs/app.jsonl -k timestamp,response_time,status -F csv | \
  qsv stats --select response_time

# Frequency analysis of error codes
kelora -j logs/app.jsonl -l error -k error_code,service -F csv | \
  qsv frequency --select error_code | \
  qsv sort --select count --reverse

# Join Kelora output with reference data
kelora -j logs/orders.jsonl -k order_id,user_id,amount -F csv > orders.csv
qsv join user_id orders.csv user_id users.csv | qsv select 'user_id,name,order_id,amount'
```

[qsv documentation](https://github.com/jqnatividad/qsv)

---

### miller — Multi-Format Data Processing

Like awk/sed/cut for name-indexed data (CSV, TSV, JSON). Supports aggregations, joins, and format conversion. Bridges formats and adds SQL-style aggregations with simple syntax.

```bash
# Kelora to JSON, miller calculates aggregates
kelora -j logs/app.jsonl -k service,response_time -J | \
  mlr --ijson --opprint stats1 -a mean,p50,p99 -f response_time -g service

# Convert Kelora CSV to pretty-printed table with computed columns
kelora -j logs/app.jsonl -k timestamp,bytes_sent,bytes_received -F csv | \
  mlr --csv --opprint put '$total_bytes = $bytes_sent + $bytes_received'

# JSON to TSV conversion with field selection
kelora -j logs/app.jsonl -J | \
  mlr --ijson --otsv cut -f timestamp,level,message
```

[miller documentation](https://miller.readthedocs.io/)

---

### sort — Column-Based Sorting

Sorts Kelora's TSV/CSV output by columns. Kelora doesn't have built-in sorting, so use `sort` when output order matters for chronological analysis, top-N queries, and ordered exports. Use `-t$'\t'` for tab delimiter. Column numbers are 1-indexed.

```bash
# Sort errors by timestamp
kelora -j logs/app.jsonl -l error -k timestamp,service,message -F tsv | \
  sort -t$'\t' -k1,1

# Sort by response time (numeric, descending)
kelora -j logs/app.jsonl -k timestamp,response_time,path -F tsv | \
  sort -t$'\t' -k2,2nr | head -20

# Multi-column sort: service (ascending), then timestamp (descending)
kelora -j logs/app.jsonl -F tsv | \
  sort -t$'\t' -k2,2 -k1,1r
```

---

### SQLite / DuckDB — SQL Analytics

Load Kelora output into a SQL database for complex queries, aggregations, and long-term storage. Use when you need JOINs, window functions, complex GROUP BY, or persistent storage. SQL provides rich aggregation, time-series analysis, and joins that complement Kelora's streaming model.

**SQLite example:**

```bash
# Export to CSV, import to SQLite
kelora -j logs/app.jsonl -k timestamp,level,service,message -F csv > logs.csv

sqlite3 logs.db <<EOF
.mode csv
.import logs.csv logs
CREATE INDEX idx_level ON logs(level);
SELECT service, COUNT(*) as error_count
FROM logs
WHERE level = 'ERROR'
GROUP BY service
ORDER BY error_count DESC;
EOF
```

**DuckDB example (faster, better CSV/JSON support):**

```bash
# Query Kelora JSON output directly with DuckDB
kelora -j logs/app.jsonl -J > logs.json

duckdb -c "
  SELECT service,
         COUNT(*) as total,
         AVG(response_time) as avg_response_time
  FROM read_json_auto('logs.json')
  WHERE status >= 500
  GROUP BY service
  ORDER BY total DESC;
"

# Or import CSV for repeated queries
kelora -j logs/app.jsonl -F csv | \
  duckdb -c "CREATE TABLE logs AS SELECT * FROM read_csv_auto('/dev/stdin');
             SELECT DATE(timestamp) as day, COUNT(*) FROM logs GROUP BY day;"
```

[SQLite](https://sqlite.org/), [DuckDB](https://duckdb.org/)

---

### angle-grinder — Alternative Query Syntax

Log aggregation and filtering with a query language different from Kelora's Rhai scripting. Offers an alternative when Rhai scripting isn't a good fit, or when you want a more declarative syntax. angle-grinder and Kelora overlap significantly—choose based on syntax preference and feature fit.

```bash
# Parse and aggregate with angle-grinder instead of Kelora
kelora -j logs/app.jsonl -J | \
  agrind '* | json | count by service'

# Combine: Kelora normalizes format, angle-grinder aggregates
kelora -f logfmt logs/mixed.log -J | \
  agrind '* | json | parse "user_id=*" as user | count by user'
```

[angle-grinder](https://github.com/rcoh/angle-grinder)

---

## Viewers & Exploration

### lnav — Interactive Log Navigator

Advanced TUI for browsing, searching, and analyzing logs with syntax highlighting and SQL queries. Provides interactive search, filtering, and timeline views that complement Kelora's batch processing.

```bash
# Pre-filter with Kelora, explore in lnav
kelora -j logs/app.jsonl -l error,warn -J | lnav

# Export Kelora results and open in lnav
kelora -j logs/app.jsonl -J -o filtered.json
lnav filtered.json
```

[lnav documentation](https://lnav.org/)

---

### csvlens — Terminal CSV Viewer

Interactive CSV viewer with search, filtering, and column inspection. Easier than scrolling through raw CSV in the terminal.

```bash
# Export to CSV and view interactively
kelora -j logs/app.jsonl -k timestamp,service,level,message -F csv | csvlens
```

[csvlens](https://github.com/YS-L/csvlens)

---

### visidata — Terminal Spreadsheet

Powerful TUI for exploring, transforming, and analyzing tabular data (CSV, TSV, JSON, SQLite, and more). Combines spreadsheet-like exploration with vim keybindings and powerful data manipulation.

```bash
# Open Kelora CSV output in visidata
kelora -j logs/app.jsonl -F csv | vd

# Open JSON output (visidata can parse nested structures)
kelora -j logs/app.jsonl -J | vd -f jsonl
```

[visidata documentation](https://www.visidata.org/)

---

### ov — Feature-Rich Terminal Pager

Advanced pager with search, filtering, and better navigation than `less`. Better UX for large outputs.

```bash
# Page through Kelora output with enhanced search
kelora -j logs/app.jsonl | ov

# Pre-filter and page
kelora -j logs/app.jsonl -l error,warn | ov
```

[ov](https://github.com/noborus/ov)

---

## Visualization

### rare — Real-Time Histograms and Charts

Creates console histograms, bar graphs, tables, and heatmaps from streaming data. Quick visual insights without leaving the terminal.

```bash
# Bar chart of log levels
kelora -j logs/app.jsonl -k level -F tsv | rare histo --field level

# Time-series histogram of events per minute
kelora -j logs/app.jsonl -k timestamp -F tsv | \
  rare histo --field timestamp --time

# Table summary of services and error counts
kelora -j logs/app.jsonl -l error -k service -F tsv | rare table
```

[rare](https://github.com/zix99/rare)

---

## Format Conversion

### column — Pretty-Print TSV Tables

Formats tab-separated data into aligned columns for readability. Makes TSV output readable in the terminal. GNU `column` (from util-linux) has more features than BSD `column`.

```bash
# Pretty-print TSV output
kelora -j logs/app.jsonl -k timestamp,level,service,message -F tsv | \
  column -ts $'\t'

# With custom separators and JSON output
kelora -j logs/app.jsonl -F tsv | \
  column -ts $'\t' -N timestamp,level,service,message -J > output.json
```

---

## Destinations (Storage & Aggregation)

For production log aggregation and long-term storage, Kelora's JSON output (`-J`) integrates well with centralized logging systems:

- **VictoriaLogs** — Fast, resource-efficient log database with LogsQL querying and Unix tool integration
- **Grafana Loki** — Horizontally-scalable, multi-tenant log aggregation
- **Graylog** — Free and open log management platform
- **OpenObserve** — Cloud-native observability for logs, metrics, and traces at petabyte scale
- **qryn** — Polyglot observability platform

**Typical integration pattern:**

```bash
# Stream Kelora output to aggregation system via HTTP
kelora -j logs/app.jsonl -J | \
  while IFS= read -r line; do
    curl -X POST http://loki:3100/loki/api/v1/push \
      -H "Content-Type: application/json" \
      -d "$line"
  done
```

**Or batch upload:**

```bash
# Process logs and upload to object storage for ingestion
kelora -j logs/*.jsonl.gz --parallel -J -o processed.json
rclone copy processed.json remote:logs/$(date +%Y-%m-%d)/
```

---

## Common Patterns

### Parallel Discovery and Processing

Combine `find`, `xargs`, and Kelora's `--parallel` for maximum throughput:

```bash
find /archives -name "*.jsonl.gz" -print0 | \
  xargs -0 -P 4 kelora -j --parallel \
    -l error \
    -e 'track_count(e.service)' \
    --metrics
```

---

### Multi-Stage Pipelines

Chain tools for complex transformations:

```bash
# Stage 1: Grep pre-filter
# Stage 2: Kelora parse and extract
# Stage 3: qsv aggregate
# Stage 4: rare visualize

grep -i "checkout" /var/log/app.log | \
  kelora -f logfmt -k timestamp,duration,user_id -F csv | \
  qsv stats --select duration | \
  rare table
```

---

### Live Monitoring with Alerts

Stream logs, filter with Kelora, visualize with rare, alert on anomalies:

```bash
tail -F /var/log/app.log | \
  kelora -j -l error,critical -J | \
  rare histo --field service &

# In another terminal: alert on thresholds
tail -F /var/log/app.log | \
  kelora -j -l error -qq \
    --filter 'e.service == "payments"' \
    -e 'eprint("PAYMENT ERROR: " + e.message)'
```

---

## Performance Tips

**When to pre-filter with grep:**
- Scanning for specific keywords in huge unstructured files
- Text search is simpler than structured field checks
- Benchmarking shows grep is faster for your use case

**When to use Kelora's --filter instead:**
- Checking structured fields (e.g., `e.status >= 500`)
- Complex boolean logic or numerical comparisons
- You need type-aware filtering (strings, numbers, timestamps)

**When to post-process with jq/qsv/miller:**
- Kelora extracted and normalized the data
- You need reshaping, aggregation, or format conversion
- SQL-style operations (GROUP BY, JOINs) are needed

**Benchmark your pipeline:**

```bash
# Measure each stage
time grep "ERROR" logs/huge.log | wc -l
time kelora -j logs/huge.log --filter 'e.level == "ERROR"' -qq | wc -l

# Compare integrated vs multi-tool approaches
hyperfine \
  'kelora -j logs/app.jsonl -l error --stats' \
  'grep -i error logs/app.log | kelora -f logfmt --stats'
```

See [Concept: Performance Comparisons](../concepts/performance-comparisons.md) for detailed benchmarks.

---

## Common Pitfalls

**Shell quoting:** Remember to quote arguments with special characters:

```bash
# Wrong: shell expands * and $
kelora -f 'cols:ts *message' --exec 'e.new_field = $other'

# Right: quote to protect from shell
kelora -f 'cols:ts *message' --exec 'e.new_field = e.other'
```

**Pipe buffering:** Some tools buffer output. Use `stdbuf` or tool-specific flags for live streaming:

```bash
# Force line buffering for live output
tail -F /var/log/app.log | stdbuf -oL kelora -j | stdbuf -oL grep "ERROR"
```

**Column indexing:** `sort` uses 1-indexed columns, not 0-indexed:

```bash
# Wrong: -k0 doesn't exist
kelora -F tsv | sort -t$'\t' -k0,0

# Right: first column is -k1
kelora -F tsv | sort -t$'\t' -k1,1
```

**Resource limits:** Parallel processing with multiple tools can exhaust file descriptors or memory:

```bash
# Increase limits if needed
ulimit -n 4096
kelora --parallel --threads 8 ... | qsv ... | visidata
```

---

## See Also

- [Process Archives at Scale](batch-process-archives.md) — Parallel processing strategies
- [Design Streaming Alerts](build-streaming-alerts.md) — Real-time monitoring workflows
- [Triage Production Errors](find-errors-in-logs.md) — Error investigation patterns
- [Concept: Performance Comparisons](../concepts/performance-comparisons.md) — Benchmark data for tool selection
- [CLI Reference](../reference/cli-reference.md) — Complete Kelora command-line reference
