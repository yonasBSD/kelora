# Verbose Redesign Spec (-v / -vv / -vvv)

## Goals
- Make `-v`-levels show useful pipeline progress, not just errors.
- Keep output bounded/safe for terminals and `--parallel`.
- Keep fast path cheap when verbose is off.

## Current State (for reference)
- `-v`..`-vvv` only affect error verbosity plus Rhai debug tracer (can be very chatty).
- No per-stage visibility or progress indicators during long runs.

## Proposed Semantics
- `-v` (progress + errors):
  - Startup plan (once): input format (resolved), multiline strategy, timestamp hints, formatter, keys/core, take/head/context, span config, parallel/threads, stage count summary.
  - Immediate error lines as today.
  - Periodic progress block every `min(5s, 5000 evts)` with counts: lines read, events emitted, errors by type (top 3), spans open/closed, throughput estimate.
- `-vv` (sampled flow):
  - Includes `-v`.
  - Stage map printed once with stable IDs: `#1 filter <expr trimmed>`, `#2 exec <script trimmed>`, etc.
  - Sampled event trace: first `K` events and every `M`th thereafter (defaults below). For each sampled event: `event <id> file:line stage_outcomes=` list of `filter#n=pass/fail`, `exec#m=ok`, `drop by filter#k` etc.; formatter preview (inspect one-liner); context marker (before/after/match) if active; span marker if active.
  - Drop reasons: when an event is skipped, log the responsible stage ID (sampled only).
  - Periodic block includes small per-stage aggregates (top 3 dropping/error stages).
- `-vvv` (deep debug):
  - Includes `-vv`.
  - Existing Rhai ExecutionTracer output (scope/AST/step) but gated by sample budget.
  - Parse errors show full offending line and line stats (current behavior), span lifecycle traces sampled, per-stage timing summary at end.

## Defaults / Caps (tunable constants)
- Sampling: `K=5`, `M=100` for `-vv`; `K=3`, `M=50` for `-vvv`.
- Global trace cap per process: `MAX_TRACE_LINES=200` (drop + note when exceeded).
- Truncation: script/expr previews 80 chars; key diffs 8 keys; formatter preview 120 chars. No full payloads before `-vvv`.
- Progress cadence: every 5s or 5000 processed events, whichever is later.
- Stop emitting traces after `--take`/`--head` exhausted.

## Parallel Mode Handling
- Stage map and startup plan emitted by coordinator only.
- Per-event traces: workers decide sampling locally (deterministic on per-worker event counter); send trace lines through the existing ordered stderr capture with a bounded buffer. If cap hit, workers drop additional traces silently; coordinator can print a single “trace cap reached” notice.
- Per-stage aggregates: use thread-local tracking with keys like `__kelora_stage_drop_<id>` / `__kelora_stage_error_<id>` and `__op_* = count` so reducer sums lock-free.
- Ordering: traces may interleave between workers; document “best-effort order”. Errors stay ordered as today.

## Data to Collect (hot-path constraints)
- Per-stage status for the current event: enum pass/fail/exec_ok/drop_reason. Avoid cloning whole events; capture only filename:line, event id, and formatted preview (inspect) on sampled events.
- Exec deltas (optional): when sampled, diff key set before/after exec; list added/removed keys only.
- Context/span markers from existing metadata.
- Counters: lines read, events emitted, errors by type, spans open/closed, per-stage drop/error counts (via tracking).

## Output Rules / Suppression
- Honor `--quiet/--no-events/--silent` → suppress traces/progress; still allow fatal errors.
- Honor `--no-diagnostics` → suppress periodic progress blocks.
- Honor `--no-emoji`/`NO_EMOJI` in prefixes for all verbose lines.

## Failure / Backpressure Strategy
- If trace buffer is full or cap exceeded, drop further traces; emit one notice. Never block workers.
- Progress blocks are best-effort; if reducer snapshot unavailable, skip the tick.

## Testing Notes
- Cover sequential vs parallel runs to ensure caps prevent runaway output.
- Validate drop reasons and stage IDs align with startup map.
- Ensure silent/metrics-only/stats-only produce no traces.
- Check long filenames/basename conflicts still format correctly in traces.

## Open Questions (to decide when implementing)
- Whether to expose user knobs (`--verbose-sample`, `--verbose-cap`) or keep constants compiled.
- Whether to include per-stage timing in `-vvv` progress summary by default, and cost thereof.
