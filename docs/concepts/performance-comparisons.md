# Performance Comparisons

Understanding how Kelora compares to other command-line tools helps you choose the right tool for each task. This page provides honest benchmarks and guidance on when to reach for Kelora versus alternatives like grep, jq, awk, or specialized CSV tools.

!!! info "Philosophy: Right Tool for the Job"
    Kelora trades raw speed for **expressiveness and convenience**. For simple pattern matching, grep is often 10-100x faster. But when you need multi-stage pipelines, structured transformations, or windowed analysis, Kelora's unified scripting model can be both faster to write *and* faster to execute than complex bash pipelines.

## Quick Decision Matrix

| Task | Best Tool | Why | When to Use Kelora Instead |
|------|-----------|-----|---------------------------|
| Simple text search | `grep` / `rg` | 50-100x faster for pattern matching | When you need structured output or follow-up processing |
| Field extraction | `awk` / `cut` | Faster for simple column splits | When you need type conversion, validation, or enrichment |
| JSON filtering | `jq` | Comparable speed, ubiquitous | Multi-stage pipelines, metrics tracking, windowing |
| JSON transformation | `jq` | Similar performance for simple queries | Complex transforms using 100+ built-in functions |
| CSV analytics | `qsv` / `miller` | Often faster for pure CSV operations | When mixing CSV with other formats or scripting logic |
| Complex pipelines | Bash + multiple tools | Variable (often slower) | Always - simpler code, fewer moving parts |
| Large archive processing | Varies | - | When you can use `--parallel` for multi-core speedup |
| Windowed analysis | Custom scripts | Hard to compare | Built-in `--window` eliminates custom code |

!!! note "Raw benchmark files"
    Every table in this page is backed by `benchmarks/comparison_results/0X_*.md`. Regenerate them with `just bench-compare` to audit or extend the data.

## Benchmark Methodology

### Test Environment (2025-10-26)
- **OS:** macOS 15.6.1 (24G90) · `x86_64`
- **CPU:** Intel Core i5 · 6 cores @ 3.0 GHz · 16 GB RAM
- **Kelora build:** `cargo build --release` at `09faf43`
- **Toolchain:** grep 2.6.0 (BSD), ripgrep 15.1.0, jq 1.6, mlr 6.15.0, qsv 8.1.1, Rust 1.90.0 (stable)

### Procedure
- Generate deterministic 100k/500k JSONL fixtures plus derived CSV/log files via `benchmarks/generate_comparison_data.sh` (wraps `generate_test_data.py`).
- Run `./benchmarks/compare_tools.sh` (alias: `just bench-compare`). Each scenario executes three times; the script reports the arithmetic mean of the recorded wall-clock values (GNU `time` when available, otherwise `date` + `bc`).
- All commands stream to `/dev/null` (or minimal projections) so we measure processing cost rather than disk output.
- Raw markdown tables for every scenario are stored in `benchmarks/comparison_results/*.md` alongside the captured system metadata so results can be audited or re-plotted later.
- Throughput figures (`events/s`) in each table are simply dataset size divided by the mean runtime (100k lines unless noted; 500k for the parallel test).

---

## Benchmark Results

!!! tip "Running Your Own Benchmarks"
    ```bash
    brew install ripgrep jq miller qsv   # install peer tools
    cargo build --release                # build Kelora
    ./benchmarks/generate_comparison_data.sh
    just bench-compare                   # wraps ./benchmarks/compare_tools.sh
    ```
    Every run records the system info and scenario tables under `benchmarks/comparison_results/`. Copy those into [Benchmark Results](benchmark-results.md) when you contribute a new machine.

### 1. Simple Text Filtering

**Task:** Find all ERROR lines in 100,000-line log file

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| grep | 0.049 s | ~2.04 M/s | Baseline - fastest text search |
| ripgrep (rg) | 0.021 s | ~4.76 M/s | Modern grep alternative |
| kelora (line) | 0.411 s | ~0.24 M/s | Full log parsing + structured output |

