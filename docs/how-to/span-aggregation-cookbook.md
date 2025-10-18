# Span Aggregation Cookbook

Use `--span` and `--span-close` to batch events for summaries, dashboards, and incident analyses.

## Problem

You need periodic rollups (per N events or per time window) without losing the per-event pipeline logic. You also want late or malformed timestamps handled gracefully.

## Solutions

### Count-Based Batches

Split the stream every _N_ events that survive filters and emit a summary per batch.

```bash
kelora -f json access.log \
  --filter 'e.status >= 400' \
  --span 500 \
  --span-close '
    let errors = span.size;
    print(span.id + ",errors=" + errors.to_string());
  '
```

- Works in arrival order; no late-event concept.
- `span.size` reflects post-filter events, so remove empty spans if needed:

```rhai
if span.size > 0 {
    print(span.id + ":" + span.size);
}
```

### Time-Based Windows

Emit metrics aligned to fixed wall-clock windows by duration.

```bash
kelora -f json --span 5m app.log \
  --span-close '
    let metrics = span.metrics;
    let hits = metrics.get_path("hits", 0);
    let slow = metrics.get_path("slow", 0);
    let rate = if hits > 0 { slow * 100 / hits } else { 0 };
    print(span.id + "," +
          "hits=" + hits.to_string() + "," +
          "slow_pct=" + rate.to_string());
  ' \
  --exec '
    track_count("hits");
    if e.duration_ms > 2000 { track_count("slow"); }
  '
```

Best practices:

- Sort or pre-group logs by timestamp for accurate window assignment.
- Events lacking `ts` appear with `meta.span_status == "unassigned"` and do not enter `span.events`.
- Late arrivals keep `meta.span_status == "late"` and include the window they missed.

### Inspect Span Events

`span.events` gives the surviving events so you can generate rollups or trace context.

```bash
kelora -f json --span 100 requests.jsonl \
  --span-close '
    let events = span.events;
    let ids = events.map(|evt| evt.request_id).join(",");
    print(span.id + ": " + ids);
  '
```

The maps inside `span.events` include all original fields plus span metadata (`span_status`, `span_start`, `span_end`).

### Track Metrics Automatically

`span.metrics` isolates the deltas for `track_*` calls inside the span.

```bash
kelora -f json --span 1m api.log \
  --exec '
    track_count("total");
    if e.level == "ERROR" { track_count("errors"); }
  ' \
  --span-close '
    let metrics = span.metrics;
    let total = metrics.get_path("total", 0);
    let errors = metrics.get_path("errors", 0);
    if total > 0 {
      let err_rate = errors * 100 / total;
      print(span.id + ",error_rate=" + err_rate.to_string());
    }
  '
```

No manual resets required—Kelora snapshots and clears span metrics after each close.

### Handle Late or Missing Timestamps

```bash
kelora -f json --span 10m logs.jsonl \
  --exec '
    if meta.span_status == "late" {
      eprint("Late: " + meta.span_id + " ← " + e.ts);
    } else if meta.span_status == "unassigned" {
      eprint("Missing ts: line " + meta.line_num.to_string());
    }
  ' \
  --span-close 'print(span.id + ":" + span.size)'
```

- `meta.span_status` is visible in `--exec` stages, so you can branch on late/unassigned events.
- Late events still pass through filters/exec but do not reopen closed spans.
- For strict timestamp enforcement, add `--strict` (events without valid `ts` become hard errors).

### Use Span Metadata Without a Close Hook

Skip `--span-close` when you only need Kelora to tag events; the span processor stays lightweight but emits per-event metadata you can forward downstream.

```bash
kelora -f json access.log --span 5m \
  --exec 'e.window = #{id: meta.span_id, start: meta.span_start, end: meta.span_end};'
```

Every event now carries its tumbling window so tools like `jq`, DuckDB, or spreadsheets can group by `e.window.id`.

```bash
kelora -f json errors.log --span 2m --output jsonl |
  jq -sc 'group_by(.meta.span_id)
          | map({span: .[0].meta.span_id, errors: length})'
```

Or filter in-line by span status while still emitting the rest:

```bash
kelora -f json access.log --span 5m \
  --filter 'meta.span_status != "late"' \
  --exec 'if meta.span_status == "late" { eprint("Late: " + e.ts + " → " + meta.span_id); }'
```

### Tips

- Time spans align to the first valid timestamp in the stream; ensure ordering or use upstream sort.
- Count spans retain all events in memory until `span.size` is reached. Watch for large values.
- `--span` disables parallel mode automatically—sequential processing is required for deterministic batching.
