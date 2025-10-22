# Scripting Stages

Deep dive into Kelora's three Rhai scripting stages: `--begin`, `--exec`, and `--end`.

## Overview

Kelora provides three scripting stages for transforming log data with Rhai scripts:

```
--begin  →  [Process Events]  →  --end
              ↓
           --exec (per event)
```

| Stage | Runs | Purpose | Access |
|-------|------|---------|--------|
| `--begin` | Once before processing | Initialize state, load data | `conf` map, file helpers |
| `--exec` | Once per event | Transform events | `e` (event), `conf`, `meta`, tracking |
| `--end` | Once after processing | Summarize, report | `metrics`, `conf` |

## Begin Stage

### Purpose

The `--begin` stage runs **once** before any events are processed. Use it to:

- Initialize lookup tables
- Load reference data from files
- Set up shared configuration
- Prepare the `conf` map for use in other stages

### The `conf` Map

The global `conf` map is **read-write** in `--begin` and **read-only** in later stages.

```bash
kelora -j \
    --begin 'conf.valid_users = ["alice", "bob", "charlie"]' \
    --exec 'e.is_valid = e.user in conf.valid_users' \
    app.log
```

### Available Helpers

Special functions available **only** in `--begin`:

#### `read_lines(path)`

Read file as array of strings (one per line, UTF-8).

```bash
kelora -j \
    --begin 'conf.blocked_ips = read_lines("blocked.txt")' \
    --exec 'if e.ip in conf.blocked_ips { e = () }' \
    app.log
```

#### `read_file(path)`

Read entire file as single string (UTF-8).

```bash
kelora -j \
    --begin 'conf.template = read_file("template.txt")' \
    --end 'print(conf.template.replace("{count}", metrics["total"].to_string()))' \
    app.log
```

#### `read_json(path)`

Parse JSON file (convenience helper).

```bash
kelora -j \
    --begin 'conf.users = read_json("users.json")' \
    --exec 'e.user_name = conf.users.get(e.user_id, "unknown")' \
    app.log
```

### Examples

#### Load Lookup Table

```bash
kelora -j \
    --begin 'conf.services = #{api: "API Gateway", db: "Database", cache: "Redis"}' \
    --exec 'e.service_name = conf.services.get(e.service, e.service)' \
    app.log
```

#### Load IP Geolocation Data

```bash
kelora -j \
    --begin 'conf.ip_to_country = read_json("geoip.json")' \
    --exec 'e.country = conf.ip_to_country.get(e.ip, "unknown")' \
    app.log
```

#### Initialize Counters

```bash
kelora -j \
    --begin 'conf.start_time = now_utc()' \
    --end 'let duration = now_utc() - conf.start_time; print("Processed in " + duration + "s")' \
    app.log
```

#### Load Configuration

```bash
kelora -j \
    --begin 'conf.threshold = 1000; conf.alert_email = "ops@company.com"' \
    --exec 'if e.duration_ms > conf.threshold { eprint("⚠️  Slow request: " + e.path) }' \
    app.log
```

## Exec Stage

### Purpose

The `--exec` stage runs **once per event**. Use it to:

- Transform event fields
- Add computed fields
- Filter events (via `e = ()`)
- Track metrics
- Emit multiple events from arrays

### The Event Variable

The current event is available as `e`. Modifications to `e` persist through subsequent `--exec` scripts.

```bash
kelora -j \
    --exec 'e.duration_s = e.duration_ms / 1000' \
    --exec 'e.is_slow = e.duration_s > 1.0' \
    app.log
```

### Multiple Exec Scripts

Multiple `--exec` scripts run in order. Each sees changes from previous scripts.

```bash
kelora -j \
    --exec 'e.duration_s = e.duration_ms / 1000' \
    --exec 'track_avg("duration", e.duration_s)' \
    --exec 'if e.duration_s > 5.0 { e.alert = true }' \
    app.log
```

**Execution order:**
1. Convert `duration_ms` to `duration_s`
2. Track average duration
3. Add `alert` field for slow requests

### Intermixing --filter and --exec

`--filter` and `--exec` are both script stages and execute in **exact CLI order**:

