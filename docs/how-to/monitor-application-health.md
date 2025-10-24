# Build a Service Health Snapshot

Create a concise operational report from JSON service logs, tracking errors, latency, and resource pressure without leaving the terminal.

## When to Reach for This
- You need a quick health summary after a deployment or incident.
- Product teams request a daily/weekly status report built from application logs.
- You are validating that a suspected fix reduced error rates or latency.

## Before You Start
- Example commands use `examples/simple_json.jsonl`. Replace it with your application logs.
- Ensure key fields exist in the payload (e.g., `service`, `level`, `duration_ms`, `memory_percent`, `status`).
- Rhai metric helpers (`track_count`, `track_avg`, `track_sum`, etc.) power the summary; see [Tutorial: Metrics and Tracking](../tutorials/metrics-and-tracking.md) if you need a refresher.

## Step 1: Build a Baseline Scoreboard
Count events by service and severity to frame the rest of the investigation.

```bash
kelora -j examples/simple_json.jsonl \
  -e 'track_count(e.service)' \
  -e 'track_count("level_" + e.level)' \
  --metrics \
  --stats
```

- `--metrics` prints the tracked values once input completes.
- `--stats` reports parse errors and throughput, giving you confidence in the numbers.

## Step 2: Add Performance Signals
Track latency and resource metrics with averages and extremes per service.

```bash
kelora -j examples/simple_json.jsonl \
  -e 'if e.has_path("duration_ms") {
        track_avg(e.service + "_latency_ms", e.duration_ms);
        track_max(e.service + "_latency_p99", e.duration_ms);
      }' \
  -e 'if e.has_path("memory_percent") {
        track_max(e.service + "_memory_peak", e.memory_percent);
      }' \
  --metrics
```

Guidance:
- Use safe guards (`e.has_path`) to avoid failing when fields are missing.
- Track additional business KPIs (orders, sign-ups) with `track_sum()` or `track_count()` as needed.

## Step 3: Flag Error Hotspots
Separate healthy traffic from incidents and identify recurring failure modes.

```bash
kelora -j examples/simple_json.jsonl \
  -l error,critical \
  -e 'let code = e.get_path("error.code", "unknown");' \
  -e 'track_count("errors|" + e.service)' \
  -e 'track_count("error_code|" + code)' \
  -k timestamp,service,message \
  --metrics
```

- Running the same pipeline without `-k` is useful when you only need counts.
- Combine multiple passes (baseline + errors) to build an overall picture or wrap the logic in a tiny shell script.

## Step 4: Generate a Shareable Summary
Create a compact report for status updates or documentation.

```bash
kelora -j examples/simple_json.jsonl \
  -e 'track_count(e.service)' \
  -e 'if e.level == "ERROR" { track_count("errors_" + e.service) }' \
  -e 'if e.has_path("duration_ms") {
        track_avg("latency_" + e.service, e.duration_ms);
      }' \
  -m \
  --end '
    print("=== Service Snapshot ===");
    for (key, value) in metrics {
      print(key + ": " + value.to_string());
    }
  '
```

- `-m` (or `--metrics`) keeps the metrics map available in the `--end` block.
- Redirect the output to a file (`> reports/service-health.txt`) or pipe it into notification tooling.

## Step 5: Export Structured Data
Share filtered events with teams that need to deep dive.

```bash
kelora -j examples/simple_json.jsonl \
  --filter 'e.level == "ERROR"' \
  -e 'e.error_code = e.get_path("error.code", "unknown")' \
  -k timestamp,service,error_code,message \
  -F csv > service-errors.csv
```

- Use `-J` or `-F json` for downstream analytics pipelines.
- When privacy matters, combine with [Sanitize Logs Before Sharing](extract-and-mask-sensitive-data.md).

## Variations
- **Per-service drilldown**  
  ```bash
  kelora -j app.log \
    --filter 'e.service == "payments"' \
    -e 'track_count(e.level)' \
    -e 'track_avg("payments_latency", e.get_path("duration_ms", 0))' \
    --metrics
  ```
- **Time-boxed reports**  
  ```bash
  kelora -j app.log \
    --since "2 hours ago" \
    -e 'e.window = e.timestamp.format("%Y-%m-%d %H:00")' \
    -e 'track_count(e.window)' \
    --metrics
  ```
- **Live monitoring**  
  ```bash
  tail -f /var/log/app.log | kelora -j -q \
    -e 'track_count(e.service)' \
    -e 'if e.level == "ERROR" { eprint("ALERT: error in " + e.service) }'
  ```
  Add `--no-emoji` when piping into systems that cannot render emoji.

## Validate and Iterate
- Compare counts against existing dashboards to ensure your pipeline includes every service instance.
- Make `--stats` part of the report so readers see parse failures or skipped events.
- If metrics explode after a deploy, combine this guide with [Triage Production Errors](find-errors-in-logs.md) to capture concrete samples.

## See Also
- [Tutorial: Metrics and Tracking](../tutorials/metrics-and-tracking.md) for a deeper tour of Rhai telemetry helpers.
- [Design Streaming Alerts](build-streaming-alerts.md) to turn the snapshot into a continuous monitor.
- [Process Archives at Scale](batch-process-archives.md) when historical context is required.
