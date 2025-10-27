# Script Variables Reference

Kelora exposes several built-in variables to Rhai scripts. Their availability depends on which stage is executing (per-event filters/execs, span hooks, begin/end hooks, etc.). Use this page whenever you need to know which data is in scope.

## Stage Overview

| Stage / Feature | Variables Available | Notes |
|-----------------|---------------------|-------|
| `--filter`, `--exec`, `--exec-file` | `line`, `e`, `meta`, `conf` | Per-event context. `e` is writable; the others are read-only. |
| `--filter` / `--exec` with `--window` | `line`, `e`, `meta`, `conf`, `window` | Adds the sliding `window` array (current event at index `0`; read-only). |
| `--begin` | `conf` | Map to seed configuration before events arrive. Read-only. |
| `--end` | `metrics`, `conf` | Inspect final tracker totals in `metrics`; both bindings are read-only. |
| `--span-close` | `span`, `metrics`, `conf` | Summarises the closed span with per-span data (`span`) and cumulative totals (`metrics`). All read-only. |

> Reading a variable that does not exist in the current stage raises a Rhai error. Variables listed here are populated with meaningful data (individual fields inside maps may still be `()`). Other globals such as `line`/`e` exist behind the scenes for compatibility but start empty in stages where they are not useful.

## Common Variables

### `line`
- Type: `String`
- Snapshot of the text the parser captured for the current record. Structured formats strip trailing newlines before storing it, `-f raw` preserves every byte (including terminators), and `--multiline` reflects the chunk assembled by the multiline stage. Present in every stage and read-only.

### `e`
- Type: `Map`
- Event map during per-event stages. Mutating `e` inside `--exec` updates the emitted event; setting a field to `()` removes it; assigning `e = ()` clears the entire event. Writable.

### `meta`
- Type: `Map`
- Metadata derived from the pipeline and span system. Attempting to mutate `meta` fields has no effect; use event fields on `e` for custom annotations instead. Read-only.

| Key | Type | Description |
|-----|------|-------------|
| `line` | `String` | Same as the standalone `line` variable. |
| `line_num` | `Int` | 1-based line number when input comes from files. |
| `filename` | `String` | Source filename, if known. |
| `span_status` | `String` | `"included"`, `"late"`, `"unassigned"`, or `"filtered"` when spans are enabled. Missing otherwise. |
| `span_id` | `String` or `()` | Span identifier for the current event. |
| `span_start` / `span_end` | `DateTime` or `()` | Span bounds for the current event. |

### `conf`
- Type: `Map`
- Configuration produced by `--begin` and CLI options; the map is frozen so mutation attempts are ignored. Read-only.

## Span Hooks (`--span-close`)

### `span`
- Type: `Span` (custom Rhai type)
- Binding available only in `--span-close`. Read-only.

| Property | Type | Description |
|----------|------|-------------|
| `span.id` | `String` | Unique span identifier (`#0`, `#1`, ... for count spans; `2024-01-15T10:00:00Z/5m` for time spans). |
| `span.start` / `span.end` | `DateTime` or `()` | Span boundary timestamps (time spans only). |
| `span.size` | `Int` | Number of events that survived filters and entered the span. |
| `span.events` | `Array<Map>` | Copy of each included event, with helper fields (`line`, `line_num`, `filename`, `span_status`, `span_id`, `span_start`, `span_end`). Read-only. |
| `span.metrics` | `Map` | Per-span deltas computed from `track_*()` calls since the span opened; zero-delta keys are omitted. Read-only.

> `--span-close` runs without a "current event", so `e` and `meta` are empty. Inspect `span.events` when you need per-event details from inside the close hook.

### `metrics` vs `span.metrics`
- `span.metrics`: Only the delta accumulated while the span was open. After the hook runs, Kelora resets the baseline for the next span; mutations are discarded. Read-only.
- `metrics`: The global tracker map used across the entire run (the same map exposed to `--end`). Values persist between spans and reflect cumulative totals; update via `track_*()` calls, not direct assignment. Read-only.

## Windowed Scripts

- `window`: Present when using `--window` features. It is an array of event maps representing the sliding window, with `window[0]` being the current event followed by prior events retained by the window manager. Read-only.

## Begin / End Hooks

- `--begin`: Runs before any event is parsed. `e` and `meta` start empty; use this stage for initialization and configuration population (`conf`).
- `--end`: Receives the final `metrics` map so you can report overall totals after processing completes.
