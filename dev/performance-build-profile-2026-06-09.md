# Build-Profile Performance Optimization (2026-06-09)

Follow-up to the March 2026 profiling pass
(`dev/performance-profiling-2026-03-09.md`), which showed the hot path is
allocation- and codegen-bound (JSON → `IndexMap` materialization, `Dynamic`/
`Event` clones, per-event scope construction).

This change targets that floor with three build-level levers — no source-logic
changes to the pipeline.

## Changes

1. **mimalloc as the global allocator** (`src/main.rs`, `Cargo.toml`).
   The per-event hot path is dominated by allocator churn (`IndexMap::insert_full`,
   `json_to_dynamic_owned`, `Dynamic::clone`, `Event::clone`). mimalloc handles
   that churn far better than the system allocator on macOS.

2. **`lto = "thin"`** (`[profile.release]`). Cross-crate optimization.
   Fat LTO was measured as runtime-equivalent here (within noise) at roughly
   double the compile time, so thin was chosen. Binary defaults to `codegen-units`
   (16) — fastest compile with no measurable runtime cost on these workloads.

3. **`panic = "abort"`** (`[profile.release]`). Removes unwind landing pads from
   the `Drop`-heavy per-event loop. Measured as a real, reproducible ~7–8% win
   (not noise — confirmed in both measurement orderings), plus ~12% smaller binary.
   Safe because no runtime `catch_unwind` exists (the only occurrences are in
   `math.rs` `#[cfg(test)]` code). Thread panics already terminated the run via
   the join handlers in `runner.rs` / `parallel/processor.rs`; see "Behavior
   change" below.

## Measurements

Host: macOS (same as the March pass). Best-of-5 wall-clock per workload,
warm page cache, against `benchmarks/bench_100k.jsonl` / `bench_500k.jsonl`.

Cumulative (baseline = system allocator, no LTO, default profile):

| Workload | Baseline | Final | Speedup |
|----------|----------|-------|---------|
| `ingest_100k` | 0.608s | 0.356s | 1.71x |
| `filter_100k` | 1.093s | 0.575s | 1.90x |
| `exec_100k`   | 1.430s | 0.730s | 1.96x |
| `seq_500k`    | 2.803s | 1.572s | 1.78x |
| `par_500k`    | 0.877s | 0.508s | 1.73x |

Isolated effect of `panic = "abort"` (thin + mimalloc, abort vs unwind,
averaged over both orderings to rule out thermal/ordering bias):

| Workload | unwind | abort | abort win |
|----------|--------|-------|-----------|
| `ingest_100k` | 0.382s | 0.356s | 6.8% |
| `filter_100k` | 0.625s | 0.575s | 8.0% |
| `exec_100k`   | 0.790s | 0.730s | 7.6% |
| `seq_500k`    | 1.717s | 1.572s | 8.4% |
| `par_500k`    | 0.548s | 0.508s | 7.3% |
| binary size   | 16.2 MB | 14.3 MB | 12% smaller |

Output verified byte-identical to the baseline binary on filter / exec / JSON /
logfmt sample runs. Full `just check` passed (fmt, lint, audit, deny, tests).

## Behavior change

`panic = "abort"` changes the exit code on an internal thread panic (reader /
worker / sink) from `1`/`101` to `134` (SIGABRT). These are bug paths that
already terminated the process before this change — only the reported code
differs. Documented in `docs/reference/exit-codes.md` and `AGENTS.md`.

## Trade-offs / notes

- Compile time: thin + default `codegen-units` builds the release binary in
  ~2m (fat + `codegen-units = 1` was ~5m for no measurable runtime gain).
- If the smallest possible release artifact matters more than build speed,
  fat LTO + `codegen-units = 1` shaved the binary further (11.7 MB) — could be a
  dist-only profile override, but it's not worth it for the default build.

## Still open (from the March plan)

The deeper source-level items remain untouched and now sit on top of this ~1.8x
floor:

- Priority 1: timestamp parser reuse / lazy extraction
- Priority 3: reduce `Dynamic`/`Event` cloning in filter/exec write-back
- Priority 4: JSON → `IndexMap` materialization cost

Recommended next tooling before attacking those: a `dhat-heap` feature for
allocation profiling and a `samply` capture to replace the ad-hoc xctrace flow.
