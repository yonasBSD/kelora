# Error Handling

Understanding Kelora's error handling modes and how to diagnose issues.

## Processing Modes

Kelora offers two error handling modes:

| Mode | Behavior | Use Case |
|------|----------|----------|
| **Resilient (default)** | Skip errors, continue processing | Production log analysis, exploratory work |
| **Strict (`--strict`)** | Fail-fast on errors | Data validation, CI/CD pipelines |

## Resilient Mode

### Overview

In resilient mode (default), Kelora continues processing even when errors occur:

- **Parse errors**: Skip unparseable lines, continue with next line
- **Filter errors**: Treat as `false`, skip event
- **Transform errors**: Return original event unchanged (atomic rollback)
- **Summary**: Show error count at end

### When to Use

- Analyzing messy production logs
- Exploratory data analysis
- Real-time log streaming
- Mixed format log files

### Example Behavior

```bash
> kelora -j app.log --exec 'e.result = e.value.to_int() * 2'
```

**Input:**
```json
{"value": "123"}
{"value": "invalid"}
{"value": "456"}
```

**Output:**
```
result=246
(skipped - error converting "invalid")
result=912
```

**Summary:**
```
🔹 Processed 3 lines, 2 events output, 1 error
```

### Error Recording

Errors are recorded but don't stop processing:

```bash
> kelora -j app.log --filter 'e.timestamp.to_unix() > 1000000'
```

If `e.timestamp` is missing or invalid:

- Filter evaluates to `false`
- Event is skipped
- Error is recorded
- Processing continues

## Strict Mode

### Overview

In strict mode (`--strict`), Kelora fails immediately on the first error:

- **Parse errors**: Show error, abort immediately
- **Filter errors**: Show error, abort immediately
- **Transform errors**: Show error, abort immediately
- **Exit code**: Non-zero on any error

### When to Use

- Data validation pipelines
- CI/CD quality gates
- Critical processing where partial results aren't acceptable
- Debugging log parsing issues

### Example Behavior

```bash
> kelora -j --strict app.log --exec 'e.result = e.value.to_int() * 2'
```

**Input:**
```json
{"value": "123"}
{"value": "invalid"}
{"value": "456"}
```

**Output:**
```
result=246
⚠️  kelora: line 2: exec error - cannot convert 'invalid' to integer
```

**Exit code:** `1`

Processing stops at the first error. Line 3 is never processed.

### Enabling Strict Mode

```bash
> kelora -j --strict app.log
```

## Error Types

### Parse Errors

Occur when input lines can't be parsed in the specified format.

**JSON parse error:**
```bash
> kelora -j app.log
```

**Input:**
```
{"valid": "json"}
{invalid json}
{"more": "valid"}
```

**Resilient behavior:**

- Line 1: Parsed successfully
- Line 2: Skipped (parse error recorded)
- Line 3: Parsed successfully

**Strict behavior:**

- Line 1: Parsed successfully
- Line 2: Error shown, processing aborts
- Line 3: Never processed

### Filter Errors

Occur when `--filter` expressions fail during evaluation.

**Example:**
```bash
> kelora -j --filter 'e.timestamp.to_unix() > 1000000' app.log
```

If `e.timestamp` is missing:

**Resilient behavior:**

- Filter evaluates to `false`
- Event is skipped
- Error recorded

**Strict behavior:**

- Error shown immediately
- Processing aborts

### Transform Errors

Occur when `--exec` scripts fail during execution.

**Example:**
```bash
> kelora -j --exec 'e.result = e.value.to_int()' app.log
```

If `e.value` is not a valid integer:

**Resilient behavior:**

- Transformation rolled back (atomic)
- Original event returned unchanged
- Error recorded

**Strict behavior:**

- Error shown immediately
- Processing aborts

## Verbose Error Reporting

### Default Error Reporting

By default, errors are collected and summarized at the end:

```bash
> kelora -j app.log --exec 'e.result = e.value.to_int()'
```

**Summary:**
```
🔹 Processed 100 lines, 95 events output, 5 errors
```

### Verbose Mode (`--verbose`)

Show each error immediately as it occurs:

```bash
> kelora -j --verbose app.log --exec 'e.result = e.value.to_int()'
```

**Output:**
```
⚠️  kelora: line 5: exec error - cannot convert 'abc' to integer
result=123
⚠️  kelora: line 12: exec error - cannot convert 'def' to integer
result=456
⚠️  kelora: line 23: exec error - field 'value' not found
result=789
```

