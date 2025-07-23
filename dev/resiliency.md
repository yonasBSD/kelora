# Kelora: Resilient Script Execution

## Philosophy

Process what you can, skip what you can't, report what happened.

Kelora is designed for real-world data - messy, incomplete, and surprising. Instead of failing on the first error, it extracts maximum value from your data while keeping you informed about what couldn't be processed.

## Core Design: Three Contexts, Three Behaviors

Different operations have different error semantics, and Kelora respects this:

### 1. Input Parsing: Skip Unparseable Lines

When reading data, garbage lines are skipped and counted:

```bash
# Input: mix of valid and invalid JSON
{"user": "alice", "status": 200}
{broken json
{"user": "bob", "status": 404}

# Output: valid events continue through the pipeline
{"user": "alice", "status": 200}
{"user": "bob", "status": 404}

# Summary shows what happened:
✓ Processed 2 events
🧱 1 line could not be parsed
```

### 2. Filtering: Errors Equal False

Filters are boolean predicates. Any error evaluates to false, naturally filtering out incomplete data:

```bash
# Events:
{"user": {"role": "admin"}, "action": "delete"}
{"user": {"role": "user"}, "action": "read"}  
{"action": "write"}  # missing user field

# Filter:
--filter 'e.user.role == "admin"'

# Result: only first event passes
# The third event fails at e.user (returns ()) and is filtered out
# No error reported - this is expected behavior
```

### 3. Transformations: Atomic Stages with Rollback

Each `--exec` stage either completes fully or leaves the event unchanged:

```bash
# Three transformation stages:
--exec 'e.user_upper = e.user.to_uppercase()'  
--exec 'e.is_admin = e.user.role == "admin"'
--exec 'e.timestamp_hour = e.timestamp.parse_timestamp().hour()'

# For an event missing 'user':
# - Stage 1: fails, event unchanged (no partial modification)
# - Stage 2: runs with original event
# - Stage 3: runs independently

# Each event gets as many enhancements as possible
```

## Understanding Field Access

Kelora uses Rhai's unit type `()` for missing fields:

```rhai
e.missing         // Returns () - the unit type
e.missing.field   // ERROR: cannot access property on ()
```

This creates a learning moment - when you see this error, you know you're accessing a field that doesn't exist in some events. The solution is explicit:

```bash
# Check existence first
--filter 'e.user && e.user.role == "admin"'

# Or use safe accessor with default
--filter 'get_path(e, "user.role", "guest") == "admin"'
```

## Variables and Data Model

Only three variables exist:

- **`e`** - The current event with all its fields
- **`meta`** - Metadata (`line`, `line_number`)
- **`window`** - Array of recent events (when `--window N` is used)

Transform events by adding fields to `e`:

```bash
# Simple transformations
--exec 'e.name_lower = e.name.to_lowercase()'
--exec 'e.full_name = e.first + " " + e.last'  

# Complex logic
--exec 'e.timestamp_parsed = parse_timestamp(e.timestamp)'
--exec 'e.hour = e.timestamp_parsed.hour()'
--exec 'e.is_business_hours = e.hour >= 9 && e.hour < 17'

# Input:  {"name": "Alice", "status": 500}
# Output: {"name": "Alice", "status": 500, "name_lower": "alice", "is_error": true}
```

## Error Reporting: Progressive Detail

### Default: Summary Statistics

See the big picture of what was processed:

```
✓ Processed 10,000 events
📊 Filtered out 2,341 events
🧱 23 lines could not be parsed
⚡ 156 events had stage errors
```

### Verbose Mode (`-v`): Understand Issues

See specific errors with helpful hints:

```
🧱 Line 42: invalid JSON - unexpected character at position 15
⚡ Line 89: stage 2 failed - cannot access 'role' on missing field 'user'
   Filter: e.user.role == "admin"
           ^^^^^^^^^^^
   Hint: Use 'e.user && e.user.role == "admin"' for safe access
```

### Strict Mode (`--strict`): Fail Fast

For production pipelines that require perfection:

