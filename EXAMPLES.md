# Examples

Kelora processes messy, real-world logs using scripts you control. These examples show how to filter, structure, enrich, and analyze logs using Kelora’s CLI and Rhai scripting — with practical explanations for each feature.

---

## 1. 📃 Parsing and Structuring Unstructured Logs

Turn raw or semi-structured input into fields you can analyze.

```bash
# Filter critical syslog messages
kelora -f syslog /var/log/syslog -l crit,error,alert

# Parse logfmt entries (key=value)
kelora -f logfmt app.log -l error

# Parse CSV without headers using default column names and display only two of them
kelora -f csvnh data.csv --filter 'c3.to_int() > 100' --keys c1,c3
```

---

## 2. 📱 Real-Time Log Streaming

Stream logs from tail, kubectl, or stdin for live triage.

```bash
# Monitor logs from tail
tail -f /var/log/syslog | kelora -f syslog -l warn,error

# Live triage from Kubernetes, summary printed after CTRL-C
kubectl logs -f mypod | kelora -f logfmt --exec 'track_count(e.level)' --metrics
```

Kelora works naturally in UNIX pipelines. Its stream-first design makes it well-suited for continuous log monitoring and alerting logic.

---

## 3. 🔍 Basic Filtering & Field Selection

Start simple: filtering logs, selecting fields, and formatting output.

```bash
# Show only server errors (status 500+)
kelora -f json app.log --filter 'e.status >= 500'

# Limit to warning and error levels
kelora -f json app.log -l warn,error

# Extract a subset of fields as CSV
kelora -f json app.log --keys ts,method,path -F csv

# Spot bursts of warnings or errors with the level map formatter
kelora -f logfmt app.log -F levelmap
```

These use simple conditions (`--filter`, `--levels`/`-l`) and the `--keys`/`-k` flag to narrow down both the *events* and the *fields* in your output. The `levelmap` formatter fills the available terminal width, prefixing each block with the first event's timestamp so you can spot bursts of specific levels at a glance.

ℹ️ You can chain multiple `--filter` and `--exec` flags — each one runs in order, forming a processing pipeline.

---

## 4. 🤎 Enriching Events with Rhai

Use `--exec` to define custom logic per event and create new fields.

```bash
# Add a status class like "2xx", "5xx", etc.
kelora -f json app.log --exec 'e.class = status_class(e.status)' --keys status,class
```

```bash
# Tag slow requests with a label
kelora -f json app.log \
  --exec 'e.label = if e.response_time.to_int() > 1000 { "slow" } else { "ok" }' \
  --keys method,path,response_time,label
```

Rhai lets you write conditional logic or transformations inline. Fields you assign to the `e` object become part of the event output if referenced via `--keys`.

---

## 5. 🕵️ Forensics & Anomaly Detection

Use filters and scripts to spot suspicious patterns in your logs.

```bash
# Show login attempts from public IPs
kelora -f json auth.log --filter 'e.ip.is_private_ip() == false'
```

```bash
# Compute and print session durations
kelora -f json sessions.log \
  --exec 'let dur = to_datetime(end) - to_datetime(start); print("Duration: " + dur.as_seconds() + "s")'
```

Built-in helpers like `is_private_ip()` and `to_datetime()` make it easy to detect irregularities and compute derived values.

---

## 6. ⏱ Timestamp & Duration Handling

Work with times and durations using built-in parsing and arithmetic.

```bash
# Filter events during business hours
kelora -f json app.log \
  --exec 'let dt = to_datetime(e.ts); if dt.hour() >= 9 && dt.hour() < 17 { print("Work hour") }'
```

```bash
# Flag requests taking longer than 1 second
kelora -f json access.log \
  --exec 'let dur = to_duration(e.latency); if dur > dur_from_seconds(1) { print("Slow: " + dur.as_milliseconds() + "ms") }'
```

You can parse ISO timestamps or human-friendly durations and apply full arithmetic or formatting.

---

## 7. 🔄 Windowed Event Correlation

Enable context-aware event logic with `--window`.

```bash
# Detect 3 consecutive errors
kelora -f json app.log --window 3 \
  --filter 'e.message.contains("error")' \
  --exec 'if window.len() > 2 { eprint("3 errors in a row") }'
```

```bash
# Identify rising CPU trend
kelora -f json metrics.log --window 4 \
  --exec 'let vals = window_numbers(window, "cpu"); if vals.len() >= 3 && vals[0] > vals[1] && vals[1] > vals[2] { print("CPU rising") }'
```

The `window` array gives access to the current and N prior events. Use `window_values()` or `window_numbers()` to extract fields across them.

ℹ️ `--levels/-l` is applied after all processing stages. To filter window input, use `--filter` before `--exec`.

---

## 8. 📊 Tracking & Summarization

Use `track_*()` functions for counting, bucketing, and summarizing.

```bash
# Count events by log level
kelora -f json app.log --exec 'track_count(e.level)' --metrics

# Track unique users
kelora -f json app.log --exec 'track_unique("users", e.user)' --metrics
```

The `--metrics` flag shows tracked data after processing. These are global analytics — not per event.

---

## 9. 🚀 Parallel & Multi-file Processing

Scale up with batching, file-ordering, and parallelism.

```bash
# Process files by modification time
kelora -f json --file-order mtime logs/*.json

# Run in parallel mode with summary counts
kelora -f json app.log --parallel --exec 'track_count(e.level)' --metrics
```

Parallel mode improves throughput and allows real-time batch analysis. Use `--unordered` for maximum performance if order doesn't matter.

---

## 10. 🧹 Skipping Noise & Ignoring Lines

Preprocess logs to remove headers, comments, or irrelevant lines.

```bash
# Ignore comment lines and blanks
kelora -f line config.log --ignore-lines '^#|^\s*$'

# Raw regex-style filtering on text input
kelora -f line access.log --filter 'e.line.contains("404")'
```

Use `--ignore-lines` as a preprocessing step, especially for mixed or semi-structured formats.

---

## 11. 🧪 Validation & Benchmarking

Show how to validate scripts and run fast, silent benchmarks.

```bash
# Validate Rhai script with no input
kelora --exec-file script.rhai -F none < /dev/null

# Measure speed of a filtering expression
time kelora -f json huge.log --filter 'e.status >= 500' -F none
```

Using `-F none` disables all event output — useful for script linting or performance checks.
