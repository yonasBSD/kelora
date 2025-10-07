# Parsing Custom Formats

Learn how to parse custom log formats using Kelora's flexible column format specification.

## What You'll Learn

By the end of this tutorial, you'll be able to:

- Parse fixed-width and delimited custom log formats
- Use column specifications with field names and widths
- Apply type annotations for automatic conversion
- Handle timestamp fields and greedy captures
- Combine custom parsing with filtering and transformation

## Prerequisites

- Completed the [Quickstart](../quickstart.md)
- Basic understanding of log formats
- Familiarity with command-line operations

## Overview

Many applications generate custom log formats that don't match standard formats like JSON or syslog. Kelora's `cols:<spec>` format lets you define custom column specifications to parse these logs.

**Time:** ~15 minutes

## Step 1: Understanding Column Specifications

The basic syntax for column format is:

```
-f 'cols:field1:width field2:width field3:*'
```

**Field specification parts:**
- `field1` - Field name
- `:width` - Field width (optional, whitespace-delimited if omitted)
- `*` - Greedy capture (consumes remainder of line)

### Example Log Format

Let's start with a simple application log:

```
ERROR api Connection failed
INFO  db  Query executed
WARN  api Slow response time
```

Each line has three whitespace-delimited fields: level, service, message.

## Step 2: Basic Whitespace-Delimited Parsing

For whitespace-delimited fields, just list the field names:

```bash exec="on" source="above" result="ansi"
echo "ERROR api Connection failed
INFO  db  Query executed
WARN  api Slow response time" | kelora -f 'cols:level service message'
```

**How it works:**
- `level` - First whitespace-delimited field
- `service` - Second whitespace-delimited field
- `message` - Third whitespace-delimited field (captures rest of line)

## Step 3: Fixed-Width Fields

For fixed-width columns, specify the character width:

```bash
> echo "ERROR     api       Connection failed" | \
    kelora -f 'cols:level:10 service:10 message:*'
```

**Column widths:**
- `level:10` - First 10 characters
- `service:10` - Next 10 characters
- `message:*` - Remainder of line

The `*` wildcard must be the last field and consumes everything remaining.

## Step 4: Adding Type Annotations

Convert fields to specific types using annotations:

```bash
> echo "200 1234 0.123
404 5678 0.456
500 9012 0.789" | \
    kelora -f 'cols:status:int bytes:int response_time:float'
```

**Supported types:**
- `:int` - Parse as integer
- `:float` - Parse as floating-point number
- `:bool` - Parse as boolean

After type conversion, you can use numeric operations:

```bash
> echo "200 1234 0.123" | \
    kelora -f 'cols:status:int bytes:int response_time:float' \
    --filter 'e.status >= 400'
```

## Step 5: Handling Timestamps

Use `:ts` for timestamp fields that Kelora should recognize:

```bash
> echo "2024-01-15T10:30:00Z ERROR Connection failed" | \
    kelora -f 'cols:timestamp:ts level message'
```

The `:ts` annotation tells Kelora to:
- Parse the field as a timestamp
- Make it available for time-based filtering (`--since`, `--until`)
- Use it for time-related functions

## Step 6: Custom Separators

For non-whitespace separators, use `--cols-sep`:

```bash
> echo "ERROR|api|Connection failed
INFO|db|Query executed" | \
    kelora -f 'cols:level service message' --cols-sep '|'
```

Works with any separator string:

```bash
# Comma-separated
> kelora -f 'cols:level service message' --cols-sep ','

# Tab-separated
> kelora -f 'cols:level service message' --cols-sep $'\t'

# Multi-character separator
> kelora -f 'cols:level service message' --cols-sep ' :: '
```

## Step 7: Real-World Example - Application Logs

Let's parse a custom application log format:

**Log format:**
```
[2024-01-15 10:30:00] ERROR api Connection failed to database
[2024-01-15 10:30:01] INFO  db  Query executed successfully
[2024-01-15 10:30:02] WARN  api Slow response: 2500ms
```

**Column specification:**
```bash
> kelora -f 'cols:timestamp:ts level:5 service:4 message:*' app.log
```

But the timestamp is wrapped in brackets. We need to extract it first.

### Using Regex in Exec

```bash
> kelora -f 'cols:raw_ts level:5 service:4 message:*' app.log \
    --exec 'e.timestamp = e.raw_ts.extract_re(r"\[(.*?)\]", 1)' \
    --exec 'e.raw_ts = ()' \
    --keys timestamp,level,service,message
```

**What this does:**
1. Parse `raw_ts` as the first field (including brackets)
2. Extract timestamp from brackets using regex
3. Remove `raw_ts` field (no longer needed)
4. Output cleaned fields

## Step 8: Combining with Transformations

Parse custom format and add computed fields:

```bash
> cat app.log | \
    kelora -f 'cols:timestamp:ts level:5 service:4 message:*' \
    --exec 'if e.message.contains("ms") { e.duration = e.message.extract_re(r"(\d+)ms", 1).to_int() }' \
    --filter 'e.has_path("duration") && e.duration > 1000' \
    --keys timestamp,service,duration,message
```

**Pipeline:**
1. Parse custom format
2. Extract duration from message if present
3. Filter for slow requests (>1000ms)
4. Output relevant fields

