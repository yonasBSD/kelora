# Pipeline Model

Understanding how Kelora processes logs through a multi-stage pipeline.

## Overview

Kelora processes logs through a streaming pipeline with distinct stages. Each stage transforms the data and passes it to the next stage. Understanding this model helps you write more effective log processing commands.

## Pipeline Stages

```
Input → Parse → Filter → Transform → Output
```

### 1. Input Stage

**Purpose:** Read and preprocess log lines from files or stdin.

**Operations:**
- Read files (including `.gz` compressed)
- Handle stdin from pipes
- Split into lines
- Skip empty lines (except `-f line` format)
- Apply `--extract-prefix` if configured

**Key Flags:**
- Files specified as positional arguments
- `--extract-prefix` - Extract prefixed text before parsing
- `--prefix-sep` - Separator for prefix extraction

**Example:**
```bash
> kelora file1.log file2.log.gz
> tail -f app.log | kelora -j
> docker compose logs | kelora --extract-prefix service
```

### 2. Parse Stage

**Purpose:** Convert raw log lines into structured events (maps/objects).

**Operations:**
- Detect or apply format (`-f json`, `-f logfmt`, etc.)
- Parse line into fields
- Handle parse errors (skip in resilient mode, abort in strict mode)
- Apply `--ts-format` and `--input-tz` for timestamps

**Key Flags:**
- `-f, --input-format` - Specify format (json, logfmt, syslog, combined, etc.)
- `--ts-format` - Custom timestamp format
- `--input-tz` - Timezone for naive timestamps
- `--strict` - Fail on parse errors

**Example:**
```bash
> kelora -f json app.log
> kelora -f combined access.log
> kelora -f "cols:timestamp:ts level:5 message:*" custom.log
```

**Output:** Structured event (map) with fields accessible as `e.field`.

### 3. Filter Stage

**Purpose:** Select which events to process further.

**Operations:**
- Level filtering (`--levels`)
- Time filtering (`--since`, `--until`)
- Custom Rhai expressions (`--filter`)
- Context lines (`--before-context`, `--after-context`)

**Key Flags:**
- `--levels` - Filter by log level
- `--since` / `--until` - Time-based filtering
- `--filter` - Custom Rhai expression (must return true/false)
- `--before-context` / `--after-context` - Include surrounding events

**Execution Order:**
1. Level filtering
2. Time filtering
3. Custom filters (each `--filter` in order)
4. Context lines (if matched)

**Example:**
```bash
> kelora -j app.log --levels error
> kelora -j app.log --since "1 hour ago"
> kelora -j app.log --filter 'e.service == "database"'
```

**Behavior:**
- Filters return `true` (keep) or `false` (skip)
- In resilient mode: filter errors return `false`
- In strict mode: filter errors abort processing

### 4. Transform Stage

**Purpose:** Modify, enrich, or aggregate event data.

**Operations:**
- Execute Rhai scripts (`--exec`)
- Run `--begin` scripts once at start
- Run `--end` scripts once at finish
- Track metrics (`track_count()`, etc.)
- Fan-out arrays (`emit_each()`)
- Remove fields (`e.field = ()`)

**Key Flags:**
- `--begin` - Run once before processing
- `--exec` - Run for each event (multiple allowed)
- `--end` - Run once after processing
- `--metrics` - Display tracked metrics at end
- `--window` - Enable windowed analysis

**Execution Order:**
1. `--begin` (once, before any events)
2. For each event:
   - Execute each `--exec` in order
   - Atomic execution (rollback on error in resilient mode)
3. `--end` (once, after all events)
4. Display `--metrics` (if enabled)

**Example:**
```bash
> kelora -j app.log \
    --begin 'print("Starting analysis")' \
    --exec 'e.duration_s = e.duration_ms / 1000' \
    --exec 'track_count(e.service)' \
    --end 'print("Complete")' \
    --metrics
```

**Behavior:**
- Transformations modify events in place
- In resilient mode: errors return original event unchanged
- In strict mode: transformation errors abort processing
- Empty events (`e = ()`) are filtered out before output

### 5. Output Stage

**Purpose:** Format and emit events.

**Operations:**
- Apply `--keys` field selection
- Convert timestamps (`--convert-ts`, `--show-ts-local`, `--show-ts-utc`)
- Format events (default, JSON, CSV, etc.)
- Apply `--take` limit
- Write to stdout or files

**Key Flags:**
- `--keys` - Select top-level fields to output
- `-F, --output-format` - Output format (default, json, csv, etc.)
- `--convert-ts` - Convert timestamp fields to RFC3339
- `--show-ts-local` / `--show-ts-utc` - Display timestamp format
- `--take` - Limit output to N events

**Example:**
```bash
> kelora -j app.log --keys timestamp,service,message
> kelora -j app.log -F json
> kelora -j app.log --take 100
```

