# Fan Out Nested Structures

Convert nested arrays and objects into individual events using `emit_each()` for processing hierarchical data.

## Problem

You have JSON logs with nested arrays (users, items, transactions) and need to process each element as a separate event for filtering, aggregation, or reporting.

## Solutions

### Basic Array Fan-Out

Convert array elements to individual events:

```bash
# Fan out users array
kelora -f json data.jsonl --exec 'emit_each(e.users)'

# Example with actual data
kelora -f json examples/json_arrays.jsonl --exec 'emit_each(e.users)' --take 5
```

The original event is suppressed; each array element becomes a new event.

### Fan-Out with Base Fields

Preserve context from parent event:

```bash
# Add batch_id to each user event
kelora -f json data.jsonl \
  --exec 'let base = #{batch_id: e.batch_id, timestamp: e.timestamp};
          emit_each(e.users, base)'

# Result: Each user event includes batch_id and timestamp fields
```

### Multi-Level Fan-Out

Fan out nested structures in stages:

```bash
# Orders → Items (two-level fan-out)
kelora -f json examples/fan_out_batches.jsonl \
  --exec 'let ctx = #{batch_id: e.batch_id}; emit_each(e.orders, ctx)' \
  --exec 'let order_ctx = #{batch_id: e.batch_id, order_id: e.order_id}; emit_each(e.items, order_ctx)'

# Now each item is a separate event with batch_id and order_id
```

### Filter After Fan-Out

Process specific elements only:

```bash
# Fan out users, then filter by score
kelora -f json data.jsonl \
  --exec 'emit_each(e.users)' \
  --filter 'e.score > 90'

# Fan out and filter in one pipeline
kelora -f json data.jsonl \
  --exec 'emit_each(e.users)' \
  --filter 'e.score > 90' \
  --keys id,name,score
```

### Count Emitted Events

Track how many events were created:

```bash
# emit_each returns count of emitted events
kelora -f json data.jsonl \
  --exec 'e.user_count = emit_each(e.users)' \
  --exec 'track_sum("total_users", e.user_count)' \
  --metrics
```

### Conditional Fan-Out

Fan out only when conditions are met:

```bash
# Only fan out batches with more than 2 items
kelora -f json data.jsonl \
  --filter 'e.users.len() > 2' \
  --exec 'emit_each(e.users)'

# Fan out high-priority items only
kelora -f json data.jsonl \
  --exec 'let high_priority = e.items.filter(|item| item.priority == "high");
          emit_each(high_priority)'
```

## Real-World Examples

### Process E-Commerce Orders

```bash
# Batch → Orders → Items (3-level fan-out)
kelora -f json orders.jsonl \
  --exec 'let batch = #{batch_id: e.batch_id, created: e.created};
          emit_each(e.orders, batch)' \
  --exec 'let order = #{batch_id: e.batch_id, order_id: e.order_id};
          emit_each(e.items, order)' \
  --exec 'e.total = e.qty * e.price' \
  --filter 'e.total > 100' \
  --keys batch_id,order_id,sku,qty,price,total
```

### Analyze User Activity

```bash
# Fan out user events and track activity types
kelora -f json activity.jsonl \
  --exec 'emit_each(e.events)' \
  --exec 'track_count(e.event_type)' \
  --metrics
```

### Extract Email Domains

```bash
# Fan out email list and extract domains
kelora -f json data.jsonl \
  --exec 'emit_each(e.emails)' \
  --exec 'e.email = e.line' \
  --exec 'e.domain = e.email.extract_domain()' \
  --exec 'track_unique("domains", e.domain)' \
  --metrics
```

### Process Log Batches

```bash
# Fan out log arrays with severity filtering
kelora -f json logs.jsonl \
  --exec 'let ctx = #{source: e.source, timestamp: e.timestamp};
          emit_each(e.logs, ctx)' \
  --filter 'e.level == "error" || e.level == "warn"' \
  --keys timestamp,source,level,msg
```

### Transaction Analysis