## Step 9: Working with Mixed-Width Fields

Some logs have consistent positions but variable content:

```
ERROR  2024-01-15 Connection failed
INFO   2024-01-15 Query OK
WARN   2024-01-15 High memory usage
```

Use whitespace delimiter for variable-width fields:

```bash
> kelora -f 'cols:level timestamp message' app.log
```

Or specify exact positions if alignment is strict:

```bash
> kelora -f 'cols:level:7 timestamp:11 message:*' app.log
```

## Step 10: Complete Example

Let's parse a complex custom format with everything we've learned:

**Input log (custom_app.log):**
```
[2024-01-15 10:30:00] 200 api    1234 0.123 Success
[2024-01-15 10:30:01] 404 web    5678 0.456 Not found
[2024-01-15 10:30:02] 500 api    9012 0.789 Server error
```

**Parse specification:**
```bash
> kelora -f 'cols:raw_ts status:int service:7 bytes:int latency:float message:*' custom_app.log \
    --exec 'e.timestamp = e.raw_ts.extract_re(r"\[(.*?)\]", 1)' \
    --exec 'e.raw_ts = ()' \
    --exec 'e.is_error = e.status >= 400' \
    --exec 'e.is_slow = e.latency > 0.5' \
    --filter 'e.is_error || e.is_slow' \
    --keys timestamp,status,service,message
```

**What happens:**
1. Parse fields with type annotations
2. Extract timestamp from brackets
3. Add `is_error` flag for HTTP errors
4. Add `is_slow` flag for high latency
5. Filter for errors or slow requests
6. Output cleaned fields

## Common Patterns

### Pattern 1: Log Level + Message

```bash
> kelora -f 'cols:level message' app.log --levels error,warn
```

### Pattern 2: Timestamp + Level + Service + Message

```bash
> kelora -f 'cols:timestamp:ts level:5 service:10 message:*' app.log \
    --filter 'e.level == "ERROR"'
```

### Pattern 3: Fixed-Width with Type Conversion

```bash
> kelora -f 'cols:status:int bytes:int duration:float path:*' access.log \
    --filter 'e.status >= 500' \
    --exec 'track_avg("latency", e.duration)' \
    --metrics
```

### Pattern 4: Extract and Transform

```bash
> kelora -f 'cols:timestamp level data:*' app.log \
    --exec 'e.values = e.data.split(",")' \
    --exec 'e.count = e.values.len()' \
    --keys timestamp,level,count
```

## Tips and Best Practices

### Use Whitespace Delimiters When Possible

Simpler and more flexible:

```bash
# Good - whitespace delimited
> kelora -f 'cols:level service message' app.log

# Avoid unless necessary - fixed width
> kelora -f 'cols:level:10 service:10 message:*' app.log
```

### Use Type Annotations Early

Convert types during parsing, not in exec:

```bash
# Good - parse as int
> kelora -f 'cols:status:int bytes:int' --filter 'e.status >= 400'

# Less efficient - convert in exec
> kelora -f 'cols:status bytes' --exec 'e.status = e.status.to_int()' --filter 'e.status >= 400'
```

### Name Fields Descriptively

```bash
# Good
> kelora -f 'cols:timestamp level service message'

# Less clear
> kelora -f 'cols:col1 col2 col3 col4'
```

### Use Greedy Capture for Messages

Always use `*` for the last field if it's a message:

```bash
> kelora -f 'cols:level service message:*'
```

### Combine with Extract Prefix

For Docker Compose-style logs:

```bash
> docker compose logs | \
    kelora --extract-prefix container \
           -f 'cols:timestamp:ts level message:*'
```

## Troubleshooting

### Fields Not Parsing Correctly

**Problem:** Fields are misaligned or missing.

**Solution:** Check separator and field widths:
```bash
# Debug by outputting all fields
> kelora -f 'cols:field1 field2 field3:*' app.log --take 3

# Try different separator
> kelora -f 'cols:field1 field2 field3' --cols-sep '|' app.log --take 3
```

### Timestamp Not Recognized

**Problem:** Timestamp field not working with `--since`.

**Solution:** Use `:ts` annotation and verify format:
```bash
> kelora -f 'cols:timestamp:ts level message' app.log --ts-format '%Y-%m-%d %H:%M:%S'
```

### Type Conversion Failures

**Problem:** Integer/float conversions failing.

**Solution:** Use `get_path` with defaults in resilient mode:
```bash
> kelora -f 'cols:status bytes' --exec 'e.status_int = to_int_or(e.status, 0)'
```

### Greedy Field Capturing Too Much

**Problem:** Last field includes separators.

**Solution:** Be specific with earlier fields:
```bash
# If fields are fixed width, specify widths
> kelora -f 'cols:level:10 service:10 message:*'
```

## Next Steps

Now that you understand custom format parsing, explore:

- **[Working with Time](working-with-time.md)** - Advanced timestamp handling
- **[Format Reference](../reference/formats.md)** - Complete format documentation
- **[Scripting Transforms](scripting-transforms.md)** - Advanced event transformation

## See Also

- [Format Reference](../reference/formats.md) - All supported formats
- [CLI Reference](../reference/cli-reference.md) - Complete flag documentation
- [Function Reference](../reference/functions.md) - String manipulation functions