## Pipeline Characteristics

### Streaming

Kelora processes events one at a time (or in batches with `--parallel`):

- **Low memory usage** - Events are processed and discarded
- **Real-time processing** - Works with `tail -f` and live streams
- **No lookahead** - Can't access future events (except with `--window`)

### Sequential vs Parallel

**Sequential (default):**
- Events processed in order
- Lower memory usage
- Predictable output order
- Simpler debugging

**Parallel (`--parallel`):**
- Events processed in batches across cores
- Higher throughput
- May reorder output (use `--ordered` to preserve)
- Higher memory usage

### Event Lifecycle

```
1. Line read from input
2. Line parsed into event map
3. Event passes through filters
4. Event transformed by --exec scripts
5. Event formatted and output
6. Event discarded (unless windowed)
```

## Common Pipeline Patterns

### Simple Filter-and-Select

```bash
> kelora -j app.log \
    --filter 'e.level == "ERROR"' \
    --keys timestamp,service,message
```

Pipeline: Input → Parse → Filter → Output (with field selection)

### Filter-Transform-Output

```bash
> kelora -j app.log \
    --filter 'e.service == "api"' \
    --exec 'e.duration_s = e.duration_ms / 1000' \
    --keys timestamp,duration_s,path
```

Pipeline: Input → Parse → Filter → Transform → Output

### Aggregate and Report

```bash
> kelora -j app.log \
    --exec 'track_count(e.service)' \
    --exec 'track_avg("response_time", e.duration_ms)' \
    --metrics \
    -F none
```

Pipeline: Input → Parse → Transform (tracking) → Output (metrics only)

### Multi-Stage Filtering

```bash
> kelora -j app.log \
    --levels error,warn \
    --since "1 hour ago" \
    --filter 'e.service != "health-check"' \
    --keys timestamp,level,service,message
```

Pipeline: Input → Parse → Filter (levels) → Filter (time) → Filter (custom) → Output

### Fan-Out Processing

```bash
> kelora -j batch.log \
    --exec 'emit_each(e.items, #{batch_id: e.batch_id})' \
    --filter 'e.status == "active"' \
    --keys batch_id,item_id,status
```

Pipeline: Input → Parse → Transform (fan-out) → Filter → Output

## Error Handling in Pipeline

### Resilient Mode (Default)

- **Parse errors:** Skip line, continue processing
- **Filter errors:** Treat as `false`, skip event
- **Transform errors:** Return original event, continue
- **Summary:** Show error count at end

### Strict Mode (`--strict`)

- **Parse errors:** Show error, abort immediately
- **Filter errors:** Show error, abort immediately
- **Transform errors:** Show error, abort immediately
- **Summary:** No summary (aborted)

### Verbose Mode (`--verbose`)

- Shows each error immediately as it occurs
- Works in both resilient and strict modes
- Useful for debugging pipeline issues

## Pipeline State

### Stateless Processing

Most operations are stateless:
- Each event processed independently
- No memory of previous events
- Can process infinite streams

### Stateful Features

Some features maintain state:

**Metrics Tracking:**
```bash
> kelora -j app.log \
    --exec 'track_count(e.service)' \
    --metrics
```

Maintains counters in memory.

**Window Functions:**
```bash
> kelora -j app.log \
    --window 5 \
    --exec 'e.recent = window_values("status")'
```

Maintains sliding window of recent events.

**Context Lines:**
```bash
> kelora -j app.log \
    --filter 'e.level == "ERROR"' \
    --before-context 2
```

Buffers events to provide context.

## Pipeline Optimization

### Early Filtering

Place cheap filters early to reduce work:

```bash
# Good: Level filter before expensive regex
> kelora -j app.log \
    --levels error \
    --filter 'e.message.has_matches(r"complex.*pattern")'

# Less efficient: Expensive filter on all events
> kelora -j app.log \
    --filter 'e.message.has_matches(r"complex.*pattern")' \
    --filter 'e.level == "ERROR"'
```

### Selective Field Access

Use `--keys` to reduce output processing:

```bash
> kelora -j app.log \
    --keys timestamp,message \
    -F json
```

### Parallel Processing

Use `--parallel` for CPU-bound transformations:

```bash
> kelora -j large.log \
    --parallel \
    --exec 'e.hash = e.content.hash("sha256")' \
    --batch-size 1000
```

### Limiting Output

Use `--take` for quick exploration:

```bash
> kelora -j large.log --take 100
```

## See Also

- [Events and Fields](events-and-fields.md) - How events are structured
- [Scripting Stages](scripting-stages.md) - Deep dive into --begin/--exec/--end
- [Error Handling](error-handling.md) - Resilient vs strict modes
- [Performance Model](performance-model.md) - Sequential vs parallel processing
