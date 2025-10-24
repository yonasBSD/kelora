# Introduction to Rhai Scripting

Learn how to write simple Rhai scripts to filter and transform log events. This tutorial bridges the gap between basic CLI usage and advanced scripting.

## What You'll Learn

- Understand the event object (`e`) and field access
- Write simple filter expressions with `--filter`
- Transform events with basic `--exec` scripts
- Use string operations and conditionals
- Convert between types safely
- Understand why pipeline order matters
- Debug scripts with `-F inspect` and `--verbose`
- Avoid common mistakes

## Prerequisites

- [Getting Started: Input, Display & Filtering](basics.md) - Basic CLI usage
- **Time:** ~20 minutes

## Sample Data

This tutorial uses `examples/simple_json.jsonl` - application logs with various services and events.

Preview the data:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl --take 5
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl --take 5
    ```

---

## Step 1: Understanding the Event Object

Every event in Kelora is represented as a map (dictionary) accessible via the variable `e` in your Rhai scripts.

**Accessing fields:**

```rhai
e.level          # Direct field access
e["level"]       # Bracket notation (useful for dynamic fields)
e.status         # Access any field in the event
```

Let's see the structure with `-F inspect`:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl -F inspect --take 1
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl -F inspect --take 1
    ```

**Key insight:** Field names become properties you can access in scripts.

---

## Step 2: Simple Filter Expressions

Use `--filter` to keep only events where the expression returns `true`.

### Filter by String Equality

Keep only ERROR level events:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl --filter 'e.level == "ERROR"'
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl --filter 'e.level == "ERROR"'
    ```

**What happened:** Only events where `e.level` equals `"ERROR"` are kept.

### Filter by Numeric Comparison

Keep only slow queries (duration > 1000ms):

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl --filter 'e.duration_ms > 1000'
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl --filter 'e.duration_ms > 1000'
    ```

**Important:** This only keeps events that **have** a `duration_ms` field. Events without it are skipped.

### Combine Conditions with Logical Operators

Find ERROR or WARN events from the database service:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --filter '(e.level == "ERROR" || e.level == "WARN") && e.service == "database"'
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --filter '(e.level == "ERROR" || e.level == "WARN") && e.service == "database"'
    ```

**Operators:**
- `==` - Equals
- `!=` - Not equals
- `>`, `>=`, `<`, `<=` - Comparison
- `&&` - AND
- `||` - OR
- `!` - NOT

---

## Step 3: Basic Transformations with --exec

Use `--exec` to modify events or add new fields.

### Add a Computed Field

Convert milliseconds to seconds:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --exec 'e.duration_s = e.duration_ms / 1000' \
        --filter 'e.duration_s > 1.0' \
        -k timestamp,service,duration_ms,duration_s
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --exec 'e.duration_s = e.duration_ms / 1000' \
        --filter 'e.duration_s > 1.0' \
        -k timestamp,service,duration_ms,duration_s
    ```

**Key insight:** `--exec` runs **before** `--filter`, so the new field is available for filtering.

### Modify Existing Fields

Normalize level to uppercase:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --exec 'e.level = e.level.to_upper()' \
        --take 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --exec 'e.level = e.level.to_upper()' \
        --take 3
    ```

---

## Step 4: String Operations

Rhai provides powerful string methods for text processing.

### Check if String Contains Text

Find events with "timeout" in the message:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --filter 'e.message.contains("timeout")'
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --filter 'e.message.contains("timeout")'
    ```

### Extract Parts of Strings

Extract just the error type from messages:

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --filter 'e.level == "ERROR"' \
        --exec 'e.error_type = e.message.split(" ")[0]' \
        -k timestamp,service,error_type,message
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --filter 'e.level == "ERROR"' \
        --exec 'e.error_type = e.message.split(" ")[0]' \
        -k timestamp,service,error_type,message
    ```

**Common string methods:**
- `contains(substr)` - Check if string contains text
- `starts_with(prefix)` - Check prefix
- `ends_with(suffix)` - Check suffix
- `split(sep)` - Split into array
- `to_upper()` / `to_lower()` - Change case
- `trim()` - Remove whitespace
- `len()` - String length

---

## Step 5: Conditionals and Logic

Use `if/else` to make decisions in your transforms.

### Add Severity Classification

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --exec 'e.severity = if e.level == "ERROR" || e.level == "CRITICAL" {
                    "high"
                } else if e.level == "WARN" {
                    "medium"
                } else {
                    "low"
                }' \
        -k level,severity,service,message \
        --take 5
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --exec 'e.severity = if e.level == "ERROR" || e.level == "CRITICAL" { "high" } else if e.level == "WARN" { "medium" } else { "low" }' \
        -k level,severity,service,message \
        --take 5
    ```

**Syntax:**
```rhai
if condition {
    // then branch
} else if another_condition {
    // else-if branch
} else {
    // else branch
}
```

---

## Step 6: Type Conversions

