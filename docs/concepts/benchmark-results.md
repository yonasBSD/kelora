# Benchmark Results

Raw performance benchmarks comparing Kelora against common command-line tools across different hardware. Throughput values are inferred directly from dataset size ÷ mean runtime (100k events per test unless noted; 500k for the parallel scenario).

!!! abstract "Looking for Guidance?"
    This page shows **raw benchmark data**. For interpretation, decision matrices, and honest performance guidance, see [Performance Comparisons](performance-comparisons.md).

---

## Test System: Apple M1 (2024-10-26)

**Hardware:**
- **CPU:** Apple M1
- **Cores:** 8 (4 performance + 4 efficiency)
- **OS:** macOS (Darwin)
- **RAM:** 16GB

**Software Versions:**
- kelora: (current development build)
- jq: 1.7.1
- ripgrep: 14.1.0
- awk: BSD awk (macOS built-in)
- miller: 6.12.0
- qsv: 0.131.0

---

### 1. Simple Text Filtering

**Task:** Find all ERROR lines in 100,000-line log file

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| grep | 0.048s | ~2.08 M/s | Baseline - fastest text search |
| ripgrep (rg) | 0.014s | ~7.14 M/s | Modern grep alternative |
| kelora (line) | 0.381s | ~0.26 M/s | Full log parsing + structured output |

**Interpretation:** grep/ripgrep are 8-27x faster for simple text matching.

---

### 2. Field Extraction

**Task:** Extract timestamp, level, component from 100,000 log lines

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| awk | 0.374s | ~0.27 M/s | Field splitting by whitespace |
| kelora (cols) | 4.723s | ~0.02 M/s | Structured parsing + type awareness |

**Interpretation:** awk is 12.6x faster for simple field extraction.

---

### 3. JSON Filtering

**Task:** Filter 100,000 JSON logs where level == 'ERROR' (25,256 matches)

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq | 1.056s | ~0.09 M/s | Standard JSON processor |
| kelora | 6.157s | ~0.02 M/s | JSON parsing with level filter |

**Interpretation:** jq is 5.8x faster for JSON filtering.

---

### 4. JSON Transformation

**Task:** Filter API logs, extract status code, add is_error field

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq | 0.763s | ~0.13 M/s | Complex jq query |
| kelora | 9.009s | ~0.01 M/s | Multi-stage pipeline |

**Interpretation:** jq is 11.8x faster for JSON transformations.

---

### 5. Complex Multi-Stage Pipeline

**Task:** Filter errors, count by component, sort by frequency

| Tool | Time | Throughput | Command Complexity |
|------|------|------------|-------------------|
| bash + jq + sort + uniq | 0.722s | ~0.14 M/s | `jq ... \| sort \| uniq -c \| sort -rn` |
| kelora | 6.786s | ~0.01 M/s | `kelora -l error --exec 'track_count(...)' --metrics` |

**Interpretation:** Multi-tool bash pipeline is 9.4x faster, but Kelora uses a single command with built-in aggregation.

---

### 6. Parallel Processing

**Task:** Process 500,000 JSON logs, filter by component

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq (sequential) | 4.426s | ~0.11 M/s | Single-threaded processing |
| kelora (sequential) | 42.717s | ~0.01 M/s | Single-threaded baseline |
| kelora (--parallel) | 8.143s | ~0.06 M/s | Multi-core processing (8 cores) |

**Interpretation:**
- Kelora sequential is 9.7x slower than jq
- Kelora `--parallel` provides 5.2x speedup over sequential
- Still 1.8x slower than jq overall

---

### 7. CSV Processing

**Task:** Filter CSV by level, select columns

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| miller | 0.147s | ~0.68 M/s | CSV swiss army knife |
| qsv | 0.050s | ~2.00 M/s | High-performance CSV tool |
| kelora | 5.787s | ~0.02 M/s | CSV + level filtering |

**Interpretation:** qsv is 115x faster, miller is 39x faster. Use specialized CSV tools for pure CSV work.

---

## Contributing Results

Have different hardware? Want to add Linux/Windows results? Contributions welcome!

**To add your results:**

1. Run benchmarks on your system:
   ```bash
   just bench-compare
   ```

2. Add a new section above with your system specs:
   ```markdown
   ## Test System: [Your System Name] (YYYY-MM-DD)
   ```

3. Submit a PR with your results

**Especially interested in:**
- Linux (x86_64, ARM)
- Windows (WSL, native)
- Different CPU architectures
- Older vs newer hardware comparisons