**Example Commands:**
```bash
# grep - simple and fast
grep 'ERROR' benchmarks/bench_100k.log

# kelora - slower but structured
kelora -f line benchmarks/bench_100k.log \
  --keep-lines 'ERROR' -q
```

Kelora is roughly **20× slower than ripgrep** (which itself is ≈2.3× faster than BSD `grep`; Kelora is ~8× slower than `grep`) because it tokenizes each line and materializes structured events. Use `grep`/`rg` when you just need text, but once you need follow-up filters or projections the extra ~0.4 s buys you typed fields and immediate Rhai hooks. To minimize the gap, combine `--keep-lines` with `-q` so Kelora skips formatter work your downstream pipeline does not need.

---

### 2. Field Extraction

**Task:** Extract timestamp, level, component from 100,000 log lines

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| awk | 0.389 s | ~0.26 M/s | Field splitting by whitespace |
| kelora (cols) | 4.672 s | ~0.02 M/s | Structured parsing + type awareness |

**Example Commands:**
```bash
# awk - fast field extraction
awk '{print $1, $3, $4}' benchmarks/bench_100k.log

# kelora - named fields with validation
kelora -f 'cols:timestamp host component level *message' \
  benchmarks/bench_100k.log \
  -k timestamp,level,component
```

`awk` stays **≈12× faster** because it only slices bytes; Kelora parses into typed columns, validates timestamps, and keeps metadata for later stages. Stick with `awk`/`cut` for quick-and-dirty splits, but prefer Kelora when you immediately need type conversion, schema-aware errors, or downstream scripting. You can trim Kelora’s cost by projecting only the fields you need with `-k` and disabling expensive formatters (`-F none`).

---

### 3. JSON Filtering

**Task:** Filter 100,000 JSON logs where `level == 'ERROR'`

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq | 1.101 s | ~0.09 M/s | Standard JSON processor |
| kelora | 6.425 s | ~0.02 M/s | JSON parsing with Rhai filter |

**Example Commands:**
```bash
# jq - compact and fast
jq -c 'select(.level == "ERROR")' benchmarks/bench_100k.jsonl

# kelora - similar syntax
kelora -j benchmarks/bench_100k.jsonl \
  -l error -F json -q
```

`jq` leads by **~5.8×** for single-pass JSON filtering because it operates entirely in native C and streams compactly. Kelora pays overhead for Rhai context, metrics counters, and optional windowing. Reach for Kelora when you need that scripting environment (multiple `--filter` stages, stateful metrics) or want to mix formats. For pure filtering, enable `--no-emoji` and emit `-F json` to hold the gap to ~4× on this machine.

---

### 4. JSON Transformation

**Task:** Filter API logs, extract status code, add computed field

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq | 0.792 s | ~0.13 M/s | Complex jq query |
| kelora | 9.203 s | ~0.01 M/s | Multi-stage pipeline |

**Example Commands:**
```bash
# jq - single complex query
jq -c 'select(.component == "api") |
  {timestamp, method, status, is_error: (.status >= 400)}' \
  benchmarks/bench_100k.jsonl

# kelora - readable pipeline
kelora -j benchmarks/bench_100k.jsonl \
  --filter "e.component == 'api'" \
  --exec "e.is_error = (e.status.to_int() >= 400)" \
  -k timestamp,method,status,is_error -q
```

`jq` wins decisively (≈11×) for short declarative transforms. Kelora’s advantage shows up when the logic no longer fits into a single jq expression—multiple `--filter` passes, reusable `--exec` snippets, or when you need to call into Rhai helpers (`parse_jwt`, `track_percentile`, etc.). If you stay in Kelora, move heavy setup into `--begin` blocks and project only the final keys to keep execution closer to 6 – 7 s.

---

### 5. Complex Multi-Stage Pipeline

**Task:** Filter errors, count by component, sort by frequency

| Tool | Time | Throughput | Command Complexity |
|------|------|------------|-------------------|
| bash + jq + sort + uniq | 0.724 s | ~0.14 M/s | `jq ... \| sort \| uniq -c \| sort -rn` |
| kelora | 6.957 s | ~0.01 M/s | `kelora --filter ... --exec 'track_count(...)' --metrics` |

