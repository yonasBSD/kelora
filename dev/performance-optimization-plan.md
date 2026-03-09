# Performance Optimization Plan

This plan is based on the profiling pass documented in
`dev/performance-profiling-2026-03-09.md`.

## Priorities

### Priority 1: Reduce timestamp overhead

Why:

- Timestamp-related work shows up in all three traces.
- `AdaptiveTsParser::new` appears repeatedly where a reused parser would be
  preferable.
- `Event::extract_timestamp_with_config` is on the steady-state path even for
  ingest-only runs.

Likely targets:

- `src/event.rs`
- `src/timestamp.rs`
- `src/pipeline/mod.rs`
- `src/pipeline/builders.rs`

Ideas:

- Reuse `AdaptiveTsParser` instead of constructing it repeatedly.
- Avoid repeated timestamp-field discovery when the relevant field is already
  known.
- Avoid reparsing timestamps after formatting/output decisions if the parsed
  value already exists on the event.

Expected outcome:

- Low-risk win across all workloads, including pure ingest.

### Priority 2: Cut Rhai scope construction cost

Why:

- `RhaiEngine::create_scope_for_event_optimized` is one of the clearest filter
  and exec hotspots.
- Filter cost appears dominated by event-to-scope setup rather than only by the
  expression itself.

Likely targets:

- `src/engine.rs`
- `src/pipeline/stages.rs`

Ideas:

- Reuse more scope state between events.
- Minimize per-event insertion/cloning when populating `e`, `meta`, and window
  data.
- Separate truly mutable scope data from stable bindings.

Expected outcome:

- Strong payoff for both `--filter` and `--exec`.

### Priority 3: Reduce event and `Dynamic` cloning

Why:

- `Dynamic::clone` and `Event::clone` are visible in both filter and exec
  traces.
- This cost compounds with large structured events.

Likely targets:

- `src/engine.rs`
- `src/event.rs`
- `src/pipeline/stages.rs`

Ideas:

- Prefer borrowing or targeted extraction over full event cloning.
- Avoid full event reconstruction after exec when only a subset of fields
  changed.
- Audit clone-heavy paths in filter context handling and exec write-back.

Expected outcome:

- Medium-to-high impact for script-heavy workloads.

### Priority 4: Lower JSON materialization cost

Why:

- Ingest-only runs are still dominated by `serde_json` plus `IndexMap`
  insertion.
- This is the baseline cost floor for JSONL mode.

Likely targets:

- `src/parsers/json.rs`
- `src/event.rs`

Ideas:

- Reduce hash/map churn during event field insertion.
- Revisit whether every parsed JSON object must become a fully owned
  `IndexMap<String, Dynamic>` immediately.
- If not, explore narrower fast paths for common field access patterns.

Expected outcome:

- Broad improvement for JSON workloads, but likely higher implementation risk.

### Priority 5: Add a true sequential fast path

Why:

- `crossbeam_channel` signal/wake paths are visible in the sequential profile.
- Sequential mode currently pays for a reader thread and channel handoff.

Likely targets:

- `src/runner.rs`

Ideas:

- Replace the threaded reader/channel path with a direct inline read/process
  loop for the default sequential case.
- Keep the more complex control-channel path only where it is required.

Expected outcome:

- Small-to-medium gain in sequential mode, especially for ingest-heavy runs.

## Suggested Execution Order

1. Reuse timestamp parser / remove repeated timestamp parsing
2. Reduce Rhai scope construction
3. Reduce exec write-back cloning
4. Add sequential no-channel fast path
5. Re-profile
6. Only then consider deeper JSON representation changes

## Measurement Gate For Each Change

Every optimization should be validated against the same baseline workloads:

- `-j benchmarks/bench_100k.jsonl --silent`
- `-j benchmarks/bench_100k.jsonl --filter 'e.level == "ERROR"' --silent`
- `-j benchmarks/bench_100k.jsonl --exec 'track_sum("status_codes", e.status)' --silent`
- `-j benchmarks/bench_500k.jsonl --silent`
- `-j benchmarks/bench_500k.jsonl --parallel --threads 4 --silent`

And for nontrivial changes:

- Capture a fresh `xctrace` Time Profiler trace before and after.

## Changes To Avoid First

- Do not start with ordered vs unordered sink changes.
  The current measurements do not show a meaningful win there for the tested
  workload.

- Do not start with formatter micro-optimizations.
  The current traces point much more strongly to parsing, timestamp work, and
  Rhai scope/cloning overhead.

## Open Questions

- Can timestamp extraction be made lazy without breaking time filters, gap
  markers, stats, or formatters?
- Can filter-only stages use a lighter scope than exec stages?
- Can exec write-back track changed fields instead of rebuilding from scope?
- Is there a practical JSON fast path for common field access without breaking
  Rhai semantics?