```bash
kelora -j \
    --exec 'e.duration_s = e.duration_ms / 1000' \    # Stage 1: Transform all events
    --filter 'e.duration_s > 1.0' \                    # Stage 2: Keep only slow events
    --exec 'track_count("slow")' \                     # Stage 3: Track (slow only)
    --exec 'e.alert = true' \                          # Stage 4: Add field (slow only)
    app.log
```

**What happens:**
1. First `--exec` adds `duration_s` field to all events
2. `--filter` removes events under 1.0s
3. Second `--exec` only processes slow events (tracks count)
4. Third `--exec` only processes slow events (adds alert field)

Later stages only see events that passed earlier filters. This allows precise control over which events are transformed or tracked.

### Atomic Execution

In resilient mode (default), exec scripts execute **atomically**:

- If an error occurs, changes are rolled back
- Original event is returned unchanged
- Processing continues with next event

```bash
kelora -j \
    --exec 'e.result = e.value.to_int() * 2' \
    app.log
```

If `e.value` is not a valid integer:

- Error is recorded
- Event passes through unchanged
- No partial modifications

In strict mode (`--strict`), errors abort immediately.

### Common Patterns

#### Transform Fields

```bash
kelora -j \
    --exec 'e.level = e.level.to_upper()' \
    --exec 'e.message = e.message.trim()' \
    app.log
```

#### Add Computed Fields

```bash
kelora -j \
    --exec 'e.duration_s = e.duration_ms / 1000' \
    --exec 'e.timestamp_unix = e.timestamp.to_unix()' \
    app.log
```

#### Conditional Field Creation

```bash
kelora -j \
    --exec 'if e.status >= 500 { e.severity = "critical" } else if e.status >= 400 { e.severity = "error" }' \
    app.log
```

#### Remove Events

```bash
kelora -j \
    --exec 'if e.level == "DEBUG" { e = () }' \
    app.log
```

#### Track Metrics

```bash
kelora -j \
    --exec 'track_count(e.service)' \
    --exec 'track_avg("response_time", e.duration_ms)' \
    --metrics \
    app.log
```

#### Fan-Out Arrays

```bash
kelora -j \
    --exec 'emit_each(e.items)' \
    app.log
```

Each array element becomes a separate event.

### Access to conf

The `conf` map from `--begin` is **read-only** in `--exec`:

```bash
kelora -j \
    --begin 'conf.multiplier = 2.5' \
    --exec 'e.adjusted = e.value * conf.multiplier' \
    app.log
```

### Access to meta

The `meta` variable provides event metadata in `--exec` and `--filter`:

```bash
kelora -j \
    --exec 'e.source = meta.filename' \
    server1.log server2.log
```

**Available metadata attributes:**

- `meta.line` - Original raw line from input (always available)
- `meta.line_num` - Line number, 1-based (available when processing files)
- `meta.filename` - Source filename (available with multiple files or explicit file arguments)

**Multi-file tracking example:**

```bash
kelora -j logs/*.log --metrics \
    --exec 'if e.level == "ERROR" { track_count(meta.filename) }' \
    --end 'for file in metrics.keys() { print(file + ": " + metrics[file] + " errors") }'
```

**Debugging with line numbers:**

```bash
kelora -j --filter 'e.status >= 500' \
    --exec 'eprint("⚠️  Server error at " + meta.filename + ":" + meta.line_num)' \
    app.log
```

**Re-parsing with raw line:**

```bash
kelora -j \
    --exec 'if e.message.contains("CUSTOM:") { e.custom = meta.line.after("CUSTOM:").parse_json() }' \
    app.log
```

## End Stage

### Purpose

The `--end` stage runs **once** after all events are processed. Use it to:

- Summarize metrics
- Generate reports
- Print final statistics
- Export aggregated data

### The metrics Map

The global `metrics` map contains all tracked data from `track_*()` functions:

```bash
kelora -j \
    --exec 'track_count(e.service)' \
    --end 'for key in metrics.keys() { print(key + ": " + metrics[key]) }' \
    app.log
```

### Available Data

In `--end`, you have access to:

- `metrics` - All tracked metrics (counts, sums, averages, etc.)
- `conf` - Read-only configuration from `--begin`
- Standard Rhai functions (print, file helpers if `--allow-fs-writes`)

### Examples

#### Print Summary Statistics