**Example Commands:**
```bash
# Bash pipeline - multiple tools
jq -r 'select(.level == "ERROR") | .component' \
  benchmarks/bench_100k.jsonl | \
  sort | uniq -c | sort -rn

# Kelora - single command
kelora -j benchmarks/bench_100k.jsonl \
  -l error \
  --exec "track_count(e.component)" \
  --metrics -F none -q
```

This scenario highlights ergonomics over raw speed: the four-stage bash pipeline is ∼10× faster, but Kelora expresses the same logic in one command that is easier to evolve (add metrics, export JSON, window counts). If you are scripting or automating, absorbing the extra six seconds is usually worth the readability. For throwaway shell use, the Unix toolkit still wins.

---

### 6. Parallel Processing

**Task:** Process 500,000 JSON logs with filtering

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq (sequential) | 4.570 s | ~0.11 M/s | Single-threaded processing |
| kelora (sequential) | 43.587 s | ~0.01 M/s | Single-threaded baseline |
| kelora (--parallel) | 8.966 s | ~0.06 M/s | Multi-core processing |

**Example Commands:**
```bash
# jq - single threaded
jq -c 'select(.component == "api")' \
  benchmarks/bench_500k.jsonl | wc -l

# kelora - sequential
kelora -j benchmarks/bench_500k.jsonl \
  --filter "e.component == 'api'" -F json -q

# kelora - parallel (auto-scales to CPU cores)
kelora -j benchmarks/bench_500k.jsonl \
  --filter "e.component == 'api'" --parallel -F json -q
```

Kelora’s interpreter overhead is obvious in the sequential run (≈10× slower than jq), but turning on `--parallel` claws back the advantage—processing drops from 43.6 s to 9.0 s on this 6‑core machine, only ~2× slower than jq while delivering richer scripting. Use this pattern for archival replays: set `--parallel`, keep `--batch-size` near cache-friendly values (1 000–4 000), and pin `--threads` to your physical cores if the workload competes with other services.

---

### 7. CSV Processing

**Task:** Filter CSV by level, select specific columns

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| miller | 0.233 s | ~0.43 M/s | CSV swiss army knife |
| qsv | 0.072 s | ~1.39 M/s | High-performance CSV tool |
| kelora | 6.815 s | ~0.01 M/s | CSV + Rhai scripting |

**Example Commands:**
```bash
# miller - powerful CSV transforms
mlr --csv filter '$level == "ERROR"' \
  then cut -f timestamp,level,component \
  benchmarks/bench_100k.csv

# qsv - blazing fast
qsv search -s level 'ERROR' benchmarks/bench_100k.csv | \
  qsv select timestamp,level,component

# kelora - scriptable CSV
kelora -f csv benchmarks/bench_100k.csv \
  -l error \
  -k timestamp,level,component -q
```

`qsv` simply screams on CSV—Kelora is ~95× slower here because it still instantiates full events and Rhai scopes. When you only touch CSV/TSV, run `qsv` or `mlr` and hand the results to Kelora only for the structured steps they cannot express. Kelora becomes attractive as soon as you need to join CSV with JSON/syslog, mask fields with built-ins, or emit a single, scriptable pipeline for your team.

---

## When Kelora Shines

### 1. Multi-Format Pipelines

**Problem:** Parse syslog messages that contain embedded logfmt

```bash
# With grep/awk/jq - messy and fragile
grep "user=" syslog.log | \
  sed 's/.*msg="\([^"]*\)".*/\1/' | \
  # ...complex parsing...

# With Kelora - clean and explicit
kelora -f syslog syslog.log \
  --exec 'if e.msg.contains("=") {
            e += e.msg.parse_logfmt()
          }' \
  --filter 'e.has_field("user")' \
  -k timestamp,host,user,action
```

### 2. Windowed Analysis

**Problem:** Detect error bursts (3+ errors in 60 seconds)

