# Drain Output Format Spec

## Goal
Provide a simple, zero-scripting way to discover common log message templates
across all events using a dedicated summary flag.

## CLI Surface
- `--drain` (summary output, post-run)
- Requires `--keys` with exactly one field (same rule as `keymap`)
  - Example: `kelora -j app.log --drain -k message`
  - Example: `kelora -f line app.log --drain -k line`

## Output Behavior
- Templates-only output; no per-event output.
- Printed at end of run (summary output, like `--metrics`).
- Sorted by count descending.
- Full list by default (no output limit flag).
- No template IDs in the table output.

## Output Shape (Table)
```
templates (N items):
  #1  <template>                              <count>
  #2  <template>                              <count>
  ...
```

### Formatting Details
- Prefix header: `templates (N items):`
- Each row:
  - `#` + 1-based rank (descending by count)
  - Template string
  - Count (right-aligned if feasible)
- Order is deterministic: primarily by count desc, then by template string
  (for stable ordering on ties).

## Non-Goals (v1)
- No IDs in table output.
- No `--drain-limit` or truncation; always full list.
- No extra columns (examples, percentage) in table output.
- No JSON mode (possible future extension).

## Notes
- Drain is stateful and order-dependent. Templates can generalize as more
  events are ingested, but the final output reflects the end-of-run state.

## Optional Rhai API (sequential-only)
- `drain_template(text [, options]) -> map`
  - Returns `{template, count, is_new}`.
  - Raises an error under `--parallel`.
- `drain_templates() -> array`
  - Returns array of `{template, count}` for end-of-run reporting.
  - Raises an error under `--parallel`.