```bash
kelora -j \
    --exec 'track_count("total"); if e.level == "ERROR" { track_count("errors") }' \
    --end 'let error_rate = metrics.errors / metrics.total * 100; print("Error rate: " + error_rate + "%")' \
    app.log
```

#### Generate Report

```bash
kelora -j \
    --exec 'track_count(e.service)' \
    --end 'print("=== Service Report ==="); for svc in metrics.keys() { print(svc + ": " + metrics[svc] + " requests") }' \
    app.log
```

#### Export Metrics to File

```bash
kelora -j --allow-fs-writes \
    --exec 'track_count(e.service)' \
    --end 'append_file("report.txt", "Total services: " + metrics.len().to_string())' \
    app.log
```

#### Calculate Percentages

```bash
kelora -j \
    --exec 'track_count("total"); track_count(e.level)' \
    --end 'for level in ["INFO", "WARN", "ERROR"] { let pct = metrics.get(level, 0) / metrics.total * 100; print(level + ": " + pct + "%") }' \
    app.log
```

## Stage Interaction

### Data Flow Between Stages

```
--begin:  Initialize conf map
    ↓
    conf (read-only)
    ↓
--exec:   Process events, track metrics
    ↓
    metrics + conf (both read-only)
    ↓
--end:    Summarize and report
```

### Complete Example

```bash
kelora -j \
    --begin 'conf.threshold = 1000; conf.start = now_utc()' \
    --exec 'if e.duration_ms > conf.threshold { track_count("slow") }' \
    --exec 'track_count("total")' \
    --end 'let elapsed = now_utc() - conf.start; print("Processed " + metrics.total + " events in " + elapsed + "s"); print("Slow requests: " + metrics.get("slow", 0))' \
    app.log
```

**Flow:**
1. `--begin`: Set threshold to 1000ms, record start time
2. `--exec` (per event): Track slow requests, track total
3. `--end`: Calculate elapsed time, print summary

## Using Exec Files

### `-E, --exec-file`

Load Rhai script from file for the exec stage:

**transform.rhai:**
```rhai
// Convert duration to seconds
e.duration_s = e.duration_ms / 1000;

// Add severity based on status
if e.status >= 500 {
    e.severity = "critical";
} else if e.status >= 400 {
    e.severity = "error";
} else {
    e.severity = "ok";
}

// Track metrics
track_count(e.severity);
track_avg("response_time", e.duration_s);
```

**Usage:**
```bash
kelora -j -E transform.rhai --metrics app.log
```

### `-I, --include`

Include Rhai library files before script stages:

**helpers.rhai:**
```rhai
fn classify_status(status) {
    if status >= 500 {
        "server_error"
    } else if status >= 400 {
        "client_error"
    } else if status >= 300 {
        "redirect"
    } else if status >= 200 {
        "success"
    } else {
        "other"
    }
}
```

**Usage:**
```bash
kelora -j \
    -I helpers.rhai \
    --exec 'e.status_class = classify_status(e.status)' \
    app.log
```

## Best Practices

### Use --begin for Initialization

**Good:**
```bash
kelora -j \
    --begin 'conf.lookup = read_json("data.json")' \
    --exec 'e.name = conf.lookup.get(e.id, "unknown")' \
    app.log
```

**Bad:**
```bash
kelora -j \
    --exec 'let lookup = read_json("data.json"); e.name = lookup.get(e.id, "unknown")' \
    app.log
```

The bad example reads the file **once per event** (slow and wasteful).

### Keep --exec Scripts Simple

Break complex logic into multiple `--exec` scripts:

**Good:**
```bash
kelora -j \
    --exec 'e.duration_s = e.duration_ms / 1000' \
    --exec 'e.is_slow = e.duration_s > 1.0' \
    --exec 'if e.is_slow { track_count("slow_requests") }' \
    app.log
```

**Bad:**
```bash
kelora -j \
    --exec 'e.duration_s = e.duration_ms / 1000; e.is_slow = e.duration_s > 1.0; if e.is_slow { track_count("slow_requests") }' \
    app.log
```

The good example is easier to read and debug.

### Use --end for Summaries

**Good:**
```bash
kelora -j \
    --exec 'track_count(e.service)' \
    --end 'print("Total services: " + metrics.len())' \
    app.log
```

**Bad:**
```bash
kelora -j \
    --exec 'track_count(e.service); print("Processing...")' \
    app.log
```

