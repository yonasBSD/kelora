# Metrics and Tracking

Turn raw log streams into actionable numbers. This tutorial walks through Kelora's
metrics pipeline, from basic counters to custom summaries that you can export or
feed into dashboards.

## What You'll Learn

- Track counts, sums, buckets, and unique values with Rhai helpers
- Combine `--metrics`, `--stats`, `--begin`, and `--end` for structured reports
- Use sliding windows and percentiles for latency analysis
- Persist metrics to disk for downstream processing

## Prerequisites

- Completed the [Quickstart](../quickstart.md)
- Familiarity with basic Rhai scripting (`--filter`, `--exec`)

## Sample Data

Commands below use fixtures from the repository. If you cloned the project, the
paths resolve relative to the docs root:

- `examples/simple_json.jsonl` — mixed application logs
- `examples/window_metrics.jsonl` — high-frequency metric samples
- `examples/web_access_large.log.gz` — compressed access logs for batch jobs

All commands print real output thanks to `markdown-exec`; feel free to tweak the
expressions and rerun them locally.

## Step 1 – Quick Counts with `track_count()`

Count how many events belong to each service while suppressing event output.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  -F none \
  --exec 'track_count(e.service)' \
  --metrics
```

`--metrics` prints the aggregated map when processing finishes. Use this pattern
any time you want a quick histogram after a batch run.

### Showing Stats at the Same Time

Pair `--metrics` with `--stats` when you need throughput details as well:

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  -F none \
  --exec 'track_count(e.service)' \
  --metrics --stats
```

`--stats` adds processing totals, time span, and field inventory without touching
your metrics map.

## Step 2 – Summaries with Sums, Buckets, and Averages

Kelora ships several helpers for numeric metrics. The following example treats
response sizes and latency as rolling aggregates.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  -F none \
  --exec 'track_sum("response_bytes", to_int_or(e.get_path("bytes"), 0))' \
  --exec 'track_avg("response_time_ms", to_int_or(e.get_path("duration_ms"), 0))' \
  --exec 'if e.has_path("duration_ms") { track_bucket("slow_requests", clamp(to_int_or(e.duration_ms, 0) / 250 * 250, 0, 2000)) }' \
  --metrics
```

- `track_sum` accumulates totals (suitable for throughput or volume).
- `track_avg` automatically maintains a running average per key.
- `track_bucket` groups values into ranges so you can build histograms.

Buckets show up as nested maps where each bucket value keeps its own count.

## Step 3 – Unique Values and Cardinality

`track_unique()` stores distinct values for a key—handy for unique user counts or
cardinality analysis.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  -F none \
  --exec 'track_unique("services", e.service)' \
  --exec 'if e.level == "ERROR" { track_unique("error_messages", e.message) }' \
  --metrics
```

Use `metrics["services"].len()` later to compute the number of distinct members.

## Step 4 – Sliding Windows and Percentiles

Enable the window buffer to examine recent events. The example below tracks a
five-event moving average and P95 latency for CPU metrics.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/window_metrics.jsonl \
  --filter 'e.metric == "cpu"' \
  --window 5 \
  --exec $'let values = window_numbers(window, "value");
if values.len() > 0 {
    let sum = values.reduce(|s, x| s + x, 0.0);
    let avg = sum / values.len();
    e.avg_last_5 = round(avg * 100.0) / 100.0;
    if values.len() >= 3 {
        e.p95_last_5 = round(values.percentile(95.0) * 100.0) / 100.0;
    }
}' \
  --take 5
```

The special `window` variable becomes available once you pass `--window`. Use
`window_numbers(window, FIELD)` for numeric arrays and `window_values(window, FIELD)`
for raw strings.

## Step 5 – Custom Reports with `--end`

Sometimes you need a formatted report instead of raw maps. Store a short Rhai
script and invoke it with `--end-file` so the same layout works across platforms.

```bash exec="on" source="above" result="ansi"
cat <<'RHAI' > metrics_summary.rhai
fn summarize_metrics() {
    let keys = metrics.keys();
    keys.sort();
    for key in keys {
        if !key.starts_with("__op_") {
            print(key + ": " + metrics[key].to_string());
        }
    }
}
RHAI

kelora -f json examples/simple_json.jsonl \
  -F none \
  --exec 'track_count(e.service)' \
  --exec 'track_count(e.level)' \
  --metrics \
  -I metrics_summary.rhai \
  --end 'summarize_metrics()'

rm metrics_summary.rhai
```

The automatically printed `--metrics` block remains, while `--end` gives you a
clean text summary that you can redirect or feed into alerts.

## Step 6 – Persist Metrics to Disk

Use `--metrics-file` to serialize the metrics map as JSON for other tools.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  -F none \
  --exec 'track_count(e.service)' \
  --metrics \
  --metrics-file metrics.json

cat metrics.json
rm metrics.json
```

The JSON structure mirrors the in-memory map, so you can load it with `jq`, a
dashboard agent, or any scripting language.

## Step 7 – Streaming Scoreboards

Kelora keeps metrics up to date even when tailing files or processing archives.
This command watches a gzipped access log and surfaces top status classes.

```bash exec="on" source="above" result="ansi"
kelora -f combined examples/web_access_large.log.gz \
  --exec 'let klass = ((e.status / 100) * 100).to_string(); track_count(klass)' \
  --metrics -F none \
  --take 0
```

Passing `--take 0` (or omitting it) keeps processing the entire file. When you run
Kelora against a stream (`tail -f | kelora ...`), the metrics snapshot updates when
you terminate the process.

Need full histograms instead of counts? Swap in `track_bucket()`:

```bash exec="on" source="above" result="ansi"
kelora -f combined examples/web_access_large.log.gz \
  --metrics \
  --exec 'track_bucket("status_family", (e.status / 100) * 100)' \
  --end '
    for bucket in metrics.status_family.keys() {
        let counts = metrics.status_family[bucket];
        print(bucket.to_string() + ": " + counts.to_string());
    }
  ' \
  -F none --take 0
```

`track_bucket(key, bucket_value)` keeps nested counters so you can emit a
human-readable histogram once processing finishes.

## Troubleshooting

- **No metrics printed**: Ensure you pass `--metrics` or consume `metrics` within
  an `--end` script. Tracking functions alone do not emit output.
- **Huge maps**: Reset counters between runs by clearing your terminal or using
  `rm metrics.json` when exporting to disk. Large cardinality sets from
  `track_unique()` are the usual culprit.
- **Unexpected `__op_*` keys**: Kelora stores internal operator metadata
  alongside user metrics. Filter them (as in the `metrics_summary.rhai` script)
  before displaying the report.
- **Sliding window functions return empty arrays**: `window_numbers(window, ...)`
  only works after you enable `--window` and the requested field exists in the
  buffered events.

## Next Steps

- Dive into the [Scripting Transforms tutorial](scripting-transforms.md) for more
  transformation patterns.
- Read the [Concepts: Scripting Stages](../concepts/scripting-stages.md) page to
  understand how `--begin`, `--exec`, and `--end` interact.
- Explore the [Function Reference](../reference/functions.md#tracking-functions)
  for the complete list of tracking helpers and their signatures.