**Summary:**
```
🔹 Processed 100 lines, 95 events output, 5 errors
```

### Multiple Verbosity Levels

```bash
-v      # Show errors immediately
-vv     # Show errors with more context
-vvv    # Show errors with full details
```

### Verbose with Strict

Combine for immediate errors and fail-fast:

```bash
> kelora -j --strict --verbose app.log
```

Errors are shown immediately, then processing aborts.

## Quiet Modes

### Graduated Quiet Levels

Suppress output for automation:

| Level | Effect |
|-------|--------|
| `-q` | Suppress diagnostics (errors, stats, context markers) |
| `-qq` | Additionally suppress event output (same as `-F none`) |
| `-qqq` | Additionally suppress script side effects (`print()`, `eprint()`) |

### Level 1: Suppress Diagnostics

```bash
> kelora -q -j app.log --stats
```

- Errors not shown
- Stats not shown
- Events still output
- Exit code indicates success/failure

### Level 2: Suppress Events

```bash
> kelora -qq -j app.log --exec 'track_count(e.service)' --metrics
```

- Errors not shown
- Events not shown
- Metrics still shown (if `--metrics`)
- Exit code indicates success/failure

### Level 3: Complete Silence

```bash
> kelora -qqq -j app.log
```

- No output at all
- Exit code is only indicator
- Useful for validation pipelines

```bash
> kelora -qqq -j app.log && echo "Clean" || echo "Has errors"
```

## Exit Codes

Kelora uses standard Unix exit codes:

| Code | Meaning |
|------|---------|
| `0` | Success (no errors) |
| `1` | Processing errors (parse/filter/exec errors) |
| `2` | Invalid usage (CLI errors, file not found) |
| `130` | Interrupted (Ctrl+C) |
| `141` | Broken pipe (normal in Unix pipelines) |

### Using Exit Codes

**In shell scripts:**
```bash
if kelora -q -j app.log; then
    echo "✓ No errors found"
else
    echo "✗ Errors detected"
    exit 1
fi
```

**In CI/CD:**
```bash
kelora -qq --strict app.log || exit 1
```

**With automation:**
```bash
kelora -qqq app.log; echo "Exit code: $?"
```

## Atomic Transformations

### How It Works

In resilient mode, `--exec` scripts execute **atomically**:

```bash
> kelora -j --exec 'e.a = 1; e.b = e.value.to_int(); e.c = 3' app.log
```

If `e.value.to_int()` fails:

- Changes to `e.a` are **rolled back**
- `e.b` is never set
- `e.c` is never set
- **Original event** is returned unchanged

### Why Atomic?

Prevents partial transformations from corrupting data:

**Without atomicity:**
```json
// Input
{"value": "invalid"}

// Broken output (partial transformation)
{"value": "invalid", "a": 1}  // Missing b and c!
```

**With atomicity:**
```json
// Input
{"value": "invalid"}

// Output (unchanged)
{"value": "invalid"}  // Clean original event
```

### Multiple --exec Scripts

Each `--exec` script is atomic independently:

```bash
> kelora -j \
    --exec 'e.a = e.x.to_int()' \
    --exec 'e.b = e.y.to_int()' \
    app.log
```

If first `--exec` fails:

- First transformation rolled back
- Second `--exec` **still runs** on original event

If second `--exec` fails:

- First transformation **preserved** (it succeeded)
- Second transformation rolled back

## Common Error Scenarios

### Missing Fields

**Problem:**
```bash
> kelora -j --filter 'e.timestamp > "2024-01-01"' app.log
```

Some events missing `timestamp` field.

**Solution:** Use safe access:
```bash
> kelora -j --filter 'e.has_path("timestamp") && e.timestamp > "2024-01-01"' app.log
```

### Type Mismatches

**Problem:**
```bash
> kelora -j --exec 'e.result = e.value * 2' app.log
```

`e.value` is a string, not a number.

**Solution:** Use type conversion with defaults:
```bash
> kelora -j --exec 'e.result = to_int_or(e.value, 0) * 2' app.log
```

### Invalid Timestamps

**Problem:**
```bash
> kelora -j --filter 'e.timestamp.to_unix() > 1000000' app.log
```

`e.timestamp` is not a valid timestamp.

