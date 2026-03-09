# Performance Profiling Notes (2026-03-09)

This note captures a first profiling pass on macOS against the current release
binary.

## Environment

- Date: 2026-03-09
- Host: macOS
- Binary: `./target/release/kelora`
- Wall-clock tool: `hyperfine 1.19.0`
- CPU profiler: `xctrace` Time Profiler

## Workloads

Wall-clock benchmarks used:

```bash
hyperfine --warmup 1 --runs 5 \
  --command-name ingest_100k "./target/release/kelora -j benchmarks/bench_100k.jsonl --silent > /dev/null" \
  --command-name filter_100k "./target/release/kelora -j benchmarks/bench_100k.jsonl --filter 'e.level == \"ERROR\"' --silent > /dev/null" \
  --command-name exec_100k "./target/release/kelora -j benchmarks/bench_100k.jsonl --exec 'track_sum(\"status_codes\", e.status)' --silent > /dev/null" \
  --command-name seq_500k "./target/release/kelora -j benchmarks/bench_500k.jsonl --silent > /dev/null" \
  --command-name par_ordered_500k "./target/release/kelora -j benchmarks/bench_500k.jsonl --parallel --threads 4 --silent > /dev/null" \
  --command-name par_unordered_500k "./target/release/kelora -j benchmarks/bench_500k.jsonl --parallel --threads 4 --unordered --silent > /dev/null"
```

Profiler workloads used:

```bash
./target/release/kelora -j benchmarks/bench_500k.jsonl --silent
./target/release/kelora -j benchmarks/bench_500k.jsonl --filter 'e.level == "ERROR"' --silent
./target/release/kelora -j benchmarks/bench_500k.jsonl --exec 'track_sum("status_codes", e.status)' --silent
```

## Hyperfine Results

| Workload | Mean | Throughput |
|----------|------|------------|
| `ingest_100k` | `0.508s` | `197k lines/s` |
| `filter_100k` | `0.888s` | `113k lines/s` |
| `exec_100k` | `1.195s` | `84k lines/s` |
| `seq_500k` | `2.510s` | `199k lines/s` |
| `par_ordered_500k` | `0.792s` | `631k lines/s` |
| `par_unordered_500k` | `0.800s` | `625k lines/s` |

Derived deltas:

- Cheap filter overhead vs ingest-only: about `+75%`
- Simple exec overhead vs ingest-only: about `+135%`
- Parallel ordered speedup vs sequential: about `3.17x`
- Parallel unordered vs ordered: effectively flat in this workload

## Time Profiler Summary

### 1. Sequential ingest-only (`-j ... --silent`)

Steady-state hot frames were concentrated in:

- JSON parse/deserialization
  - `JsonlParser::parse`
  - `serde_json` map/string parsing
- `IndexMap` operations
  - `insert_full`
  - `get_index_of`
- Event conversion
  - `json_to_dynamic_owned`
- Timestamp handling
  - `Event::extract_timestamp_with_config`
  - `identify_timestamp_field`
  - `AdaptiveTsParser::new`
- Sequential coordination overhead
  - `crossbeam_channel` wake/signal paths

Interpretation:

- The baseline hot path is parse-heavy, map-heavy, and allocation-heavy.
- Timestamp extraction is visible often enough to be a meaningful cost.
- Sequential mode still pays measurable thread/channel overhead.

### 2. Sequential filter (`--filter 'e.level == "ERROR"'`)

New hot frames relative to ingest-only:

- `RhaiEngine::create_scope_for_event_optimized`
- `RhaiEngine::execute_compiled_filter_with_window`
- `Scope::set_value`
- `Dynamic::clone`
- `Event::clone`

Interpretation:

- Filter cost is not just expression evaluation.
- A large part of the overhead is per-event scope construction and ownership
  churn while preparing data for Rhai.

### 3. Sequential exec (`--exec 'track_sum("status_codes", e.status)'`)

New hot frames relative to ingest-only:

- `RhaiEngine::create_scope_for_event_optimized`
- `RhaiEngine::update_event_from_scope`
- `RhaiEngine::execute_compiled_exec_with_window`
- `Dynamic::clone`
- `Event::clone`

Interpretation:

- Exec pays the same scope-creation cost as filter.
- It also pays a second write-back cost when updating the event from the Rhai
  scope after execution.

## Main Findings

Ordered by confidence and likely impact:

1. Timestamp parsing/setup is hotter than expected and should be reduced.
2. Rhai scope creation is a major part of filter/exec cost.
3. Event and `Dynamic` cloning are material costs in script-heavy paths.
4. JSON to `IndexMap<String, Dynamic>` materialization remains a dominant base
   cost.
5. Sequential mode has avoidable channel/thread overhead.

## Non-findings

- For the tested JSONL batch workload, ordered parallel output was not
  meaningfully slower than unordered parallel output.
- The first optimization target should not be ordered sink logic.

## Artifacts

Profiler exports used during analysis were written outside the repo:

- `/tmp/kelora-seq.trace`
- `/tmp/kelora-seq.xml`
- `/tmp/kelora-filter-2.trace`
- `/tmp/kelora-filter.xml`
- `/tmp/kelora-exec-2.trace`
- `/tmp/kelora-exec.xml`