---

## Historical Results

Track performance improvements across Kelora versions:

| Version | Test | M1 Result | Intel Mac Result | Notes |
|---------|------|-----------|------------------|-------|
| 0.x.x | JSON Filter | 6.157s | 6.425s | Initial benchmark |
| (future) | | | | |

---

## Test System: Intel Core i5 (2025-10-26)

**Hardware:**
- **CPU:** Intel Core i5 (3.0 GHz, 6 cores)
- **Cores:** 6
- **OS:** macOS 15.6.1 (24G90) · `x86_64`
- **RAM:** 16 GB

**Software Versions:**
- Kelora: `target/release/kelora` @ `09faf43`
- Rust: 1.90.0
- grep: 2.6.0 (BSD)
- ripgrep: 15.1.0
- jq: 1.6
- mlr: 6.15.0
- qsv: 8.1.1

---

### 1. Simple Text Filtering

**Task:** Find all ERROR lines in 100,000-line log file

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| grep | 0.049s | ~2.04 M/s | Baseline - fastest text search |
| ripgrep (rg) | 0.021s | ~4.76 M/s | Modern grep alternative |
| kelora (line) | 0.411s | ~0.24 M/s | Full log parsing + structured output |

**Interpretation:** ripgrep is ~20× faster (≈2.3× faster than BSD `grep`, which Kelora trails by ~8×); the extra time goes into emitting structured events that make follow-up scripting trivial.

---

### 2. Field Extraction

**Task:** Extract timestamp, level, component from 100,000 log lines

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| awk | 0.389s | ~0.26 M/s | Field splitting by whitespace |
| kelora (cols) | 4.672s | ~0.02 M/s | Structured parsing + type awareness |

**Interpretation:** awk is ≈12× faster for pure column splits. Use Kelora when you need schema validation or type conversions downstream.

---

### 3. JSON Filtering

**Task:** Filter 100,000 JSON logs where level == 'ERROR'

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq | 1.101s | ~0.09 M/s | Standard JSON processor |
| kelora | 6.425s | ~0.02 M/s | JSON parsing with Rhai filter |

**Interpretation:** jq keeps a ~5.8× edge for simple filters. Kelora catches up when you chain multiple filters, metrics, or Rhai helpers.

---

### 4. JSON Transformation

**Task:** Filter API logs, extract status code, add is_error field

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq | 0.792s | ~0.13 M/s | Complex jq query |
| kelora | 9.203s | ~0.01 M/s | Multi-stage pipeline |

**Interpretation:** jq is ≈11.6× faster for single-expression transforms. Kelora shines when that logic spans multiple stages or needs custom functions.

---

### 5. Complex Multi-Stage Pipeline

**Task:** Filter errors, count by component, sort by frequency

| Tool | Time | Throughput | Command Complexity |
|------|------|------------|-------------------|
| bash + jq + sort + uniq | 0.724s | ~0.14 M/s | `jq ... \| sort \| uniq -c \| sort -rn` |
| kelora | 6.957s | ~0.01 M/s | `kelora -l error --exec 'track_count(...)' --metrics` |

**Interpretation:** The Unix pipeline is ~10× faster but spans multiple processes. Kelora trades those seconds for a single, maintainable command.

---

### 6. Parallel Processing

**Task:** Process 500,000 JSON logs, filter by component

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| jq (sequential) | 4.570s | ~0.11 M/s | Single-threaded processing |
| kelora (sequential) | 43.587s | ~0.01 M/s | Single-threaded baseline |
| kelora (--parallel) | 8.966s | ~0.06 M/s | Multi-core processing |

**Interpretation:** Kelora sequential is ~9.5× slower than jq, but `--parallel` recovers most of that gap (4.9× faster than sequential, only 2× slower than jq).

---

### 7. CSV Processing

**Task:** Filter CSV by level, select columns

| Tool | Time | Throughput | Notes |
|------|------|------------|-------|
| miller | 0.233s | ~0.43 M/s | CSV swiss army knife |
| qsv | 0.072s | ~1.39 M/s | High-performance CSV tool |
| kelora | 6.815s | ~0.01 M/s | CSV + Rhai scripting |

**Interpretation:** CSV-specialized tools dominate (Kelora is 95× slower than qsv). Use Kelora when you need to mix CSV with other formats or invoke Rhai logic inline.

---

### Raw Files

Source measurements live in `benchmarks/comparison_results/01_simple_filter.md` through `07_csv.md` from the same commit/date for full reproducibility.