```bash
# Fan out purchases and calculate totals
kelora -f json transactions.jsonl \
  --exec 'let tx = #{transaction_id: e.id, user: e.user};
          emit_each(e.purchases, tx)' \
  --exec 'e.line_total = e.price * e.qty' \
  --exec 'track_sum("revenue", e.line_total)' \
  --exec 'track_count(e.item)' \
  --metrics
```

### Filter Active Items from Nested Batches

```bash
# Multi-level with filtering at each stage
kelora -f json examples/fan_out_batches.jsonl \
  --exec 'emit_each(e.batches)' \
  --exec 'let batch_ctx = #{batch_name: e.name}; emit_each(e.items, batch_ctx)' \
  --filter 'e.status == "active"' \
  --filter 'e.priority == "high"' \
  --keys batch_name,id,status,priority
```

### Aggregate Nested Statistics

```bash
# Fan out and calculate per-item statistics
kelora -f json data.jsonl \
  --exec 'emit_each(e.items)' \
  --exec 'track_sum("total_quantity", e.qty)' \
  --exec 'track_sum("total_revenue", e.price * e.qty)' \
  --exec 'track_unique("skus", e.sku)' \
  --metrics
```

### Export Flattened Data

```bash
# Fan out nested data and export as CSV
kelora -f json nested.jsonl \
  --exec 'let parent = #{parent_id: e.id, created: e.timestamp};
          emit_each(e.children, parent)' \
  --keys parent_id,created,child_id,name,value \
  -F csv > flattened.csv
```

## Fan-Out Behavior

### Original Event Handling

```bash
# Original event is suppressed after emit_each
kelora -f json data.jsonl --exec 'emit_each(e.users)'
# Output: Only user events, not the original batch event

# To keep original + fanned out events, emit before fan-out
# (Not currently supported - fan-out suppresses original)
```

### Empty Arrays

```bash
# Empty arrays emit 0 events
kelora -f json data.jsonl \
  --exec 'e.count = emit_each(e.items)' \
  --exec 'track_count(if e.count == 0 { "empty" } else { "has_items" })'
```

### Error Handling

**Resilient mode (default):**
- Invalid arrays are skipped
- Original event is suppressed
- Processing continues

**Strict mode:**
- Errors abort processing
- Use `--strict` for fail-fast behavior

```bash
# See errors if fan-out fails
kelora -f json data.jsonl --exec 'emit_each(e.users)' --verbose
```

## Tips

**Performance:**
- Fan-out increases event count significantly
- Use `--parallel` for large datasets
- Filter before fan-out when possible to reduce processing

**Memory:**
- Each fanned-out event is a separate allocation
- Large arrays can increase memory usage
- Consider batch processing with `--take` for testing

**Field Access:**
- After fan-out, access element fields directly: `e.name` not `e.users[0].name`
- Base fields are merged: `e.batch_id` available after fan-out with base map

**Metrics:**
- Track fan-out count: `e.count = emit_each(...)`
- Aggregate after fan-out: `track_sum()`, `track_count()`, etc.
- Use `--metrics` to see tracked values

**Common Patterns:**
```bash
# Preserve parent ID
let ctx = #{parent_id: e.id}; emit_each(e.children, ctx)

# Multi-level with context
emit_each(e.level1) then emit_each(e.level2, #{level1_id: e.id})

# Filter then fan-out
filter 'e.items.len() > 0' then emit_each(e.items)

# Fan-out then aggregate
emit_each(e.data) then track_count(e.category)
```

**Pipeline Order:**
1. Filter parent events (reduce fan-out volume)
2. Fan-out arrays
3. Filter child events (specific criteria)
4. Transform/enrich child events
5. Aggregate or export

## See Also

- [Process CSV Data](process-csv-data.md) - Similar patterns for tabular data
- [Monitor Application Health](monitor-application-health.md) - Nested JSON processing
- [Function Reference](../reference/functions.md) - Array functions and emit_each
- [Scripting Transforms Tutorial](../tutorials/scripting-transforms.md) - Advanced Rhai patterns