Fields may be strings when you need numbers (or vice versa). Convert types safely.

### Safe Conversion with Fallbacks

Use `to_int_or()` to handle conversion failures:

=== "Command"

    ```bash
    echo '{"id":"123","status":"200","invalid":"abc"}
    {"id":"456","status":"404","invalid":"xyz"}' | \
        kelora -j \
        --exec 'e.id_num = e.id.to_int_or(-1)' \
        --exec 'e.status_num = e.status.to_int_or(0)' \
        --exec 'e.invalid_num = e.invalid.to_int_or(999)' \
        -k id,id_num,status,status_num,invalid,invalid_num
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    echo '{"id":"123","status":"200","invalid":"abc"}
    {"id":"456","status":"404","invalid":"xyz"}' | \
        kelora -j \
        --exec 'e.id_num = e.id.to_int_or(-1)' \
        --exec 'e.status_num = e.status.to_int_or(0)' \
        --exec 'e.invalid_num = e.invalid.to_int_or(999)' \
        -k id,id_num,status,status_num,invalid,invalid_num
    ```

**Safe conversion functions:**
- `to_int_or(fallback)` - Convert to integer or use fallback
- `to_float_or(fallback)` - Convert to float or use fallback
- `to_string()` - Convert to string (always succeeds)

---

## Step 7: Pipeline Order Matters

The order of `--filter` and `--exec` flags determines execution order.

### ❌ Wrong Order: Filter Before Creating Field

This **won't work** because `duration_s` doesn't exist yet:

```bash
# WRONG - will fail
kelora -j examples/simple_json.jsonl \
    --filter 'e.duration_s > 1.0' \
    --exec 'e.duration_s = e.duration_ms / 1000'
```

### ✅ Correct Order: Create Field Before Filtering

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --exec 'e.duration_s = e.duration_ms / 1000' \
        --filter 'e.duration_s > 1.0' \
        -k service,duration_s,message
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --exec 'e.duration_s = e.duration_ms / 1000' \
        --filter 'e.duration_s > 1.0' \
        -k service,duration_s,message
    ```

**Rule:** Fields must exist before you filter on them. Scripts run in CLI order.

---

## Step 8: Checking if Fields Exist

Not all events have the same fields. Use `has_field()` to check before accessing.

### Safe Field Access

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --exec 'e.has_duration = e.has_field("duration_ms")' \
        --exec 'if e.has_field("duration_ms") {
                    e.slow = e.duration_ms > 1000
                } else {
                    e.slow = false
                }' \
        -k service,has_duration,slow,message \
        --take 5
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --exec 'e.has_duration = e.has_field("duration_ms")' \
        --exec 'if e.has_field("duration_ms") { e.slow = e.duration_ms > 1000 } else { e.slow = false }' \
        -k service,has_duration,slow,message \
        --take 5
    ```

**Pattern:**
```rhai
if e.has_field("field_name") {
    // Safe to access e.field_name
} else {
    // Provide default behavior
}
```

---

## Step 9: Debugging Your Scripts

When scripts don't work as expected, use these techniques.

