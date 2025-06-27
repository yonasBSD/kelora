# Kelora Performance Benchmark Suite

A lightweight benchmarking system to prevent performance regressions and track improvements over time.

## Quick Start

```bash
# Run quick benchmarks (10k dataset)
make bench-quick

# Run full benchmark suite (10k + 50k datasets)
make bench

# Update performance baseline
make bench-baseline
```

## Benchmark Tests

| Test | Dataset | Description |
|------|---------|-------------|
| **small_filter** | 100k lines | Filter ERROR level logs - tests basic filtering performance |
| **medium_processing** | 500k lines | Filter + tracking - tests evaluation and state management |
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
make bench-baseline
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
5. **Update baseline**: `make bench-baseline` (if improvements are intentional)

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