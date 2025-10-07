# Scripting Transforms

Master Kelora's Rhai scripting stages to reshape events, enrich payloads, and
fan out nested structures. This guide focuses on practical patterns you can drop
into your own pipelines.

## What You'll Learn

- Normalize inconsistent fields and add derived values
- Build multi-stage transformations with `--exec`
- Validate and discard malformed events safely
- Fan out nested arrays and preserve context
- Share reusable helpers with `--include` / `--exec-file`
- Understand resilient vs strict error handling

## Prerequisites

- Completed the [Quickstart](../quickstart.md)
- Reviewed [Metrics and Tracking](metrics-and-tracking.md) for pipeline context

## Step 1 – Normalize Fields Safely

Start with `examples/errors_exec_transform.jsonl`, which includes messy values.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/errors_exec_transform.jsonl \
  --exec 'e.status_code = to_int_or(e.status, -1)' \
  --exec 'e.bytes_int = to_int_or(e.bytes, 0)' \
  -F json --take 3
```

`to_int_or(value, fallback)` converts strings to integers and substitutes the
fallback when conversion fails. Stacking `--exec` scripts lets you layer
transformations without writing monolithic expressions.

## Step 2 – Derive New Fields and Filter by Them

Use the normalized values to add severity classification, then keep only error
events.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/errors_exec_transform.jsonl \
  --exec 'e.status_code = to_int_or(e.status, -1)' \
  --exec 'e.severity = if e.status_code >= 500 { "critical" } else if e.status_code >= 400 { "error" } else { "ok" }' \
  --filter 'e.severity != "ok"' \
  --keys timestamp,status_code,severity \
  -F json
```

Filters run between exec scripts. Any fields you create earlier are immediately
available to later filters or transformations.

## Step 3 – Guard Against Bad Data

Kelora defaults to resilient mode: exec errors roll back the event and
processing continues. Still, it is better to guard the data and drop malformed
records explicitly.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/errors_exec_transform.jsonl \
  --exec 'if !("tags" in e) || type_of(e.tags) != "array" { e = () } else { e.tag_count = e.tags.len(); }' \
  -F json
```

`e = ()` removes the event from the pipeline. The type check avoids runtime
errors when the `tags` field contains a string instead of an array.

!!! warning
    Add `--strict` if you want Kelora to abort on the first error instead of
    skipping bad events. Strict mode is ideal for CI pipelines where silent
    fallback would hide problems.

## Step 4 – Fan Out Nested Data

`emit_each()` converts arrays into individual events while keeping context. The
orders fixture demonstrates nested arrays that you can flatten in two stages.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/fan_out_batches.jsonl \
  --exec 'if e.has_path("orders") { emit_each(e.orders, #{batch_id: e.batch_id, created: e.created}) }' \
  --exec 'if e.has_path("items") { emit_each(e.items, #{batch_id: e.batch_id, order_id: e.order_id}) }' \
  --filter 'e.has_path("sku")' \
  --keys batch_id,order_id,sku,qty,price \
  -F json --take 4
```

The first exec fans out orders while copying batch metadata. The second exec
fans out individual items, enriching each emitted event with both batch and
order identifiers.

## Step 5 – Window-Aware Enrichment

Enable the sliding window to compare the current event to recent history.

```bash exec="on" source="above" result="ansi"
kelora -f json examples/window_metrics.jsonl \
  --filter 'e.metric == "cpu"' \
  --window 3 \
  --exec $'let values = window_numbers(window, "value");
if values.len() >= 2 {
    let diff = values[0] - values[1];
    e.delta_vs_prev = round(diff * 100.0) / 100.0;
}' \
  --keys timestamp,value,delta_vs_prev \
  -F json --take 5
```

`window` holds the current event plus the previous `N` events (here `N = 3`).
Using `window_numbers(window, FIELD)` avoids manual parsing and gracefully
skips missing values.

## Step 6 – Reuse Logic with `--include` and `--exec-file`

Keep complex logic in separate files. The snippet below defines a helper and
reuses it across multiple commands.

```bash exec="on" source="above" result="ansi"
cat <<'RHAI' > classifiers.rhai
fn classify_status(status) {
    let code = to_int_or(status, -1);
    if code >= 500 {
        "critical"
    } else if code >= 400 {
        "error"
    } else if code >= 200 {
        "ok"
    } else {
        "other"
    }
}
RHAI

kelora -f json examples/errors_exec_transform.jsonl \
  -I classifiers.rhai \
  --exec 'e.severity = classify_status(e.status)' \
  --keys timestamp,status,severity \
  -F json --take 3
rm classifiers.rhai
```

You can also move long exec blocks to a dedicated Rhai file:

```bash
kelora -f json app.jsonl --exec-file transforms.rhai
```

Put shared helpers under `scripts/` or `dev/` and version them alongside your
pipeline definitions.

## Step 7 – Understand Error Handling Semantics

- **Resilient mode (default)**: Kelora rolls back the event when an exec throws
  and prints a warning. The pipeline continues with the next event. Use this for
  exploratory work and long-running tail sessions.
- **Strict mode (`--strict`)**: The first exec error stops the process with
  exit code `1`. Ideal for CI, production batch jobs, or when you have unit
  tests guarding your scripts.
- **Filtering events**: Set `e = ()` to drop a record after you detect invalid
  data. This is preferable to letting bad events fail later stages.

## Troubleshooting Checklist

- **Function not found**: Ensure supporting files are loaded via `-I` or
  `--exec-file`, and double-check spelling—Rhai treats names as case-sensitive.
- **Unexpected fields missing**: Remember that filters remove events between
  exec stages. If a later exec cannot find a field, verify the event survived the
  earlier filters.
- **Decimal precision**: Functions like `round(value)` operate on whole numbers.
  Multiply before rounding (`round(value * 100.0) / 100.0`) to keep two decimal
  places.
- **Fan-out explosion**: Each `emit_each` can multiply events. Add guard
  conditions (`if e.has_path("orders")`) to avoid emitting empty rows.

## Next Steps

- Deepen your understanding of the [Scripting Stages concept](../concepts/scripting-stages.md).
- Learn how Kelora [handles multiline input](../concepts/multiline-strategies.md)
  for stack traces and payloads.
- Explore the [Function Reference](../reference/functions.md) for every helper
  available in the scripting environment.