The bad example prints on every event (noisy).

### Leverage File Helpers

For complex logic, use `-E` and `-I`:

```bash
kelora -j -I helpers.rhai -E transform.rhai --metrics app.log
```

This keeps command lines clean and logic maintainable.

## Performance Considerations

### Begin Stage Overhead

The `--begin` stage runs once, so file I/O here is acceptable:

```bash
kelora -j \
    --begin 'conf.large_dataset = read_json("10mb.json")' \
    --exec 'e.enriched = conf.large_dataset.get(e.id, #{})' \
    app.log
```

### Exec Stage Optimization

The `--exec` stage runs per event. Avoid expensive operations:

**Slow:**
```bash
kelora -j \
    --exec 'let lookup = read_json("data.json"); e.name = lookup.get(e.id, "unknown")' \
    app.log
```

**Fast:**
```bash
kelora -j \
    --begin 'conf.lookup = read_json("data.json")' \
    --exec 'e.name = conf.lookup.get(e.id, "unknown")' \
    app.log
```

### End Stage Overhead

The `--end` stage runs once, so complex calculations are fine:

```bash
kelora -j \
    --exec 'track_count(e.service)' \
    --end 'let sorted = metrics.keys().sort(); for key in sorted { print(key + ": " + metrics[key]) }' \
    app.log
```

## Parallel Processing

When using `--parallel`, scripting stages behave differently:

### Begin and End

`--begin` and `--end` run **once** (not parallelized):

```bash
kelora -j --parallel \
    --begin 'conf.start = now_utc()' \
    --exec 'track_count(e.service)' \
    --end 'print("Duration: " + (now_utc() - conf.start))' \
    app.log
```

### Exec Stage

`--exec` runs in parallel across worker threads:

- Each thread has its own copy of `conf` (read-only)
- Tracking functions aggregate across threads
- Event modifications are isolated per thread

```bash
kelora -j --parallel \
    --exec 'e.duration_s = e.duration_ms / 1000' \
    --exec 'track_count(e.service)' \
    app.log
```

### Thread Safety

Kelora handles thread safety automatically:

- `conf` is cloned per thread (immutable)
- `metrics` uses thread-safe aggregation
- Event modifications are isolated

You don't need to worry about race conditions in scripts.

## Troubleshooting

### conf is Read-Only in --exec

**Problem:**
```bash
kelora -j --exec 'conf.value = 42' app.log
# Error: conf is read-only in exec stage
```

**Solution:** Initialize in `--begin`:
```bash
kelora -j --begin 'conf.value = 42' --exec 'e.result = conf.value * 2' app.log
```

### metrics Not Available in --exec

**Problem:**
```bash
kelora -j --exec 'print(metrics["total"])' app.log
# Error: metrics not available in exec stage
```

**Solution:** Use `--end`:
```bash
kelora -j --exec 'track_count("total")' --end 'print(metrics["total"])' app.log
```

### File Helpers Not Working

**Problem:**
```bash
kelora -j --exec 'append_file("out.txt", e.message)' app.log
# Error: filesystem writes not allowed
```

**Solution:** Add `--allow-fs-writes`:
```bash
kelora -j --allow-fs-writes --exec 'append_file("out.txt", e.message)' app.log
```

### Script Stage Ordering

**Understanding:** `--filter` and `--exec` execute in **exact CLI order**, intermixed.

```bash
# Both --exec scripts run on all events
kelora -j --exec 'e.a = 1' --exec 'e.b = e.a + 1' app.log

# Filter runs between execs - second --exec only sees filtered events
kelora -j --exec 'e.a = 1' --filter 'e.level == "ERROR"' --exec 'e.b = e.a + 1' app.log
```

**What happens in the second example:**
1. First `--exec` adds field `a=1` to all events
2. `--filter` removes non-ERROR events
3. Second `--exec` adds field `b=2` to ERROR events only

This is **expected behavior** - later stages only process events that passed earlier filters. Each stage (filter or exec) processes the output of the previous stage sequentially.

## See Also

- [Processing Architecture](pipeline-model.md) - Three-layer processing model and script stage ordering
- [Events and Fields](events-and-fields.md) - Working with event data
- [Function Reference](../reference/functions.md) - All available Rhai functions
- [CLI Reference](../reference/cli-reference.md) - Complete flag documentation