```bash
# With awk - complex state management
awk '...[50 lines of window logic]...'

# With Kelora - built-in windows
kelora -j app.jsonl --window 60 \
  --exec 'let errors = window_values(window, "level")
            .filter(|x| x == "ERROR");
          if errors.len() >= 3 {
            eprint("Burst at " + e.timestamp)
          }'
```

### 3. Enrichment & Privacy

**Problem:** Mask IPs, parse JWTs, add computed fields

```bash
# Kelora's built-in functions make this trivial
kelora -j security.jsonl \
  --exec 'e.ip = e.ip.mask_ip(2);
          if e.has_field("token") {
            let jwt = e.token.parse_jwt();
            e.role = jwt.get_path("claims.role", "guest")
          }'
```

---

## When to Use Something Else

### Use `grep` when:
- Simple pattern matching is all you need
- Speed is absolutely critical
- Output is for human eyes, not further processing

### Use `jq` when:
- Pure JSON transformations
- You need jq's advanced tree manipulation
- Integrating with existing jq-based workflows

### Use `awk` when:
- Simple field extraction from delimited files
- You're more familiar with awk's syntax
- Legacy scripts already use it

### Use `qsv` / `miller` when:
- Working exclusively with CSV/TSV
- Need specialized CSV operations (joins, statistics, sampling)
- Performance on CSV is paramount

### Use `lnav` / `SQLite` / `DuckDB` when:
- Interactive exploration and ad-hoc queries
- Need SQL for complex aggregations
- Building dashboards or reports

---

## Performance Tips

From fastest to slowest approaches:

### ✅ Fastest: Pre-filter before complex processing
```bash
# Filter cheap fields first
kelora -j huge.jsonl \
  --filter "e.level == 'ERROR'" \
  --exec "expensive_parsing(e)" \
  --filter "e.computed > 100"
```

### ⚡ Fast: Use parallel mode for CPU-bound work
```bash
kelora -j archive.jsonl --parallel --threads 0 \
  --exec "e.parsed = e.msg.parse_custom_format()"
```

### 🐌 Slower: Complex regex on every event
```bash
# Move static setup to --begin if possible
kelora --exec "e.extracted = complex_regex(e.line)"
```

See [Performance Model](performance-model.md) for deep dive on tuning.

---

## Reproduction Guide

1. **Install comparison tools.** `brew install ripgrep jq miller qsv` (macOS) or the equivalent packages via `apt`, `dnf`, etc.
2. **Build Kelora in release mode.**
   ```bash
   cargo build --release
   ```
3. **Generate the deterministic datasets.**
   ```bash
   ./benchmarks/generate_comparison_data.sh
   ```
   This script ensures the 100k/500k JSONL fixtures plus CSV/syslog derivatives match what the docs describe.
4. **Run the comparison suite.**
   ```bash
   just bench-compare    # wraps ./benchmarks/compare_tools.sh
   ```
   The runner prints human-friendly summaries and writes raw markdown tables to `benchmarks/comparison_results/0*.md`.
5. **Update the docs.** Copy the system-info block and any new tables into `docs/concepts/benchmark-results/` (one file per machine) and open a PR that references your hardware + date.

Every rerun should state the git commit, OS version, CPU, and tool versions so readers can compare apples to apples.

---

## Related Documentation

- **[Performance Model](performance-model.md)** - Kelora's internal performance characteristics, tuning `--parallel` and `--batch-size`
- **[How-To: Batch Process Archives](../how-to/batch-process-archives.md)** - Practical guide to parallel processing and maximizing throughput
- **[CLI Reference](../reference/cli-reference.md)** - Complete flag documentation
- **[Functions Reference](../reference/functions.md)** - All 100+ built-in functions

---

## Contributing Benchmarks

Have a different machine? Want to test additional tools? Benchmark contributions are welcome!

1. Run `./benchmarks/compare_tools.sh` on your machine
2. Save results with hardware info
3. Submit a PR with your results in `docs/concepts/benchmark-results/`

We're especially interested in:
- Different CPU architectures (ARM, x86, RISC-V)
- Linux vs macOS vs Windows
- Comparison with additional tools (angle, pv+awk, etc.)
