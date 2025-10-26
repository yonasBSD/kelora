# Kelora Performance Benchmark Suite

A comprehensive benchmarking system for both internal performance testing and external tool comparisons.

## Quick Start

### Internal Benchmarks (Regression Testing)

```bash
# Run quick benchmarks (50k dataset)
just bench-quick

# Run full benchmark suite (100k + 500k datasets)
just bench

# Update performance baseline
just bench-update
```

### External Tool Comparisons

```bash
# Generate comparison datasets (CSV, syslog formats)
just bench-datasets

# Run comparisons against grep, jq, awk, miller, etc.
just bench-compare

# Run all benchmarks (internal + external)
just bench-all
```

## Benchmark Tests

| Test | Dataset | Description |
|------|---------|-------------|
| **small_filter** | 100k lines | Filter ERROR level logs - tests basic filtering performance |
| **medium_processing** | 500k lines | Filter + tracking - tests exec scripts and state management |
| **large_parallel** | 500k lines | Parallel processing with 4 threads - tests concurrency |
| **sequential_throughput** | 500k lines | Raw sequential processing - tests baseline throughput |

## Test Data

Datasets are **generated on-demand** to avoid bloating the repository:

- **bench_50k.jsonl**: 50,000 synthetic log entries (~11MB) - for quick tests
- **bench_100k.jsonl**: 100,000 synthetic log entries (~23MB) - for small_filter test
- **bench_500k.jsonl**: 500,000 synthetic log entries (~113MB) - for full benchmarks

Generated with realistic log structures including:
- Multiple log levels (DEBUG, INFO, WARN, ERROR)
- Various components (api, database, auth, cache, etc.)
- Structured fields for filtering and processing

## Usage Examples

### Basic Benchmarking
```bash
# Quick development cycle benchmarks
make bench-quick

# Compare against baseline
./benchmarks/run_benchmarks.sh
```

### Setting Baselines
```bash
# After implementing optimizations
make bench-update
```

### CI Integration
```bash
# In CI pipeline - fails if >10% regression
./benchmarks/run_benchmarks.sh --quick
if [ $? -ne 0 ]; then
    echo "Performance regression detected!"
    exit 1
fi
```

## Output

Results are saved to `benchmarks/benchmark_results.json`:

```json
{
  "timestamp": "2025-06-27T09:13:54Z",
  "results": [
    {
      "test": "small_filter",
      "runs": 3,
      "times": [0.012, 0.011, 0.012],
      "avg_time": 0.012,
      "min_time": 0.011,
      "max_time": 0.012,
      "timestamp": "2025-06-27T09:13:54Z"
    }
  ]
}
```

## Baseline Comparison

The benchmark system automatically compares results against a baseline stored in `benchmarks/baseline_results.json`:

- **Green**: >5% improvement
- **Yellow**: <±10% change (acceptable)
- **Red**: >10% regression (needs investigation)

## Development Workflow

1. **Before changes**: `make bench-baseline` (establish baseline)
2. **Make changes**: Implement optimizations or features
3. **Test**: `make bench-quick` (verify no major regressions)
4. **Full validation**: `make bench` (comprehensive test)
5. **Update baseline**: `make bench-update` (if improvements are intentional)

## File Structure

```
benchmarks/
├── README.md                 # This documentation
├── run_benchmarks.sh         # Main benchmark runner
├── generate_test_data.py     # Test data generator
├── bench_10k.jsonl          # 10k test dataset
├── bench_50k.jsonl          # 50k test dataset
├── benchmark_results.json   # Latest results
└── baseline_results.json    # Baseline for comparison
```

## Performance Expectations

Based on current optimizations (MacBook with SSD):

| Test | Expected Time | Notes |
|------|---------------|-------|
| **small_filter** | ~0.12s | Fast filtering on 100k dataset (includes cold start) |
| **medium_processing** | ~0.01s | Filter + tracking (highly selective filter) |
| **large_parallel** | ~0.01s | Parallel processing (highly selective filter) |
| **sequential_throughput** | ~6.3s | Raw processing 500k records baseline |

**Note**: Times include "cold start" overhead (~0.3s first run). Subsequent runs are much faster (~0.01s).

These are rough guidelines - actual performance depends on hardware and system load.

---

## External Tool Comparisons

The `compare_tools.sh` script benchmarks Kelora against common command-line tools:

### Comparison Matrix

| Category | Tools Compared | Purpose |
|----------|---------------|---------|
| **Text Filtering** | grep, ripgrep, kelora | Simple pattern matching baseline |
| **Field Extraction** | awk, kelora | Column splitting and extraction |
| **JSON Processing** | jq, kelora | JSON filtering and transformation |
| **Complex Pipelines** | bash+jq+sort, kelora | Multi-stage processing |
| **Parallel Processing** | jq, kelora (sequential vs parallel) | Multi-core scaling |
| **CSV Analytics** | miller, qsv, kelora | Structured data operations |

### Running Comparisons

```bash
# Install comparison tools (macOS)
brew install grep ripgrep jq miller qsv

# Install comparison tools (Ubuntu/Debian)
apt install grep ripgrep jq miller qsv

# Generate test datasets
just bench-datasets

# Run all comparisons
just bench-compare
```

Results are saved to `benchmarks/comparison_results/` as individual markdown files.

### Comparison Datasets

The comparison benchmarks use additional test datasets:

- **bench_100k.csv** - CSV format for miller/qsv comparison
- **bench_100k.log** - Syslog-style text for grep/awk comparison
- **bench_100k.jsonl** - JSON lines (already exists)
- **bench_500k.jsonl** - Larger dataset for parallel testing

These are generated automatically from the JSON test data.

### Documentation

Comparison results and analysis are documented in:
- **[docs/concepts/performance-comparisons.md](../docs/concepts/performance-comparisons.md)** - Comprehensive guide with decision matrix
- Results include honest assessment of when to use each tool