### Use -F inspect to See Types

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --exec 'e.status = 200' \
        --exec 'e.computed = e.status * 2' \
        -F inspect --take 1
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --exec 'e.status = 200' \
        --exec 'e.computed = e.status * 2' \
        -F inspect --take 1
    ```

### Use --verbose to See Errors

When scripts fail in resilient mode, use `--verbose` to see what went wrong:

=== "Command"

    ```bash
    echo '{"value":"not_a_number"}' | \
        kelora -j \
        --exec 'e.num = e.value.to_int()' \
        --verbose
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    echo '{"value":"not_a_number"}' | \
        kelora -j \
        --exec 'e.num = e.value.to_int()' \
        --verbose
    ```

**Debug workflow:**
1. Use `-F inspect` to check field types
2. Use `--verbose` to see error messages
3. Use `--strict` to fail fast on first error
4. Add temporary fields to see intermediate values

---

## Step 10: Multi-Stage Pipelines

Chain multiple `--exec` and `--filter` stages for complex logic.

### Progressive Refinement

=== "Command"

    ```bash
    kelora -j examples/simple_json.jsonl \
        --exec 'e.is_error = e.level == "ERROR" || e.level == "CRITICAL"' \
        --exec 'e.is_slow = e.has_field("duration_ms") && e.duration_ms > 1000' \
        --exec 'e.needs_attention = e.is_error || e.is_slow' \
        --filter 'e.needs_attention' \
        -k service,level,is_error,is_slow,message
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/simple_json.jsonl \
        --exec 'e.is_error = e.level == "ERROR" || e.level == "CRITICAL"' \
        --exec 'e.is_slow = e.has_field("duration_ms") && e.duration_ms > 1000' \
        --exec 'e.needs_attention = e.is_error || e.is_slow' \
        --filter 'e.needs_attention' \
        -k service,level,is_error,is_slow,message
    ```

**Pattern:** Build up computed fields step-by-step, then filter on the final result.

---

## Common Mistakes and Solutions

### ❌ Mistake 1: Accessing Missing Fields

**Problem:**
```bash
kelora -j app.log --filter 'e.status >= 400'  # Fails if status doesn't exist
```

**Solution:**
```bash
kelora -j app.log --filter 'e.has_field("status") && e.status >= 400'
```

---

### ❌ Mistake 2: String vs Number Comparison

**Problem:**
```bash
# If status is string "200", this won't match
kelora -j app.log --filter 'e.status == 200'
```

**Solution:**
```bash
# Convert to int first
kelora -j app.log --filter 'e.status.to_int_or(0) == 200'
```

---

### ❌ Mistake 3: Wrong Pipeline Order

**Problem:**
```bash
# Field doesn't exist yet!
kelora -j app.log --filter 'e.is_slow' --exec 'e.is_slow = e.duration > 1000'
```

**Solution:**
```bash
# Create field first, then filter
kelora -j app.log --exec 'e.is_slow = e.duration > 1000' --filter 'e.is_slow'
```

---

### ❌ Mistake 4: Forgetting Quotes

**Problem:**
```bash
# Shell interprets && as command separator
kelora -j app.log --filter e.level == ERROR && e.service == api
```

**Solution:**
```bash
# Quote the entire expression
kelora -j app.log --filter 'e.level == "ERROR" && e.service == "api"'
```

---

## Quick Reference

### Accessing Fields
```rhai
e.field_name              # Direct access
e["field_name"]           # Bracket notation
e.has_field("name")       # Check existence
e.get("name", default)    # Get with fallback
```

### Filter Operators
```rhai
==  !=                    # Equality
<  <=  >  >=              # Comparison
&&  ||  !                 # Logical AND, OR, NOT
```

### String Methods
```rhai
.contains("text")         # Check substring
.starts_with("pre")       # Check prefix
.ends_with("suf")         # Check suffix
.to_upper()  .to_lower()  # Change case
.split(" ")               # Split into array
.trim()                   # Remove whitespace
.len()                    # String length
```

### Type Conversions
```rhai
.to_int_or(fallback)      # String → Int
.to_float_or(fallback)    # String → Float
.to_string()              # Any → String
```

### Conditionals
```rhai
if condition {
    // code
} else if other {
    // code
} else {
    // code
}
```

---

## Practice Exercises

Try these on your own:

### Exercise 1: Find High-Memory Warnings
Filter for WARN events where `memory_percent` > 80:

<details>
<summary>Solution</summary>

```bash
kelora -j examples/simple_json.jsonl \
    --filter 'e.level == "WARN" && e.has_field("memory_percent") && e.memory_percent > 80'
```
</details>

### Exercise 2: Classify Request Speeds
Add a `speed` field: "fast" if duration < 100ms, "normal" if < 1000ms, else "slow":

<details>
<summary>Solution</summary>

```bash
kelora -j examples/simple_json.jsonl \
    --exec 'if e.has_field("duration_ms") {
                e.speed = if e.duration_ms < 100 { "fast" }
                         else if e.duration_ms < 1000 { "normal" }
                         else { "slow" }
            } else {
                e.speed = "unknown"
            }' \
    -k service,duration_ms,speed,message
```
</details>

### Exercise 3: Extract HTTP Method
For events with a `method` field, add `is_safe_method` (true for GET/HEAD):

<details>
<summary>Solution</summary>

```bash
kelora -j examples/simple_json.jsonl \
    --exec 'if e.has_field("method") {
                e.is_safe_method = e.method == "GET" || e.method == "HEAD"
            }' \
    --filter 'e.has_field("is_safe_method")' \
    -k method,is_safe_method,path
```
</details>

---

## Summary

You've learned:

- ✅ Access event fields with `e.field_name`
- ✅ Filter events with `--filter` boolean expressions
- ✅ Transform events with `--exec` scripts
- ✅ Use string methods like `.contains()`, `.split()`, `.to_upper()`
- ✅ Convert types safely with `to_int_or()`, `to_float_or()`
- ✅ Write conditionals with `if/else`
- ✅ Check field existence with `has_field()`
- ✅ Debug with `-F inspect` and `--verbose`
- ✅ Understand pipeline order (exec before filter)
- ✅ Build multi-stage pipelines

## Next Steps

Now that you understand basic Rhai scripting, continue to:

- **[Working with Time](working-with-time.md)** - Time filtering and timezone handling
- **[Metrics and Tracking](metrics-and-tracking.md)** - Aggregate data with `track_*()` functions
- **[Scripting Transforms](scripting-transforms.md)** - Advanced patterns and techniques

**Related guides:**
- [Function Reference](../reference/functions.md) - Complete function catalog
- [Rhai Cheatsheet](../reference/rhai-cheatsheet.md) - Quick syntax reference
- [How-To: Triage Production Errors](../how-to/find-errors-in-logs.md) - Practical examples