**Solution:** Use safe access:
```bash
> kelora -j --filter 'e.has_path("timestamp") && e.timestamp.to_unix() > 1000000' app.log
```

### Array Index Out of Bounds

**Problem:**
```bash
> kelora -j --exec 'e.first = e.items[0]' app.log
```

`e.items` is empty or missing.

**Solution:** Check array length:
```bash
> kelora -j --exec 'if e.has_path("items") && e.items.len() > 0 { e.first = e.items[0] }' app.log
```

### Division by Zero

**Problem:**
```bash
> kelora -j --exec 'e.ratio = e.success / e.total' app.log
```

`e.total` is zero.

**Solution:** Add guard:
```bash
> kelora -j --exec 'if e.total > 0 { e.ratio = e.success / e.total } else { e.ratio = 0.0 }' app.log
```

## Debugging Strategies

### Use Verbose Mode

See errors as they happen:

```bash
> kelora -j --verbose app.log --exec 'e.result = e.value.to_int()'
```

### Enable Strict Mode

Find first error quickly:

```bash
> kelora -j --strict app.log
```

### Inspect Problematic Lines

Use `--take` to limit processing:

```bash
> kelora -j --strict --take 100 app.log
```

Process only first 100 lines to find issues faster.

### Check Field Existence

Verify fields exist before accessing:

```bash
> kelora -j --exec 'if !e.has_path("value") { eprint("Line missing value: " + e) }' app.log
```

### Use Type Checking

Verify field types before operations:

```bash
> kelora -j --exec 'if type_of(e.value) != "i64" { eprint("Value is not integer: " + e.value) }' app.log
```

### Validate Input Format

Test parsing with strict mode:

```bash
> kelora -j --strict --stats-only app.log
```

No output, but exits with error if parsing fails.

## Error Messages

### Parse Error Format

```
⚠️  kelora: line 42: parse error - invalid JSON at position 15
```

- `line 42`: Line number in input
- `parse error`: Error category
- Details: Specific error message

### Filter Error Format

```
⚠️  kelora: line 42: filter error - field 'timestamp' not found
```

### Exec Error Format

```
⚠️  kelora: line 42: exec error - cannot convert 'abc' to integer
```

### Enhanced Error Summaries

With `--verbose`, get example errors:

```
🔹 Processed 1000 lines, 950 events output, 50 errors

Error examples:
  line 42: exec error - cannot convert 'abc' to integer
  line 103: exec error - field 'value' not found
  line 287: filter error - timestamp is null
```

## Best Practices

### Use Resilient Mode for Production

Production logs are messy - resilient mode handles gracefully:

```bash
> kelora -j app.log --levels error --keys timestamp,message
```

### Use Strict Mode for Validation

Validate data quality in pipelines:

```bash
> kelora -j --strict app.log > /dev/null && echo "✓ Valid"
```

### Combine Quiet and Exit Codes

For automation, use exit codes:

```bash
> kelora -qq app.log
if [ $? -eq 0 ]; then
    echo "No errors"
else
    echo "Has errors"
fi
```

### Add Defensive Checks

Use safe field access patterns:

```bash
> kelora -j --exec 'e.result = e.get_path("nested.value", 0) * 2' app.log
```

### Log Errors to File

Capture errors for later analysis:

```bash
> kelora -j --verbose app.log 2> errors.log
```

### Use Stats for Summary

Get error counts without verbose output:

```bash
> kelora -j --stats app.log
```

Shows error count in summary.

## Parallel Processing

### Error Handling in Parallel Mode

When using `--parallel`, error handling works the same:

```bash
> kelora -j --parallel app.log
```

- Errors still recorded per event
- Summary shows total errors across all threads
- Exit code reflects any errors from any thread

### Verbose with Parallel

Verbose errors are shown immediately, but may be interleaved:

```bash
> kelora -j --parallel --verbose app.log
```

Errors from different threads may appear out of order.

### Strict with Parallel

First error from any thread aborts all processing:

```bash
> kelora -j --parallel --strict app.log
```

## See Also

- [Pipeline Model](pipeline-model.md) - How error handling fits into processing stages
- [Scripting Stages](scripting-stages.md) - Error handling in --begin/--exec/--end
- [CLI Reference](../reference/cli-reference.md) - All error handling flags
- [Exit Codes Reference](../reference/exit-codes.md) - Complete exit code documentation