```
🧱 kelora: line 42: parse error - invalid JSON
# Exit code 1
```

## Built-in Safety Functions

For common patterns, Kelora provides safe alternatives:

```bash
# Safe field access with defaults
get_path(e, "user.role", "guest")           # Returns "guest" if missing
get_path(e, "metrics.cpu.usage", 0.0)       # Returns 0.0 if missing

# Type conversion with defaults
to_number(e.amount, 0)                      # "100.50" → 100.5, default 0
to_bool(e.active, false)                    # "yes"/"1"/"true" → true

# Existence checking
has_path(e, "user.email")                   # true/false
path_equals(e, "user.role", "admin")        # Safe equality check
```

## Real-World Patterns

### Progressive Enhancement

Each stage adds what value it can:

```bash
kelora -f jsonl logs \
  --exec 'e.has_user = e.user != ()' \
  --exec 'e.user_type = get_path(e, "user.role", "guest")' \
  --exec 'e.session_upper = e.session.id.to_uppercase()' \
  --exec 'e.is_long = e.duration > 3600'

# Events missing 'session' still get user_type and is_long
# Every event is enhanced as much as possible
```

### Window Analysis

Detect patterns across events:

```bash
kelora -f jsonl auth.log --window 3 \
  --filter 'e.user != window[1].user' \
  --exec 'e.user_changed = true' \
  --exec 'e.previous_user = get_path(window[1], "user", "unknown")'
```

### Multi-Stage Processing

Build complex pipelines with confidence:

```bash
kelora -f jsonl api.log \
  --filter 'e.method == "POST"' \
  --exec 'e.endpoint_category = e.path.split("/")[2]' \
  --exec 'e.is_api_v2 = e.path.starts_with("/api/v2/")' \
  --exec 'e.response_time_ms = e.response_time * 1000' \
  --filter 'e.response_time_ms > 1000' \
  --exec 'e.alert_team = get_path(e, "endpoint_category", "unknown") + "_oncall"'
```

## Why This Design

Traditional tools make you choose:
- **Strict**: Fail on first error (jq)
- **Defensive**: Write verbose error handling everywhere
- **Complex**: Configure schemas, dead-letter queues, error policies

Kelora chooses differently:
- **Resilient**: Process everything possible
- **Informative**: Know what was skipped and why
- **Progressive**: Start simple, add safety as needed

This matches how people actually explore and transform data.

## Complete Example

Process web logs with realistic complexity:

```bash
kelora -f jsonl production.log \
  --filter 'e.status >= 400' \
  --exec 'e.is_client_error = e.status >= 400 && e.status < 500' \
  --exec 'e.is_server_error = e.status >= 500' \
  --exec 'e.endpoint = e.path.split("?")[0]' \
  --exec 'e.user_id = get_path(e, "session.user_id", "anonymous")' \
  --exec 'e.error_type = e.error.type.to_lowercase()' \
  --filter 'e.is_server_error' \
  --exec 'track_count("errors_by_endpoint", e.endpoint)' \
  --exec 'track_unique("affected_users", e.user_id)'

# Output:
✓ Processed 1,000,000 events
📊 Filtered out 987,234 events (non-errors, then non-500s)
⚡ 1,234 events had stage errors (missing error.type)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Server Errors by Endpoint:
  /api/payment: 234
  /api/auth: 156
  /api/users: 89

Affected Users: 3,421
```

## Command Line Interface

```
--filter EXPR      Boolean filter (can be chained)
--exec SCRIPT      Transform stage (can be chained)  
--window N         Access N previous events in 'window' array
-v, --verbose      Show detailed error information
--strict           Exit on first error
```

## The Kelora Promise

Write natural scripts:
```bash
--exec 'e.alert = e.error.severity == "critical"'
```

Not defensive code:
```bash
# NOT NEEDED:
--exec 'if e.error && e.error.severity { 
          e.alert = e.error.severity == "critical" 
        }'
```

**Process what you can. Skip what you can't. Keep the data flowing.**

That's Kelora - data processing that works with the real world, not against it.