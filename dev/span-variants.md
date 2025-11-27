# Span Variants: Field-Based and Idle-Based

**Status:** Design spec
**Date:** 2025-11-27
**Context:** Extend `--span` with field-based grouping and add explicit `--span-idle` for inactivity-based grouping (mutually exclusive with `--span`)

## Motivation

Current `--span` supports count-based (`--span N`) and time-based (`--span 5m`) aggregation. Two additional modes would significantly improve log analysis:

1. **Field-based spans**: Group events by field value changes (request_id, session_id, trace_id)
2. **Idle-based spans**: Detect sessions/bursts separated by inactivity gaps

Both fit the existing architecture and follow established patterns.

## Current Implementation

### Existing Span Modes

```rust
// src/config.rs:177-180
pub enum SpanMode {
    Count { events_per_span: usize },
    Time { duration_ms: i64 },
}
```

### Architecture Summary

- **Single active span model**: Only one span open at a time
- **Enum-based dispatch**: Match on SpanMode in prepare_event/record_emitted_event
- **Span lifecycle**: Open → Accumulate events → Close (trigger --span-close hook)
- **Error handling**: Follows `--strict` pattern (fail-fast vs resilient)
- **Interleaving behavior**: With one active span, interleaved IDs create multiple spans per ID (e.g., `req-1, req-2, req-1` yields two `req-1` spans)

### Key Files

- `src/config.rs`: CLI parsing, SpanMode enum, SpanConfig struct
- `src/pipeline/span.rs`: SpanProcessor, ActiveSpan, span lifecycle logic
- `src/cli.rs`: CLI flag definitions

## Proposed Extensions

### 1. Field-Based Spans

**Syntax:** `--span <field_name>` (auto-detected as field if not integer/duration)

**Behavior:**
- Open new span when field value changes
- Span ID = field value itself (e.g., "req-12345", "session-abc")
- Missing field: Continue current span (lenient), error with `--strict`

**Use Cases:**
```bash
# Group by request ID
kelora -j --span request_id --span-close 'print(span.id + ": " + span.size + " events")'

# Distributed tracing
kelora -j --span trace_id -F json > traces.jsonl

# Session analysis
kelora -j --span session_id --span-close 'track_sum("duration", span.metrics["response_time"].sum)'

# Strict mode: fail on missing field
kelora -j --span user_id --strict app.log
```

**Examples:**

Input with field changes:
```json
{"request_id": "req-1", "msg": "start"}
{"request_id": "req-1", "msg": "processing"}
{"request_id": "req-2", "msg": "start"}
{"request_id": "req-2", "msg": "done"}
```

Output (with --span-close):
```
req-1: 2 events
req-2: 2 events
```

Input with missing fields:
```json
{"request_id": "req-1", "msg": "a"}
{"msg": "b"}
{"request_id": "req-2", "msg": "c"}
```

Lenient behavior: Event without field stays in "req-1" span
```
req-1: 2 events
req-2: 1 event
```

Strict behavior: Error on second event
```
Error: event missing required field 'request_id' for --span
```

### 2. Idle-Based Spans

**Syntax:** `--span-idle <duration>` (separate flag; cannot be combined with `--span`)

**Behavior:**
- Close span after N duration of inactivity (no events)
- Requires timestamps (like time-based spans)
- Span ID = `idle-#<seq>-<start_timestamp>`

**Use Cases:**
```bash
# Session detection (5 min inactivity = new session)
kelora -j --span-idle 5m --span-close 'print("Session " + span.id + ": " + span.size + " events")'

# Error burst detection (10s quiet = separate burst)
kelora -j -l error --span-idle 10s --span-close 'print("Burst: " + span.size + " errors in " + span.duration)'

# Transaction grouping with timeout
kelora -j --span-idle 30s --span-close 'if span.size > 1 { print("Multi-event transaction") }'

# Strict mode: fail on missing timestamps
kelora -j --span-idle 5m --strict logs.jsonl
```

**Examples:**

Input with inactivity gaps:
```json
{"ts": "2025-01-15T10:00:00Z", "msg": "login"}
{"ts": "2025-01-15T10:00:03Z", "msg": "click"}
{"ts": "2025-01-15T10:06:00Z", "msg": "action"}
```

With `--span-idle 5m`:
```
Session idle-#0-2025-01-15T10:00:00Z: 2 events
Session idle-#1-2025-01-15T10:06:00Z: 1 event
```

## Implementation Plan

### Phase 1: Add SpanMode Variants

**File:** `src/config.rs` (lines 177-180)

```rust
pub enum SpanMode {
    Count { events_per_span: usize },
    Time { duration_ms: i64 },
    Field { field_name: String },  // NEW
    Idle { timeout_ms: i64 },      // NEW
}
```

### Phase 2: CLI Parsing with Auto-Detection

**File:** `src/config.rs` (parse_span_config, lines 1105-1144)

Auto-detection logic:
1. Try parse as `usize` → Count mode
2. Try parse as duration (via parse_duration) → Time mode
3. Else: Treat as field name → Field mode

Validation:
- Field name must be valid identifier (alphanumeric + underscore)
- Reject empty strings, reserved keywords

New flag:
```rust
// src/cli.rs
#[arg(long, value_name = "DURATION")]
/// Close span after this duration of inactivity (e.g., --span-idle 5m)
/// Requires events have valid timestamps
pub span_idle: Option<String>,
```

Parse `--span-idle`:
1. Parse duration using existing `parse_duration()`
2. Store as `SpanMode::Idle { timeout_ms }`
3. Conflict check: Cannot combine with `--span`

### Phase 3: Span Processor Logic

**File:** `src/pipeline/span.rs`

#### Field-Based Spans

**prepare_event() addition:**
```rust
SpanMode::Field { field_name } => {
    // Get field value from event
    let field_value = event.fields.get(field_name);

    match field_value {
        Some(value) => {
            let value_str = value.to_string();

            // Check if value changed
            let should_close = match &self.active_span {
                Some(span) => span.span_id != value_str,
                None => false,
            };

            if should_close {
                self.close_current_span(ctx)?;
            }

            if self.active_span.is_none() {
                self.open_field_span(value_str.clone(), ctx)?;
            }

            // Assign to span
            let assignment = SpanAssignment::new(SpanStatus::Included)
                .with_span(
                    self.active_span.as_ref().unwrap().span_id.clone(),
                    self.active_span.as_ref().unwrap().span_start,
                    self.active_span.as_ref().unwrap().span_end,
                );
            self.apply_assignment(event, ctx, &assignment);
            self.pending = Some(PendingEvent::new(assignment));
        }
        None => {
            // Missing field
            if ctx.config.strict {
                return Err(anyhow!(
                    "event missing required field '{}' for --span",
                    field_name
                ));
            }

            // Lenient: continue current span or open "(unset)"
            if self.active_span.is_none() {
                self.open_field_span("(unset)".to_string(), ctx)?;
            }

            let assignment = SpanAssignment::new(SpanStatus::Included)
                .with_span(
                    self.active_span.as_ref().unwrap().span_id.clone(),
                    None,
                    None,
                );
            self.apply_assignment(event, ctx, &assignment);
            self.pending = Some(PendingEvent::new(assignment));
        }
    }
}
```

**record_emitted_event():**
- No special logic needed (spans close in prepare_event)
- Just accumulate metrics as usual

**New helper methods:**
```rust
fn open_field_span(
    &mut self,
    field_value: String,
    ctx: &PipelineContext,
) -> Result<()> {
    let sequence = self.next_span_sequence;
    self.next_span_sequence += 1;

    let collect_details = self.compiled_close.is_some();
    self.active_span = Some(ActiveSpan::new_field(
        sequence,
        field_value,
        &ctx.user_trackers,
        collect_details,
    ));
    Ok(())
}
```

**ActiveSpan constructor:**
```rust
fn new_field(
    sequence: u64,
    field_value: String,
    baseline_user: &UserTrackers,
    collect_details: bool,
) -> Self {
    ActiveSpan {
        span_id: field_value,  // Use value as ID
        span_start: None,      // No time bounds
        span_end: None,
        sequence,
        events: Vec::new(),
        events_seen: 0,
        included_count: 0,
        baseline_user: baseline_user.clone(),
        collect_details,
        last_event_timestamp: None,  // Not used for field spans
    }
}
```

#### Idle-Based Spans

**Add to ActiveSpan struct:**
```rust
struct ActiveSpan {
    // ... existing fields
    pub last_event_timestamp: Option<DateTime<Utc>>,  // NEW: for idle tracking
}
```

**prepare_event() addition:**
```rust
SpanMode::Idle { timeout_ms } => {
    // Extract timestamp (required)
    if event.parsed_ts.is_none() {
        event.extract_timestamp();
    }

    let timestamp = match event.parsed_ts {
        Some(ts) => ts,
        None => {
            // Missing timestamp
            if ctx.config.strict {
                return Err(anyhow!(
                    "event missing required timestamp for --span-idle"
                ));
            }

            // Lenient: mark as unassigned
            let assignment = SpanAssignment::new(SpanStatus::Unassigned);
            self.apply_assignment(event, ctx, &assignment);
            self.pending = Some(PendingEvent::new(assignment));
            return Ok(());
        }
    };

    // Check for inactivity gap
    let should_close = match &self.active_span {
        Some(span) => {
            if let Some(last_ts) = span.last_event_timestamp {
                // Only forward gaps close spans; out-of-order events do not
                let gap_ms = timestamp.timestamp_millis()
                    - last_ts.timestamp_millis();
                gap_ms > *timeout_ms
            } else {
                false
            }
        }
        None => false,
    };

    if should_close {
        self.close_current_span(ctx)?;
    }

    if self.active_span.is_none() {
        self.open_idle_span(timestamp, ctx)?;
    }

    // Update last event timestamp
    if let Some(span) = &mut self.active_span {
        span.last_event_timestamp = Some(timestamp);
    }

    // Assign to span
    let assignment = SpanAssignment::new(SpanStatus::Included)
        .with_span(
            self.active_span.as_ref().unwrap().span_id.clone(),
            self.active_span.as_ref().unwrap().span_start,
            self.active_span.as_ref().unwrap().span_end,
        );
    self.apply_assignment(event, ctx, &assignment);
    self.pending = Some(PendingEvent::new(assignment));
}
```

**New helper method:**
```rust
fn open_idle_span(
    &mut self,
    start_ts: DateTime<Utc>,
    ctx: &PipelineContext,
) -> Result<()> {
    let sequence = self.next_span_sequence;
    self.next_span_sequence += 1;

    let collect_details = self.compiled_close.is_some();
    self.active_span = Some(ActiveSpan::new_idle(
        sequence,
        start_ts,
        &ctx.user_trackers,
        collect_details,
    ));
    Ok(())
}
```

**ActiveSpan constructor:**
```rust
fn new_idle(
    sequence: u64,
    start_ts: DateTime<Utc>,
    baseline_user: &UserTrackers,
    collect_details: bool,
) -> Self {
    let span_id = format!("idle-#{}-{}", sequence, start_ts.to_rfc3339());
    ActiveSpan {
        span_id,
        span_start: Some(start_ts),
        span_end: None,  // Will be set when closed
        sequence,
        events: Vec::new(),
        events_seen: 0,
        included_count: 0,
        baseline_user: baseline_user.clone(),
        collect_details,
        last_event_timestamp: Some(start_ts),
    }
}
```

### Phase 4: Update Existing Constructors

Update `new_count()` and `new_time()` to initialize `last_event_timestamp`:
```rust
fn new_count(...) -> Self {
    ActiveSpan {
        // ... existing fields
        last_event_timestamp: None,  // Not used for count spans
    }
}

fn new_time(...) -> Self {
    ActiveSpan {
        // ... existing fields
        last_event_timestamp: None,  // Not needed (has span_start/end)
    }
}
```

### Phase 5: Documentation

**File:** `docs/reference/cli-reference.md`

Update `--span` section:
```markdown
#### `--span <N | DURATION | FIELD>`

Aggregate events into consecutive spans. Kelora auto-detects the mode:

- `--span <N>` – Count-based spans. Close after every **N** events that survive all filters.
- `--span <DURATION>` – Time-based spans aligned to event timestamps (e.g., `1m`, `5m`, `1h`).
- `--span <FIELD>` – Field-based spans. Close when the specified field value changes.

**Examples:**

Count-based:
```bash
kelora -j --span 100 --span-close 'print("Batch: " + span.size)' app.log
```

Time-based:
```bash
kelora -j --span 5m --span-close 'print("Window: " + span.id)' app.log
```

Field-based:
```bash
kelora -j --span request_id --span-close 'print("Request " + span.id + ": " + span.size + " events")' app.log
```

**Field-based behavior:**
- Opens new span when field value changes
- Span ID is the field value itself
- Missing field: continues current span (default), errors with `--strict`
- First event with missing field: opens span with ID `(unset)`
- Interleaved IDs create multiple spans per ID (single-active-span model)

#### `--span-idle <DURATION>`

Close span after the specified duration of inactivity (no events). Requires events to have valid timestamps.

**Examples:**

Session detection:
```bash
kelora -j --span-idle 5m --span-close 'print("Session: " + span.size + " events")' app.log
```

Error burst analysis:
```bash
kelora -j -l error --span-idle 10s --span-close 'print("Burst: " + span.size)' app.log
```

**Behavior:**
- Span closes when gap between events exceeds timeout
- Span ID format: `idle-#<seq>-<start_timestamp>`
- Missing timestamp: event marked as unassigned (default), errors with `--strict`
- Cannot combine with `--span` (mutually exclusive)
```

**File:** `docs/tutorials/span-aggregation.md`

Add new section after time-based examples:

```markdown
## Field-Based Spans

Group events by a field value. Kelora opens a new span each time the field changes.

### Basic Field Grouping

Group all logs by request ID:

```bash
kelora -j examples/requests.jsonl \
      --span request_id \
      --span-close 'print("Request " + span.id + ": " + span.size + " events")'
```

**Input:**
```json
{"request_id": "req-123", "action": "start"}
{"request_id": "req-123", "action": "query_db"}
{"request_id": "req-456", "action": "start"}
{"request_id": "req-456", "action": "done"}
```

**Output:**
```
Request req-123: 2 events
Request req-456: 2 events
```

### Missing Fields

By default, events without the tracked field stay in the current span:

```bash
echo '{"request_id":"req-1","msg":"a"}
{"msg":"b"}
{"request_id":"req-2","msg":"c"}' | \
kelora -f json --span request_id --span-close 'print(span.id + ": " + span.size)'
```

**Output:**
```
req-1: 2
req-2: 1
```

Use `--strict` to error on missing fields:

```bash
kelora -j --span session_id --strict app.log
# Error: event missing required field 'session_id' for --span
```

### Per-Request Metrics

Combine with metric tracking for per-request analysis:

```bash
kelora -j logs.jsonl \
      --span request_id \
      --exec 'track_sum("bytes", e.bytes); track_max("latency", e.duration)' \
      --span-close '
        print(span.id + ": " + span.size + " events");
        print("  Total bytes: " + span.metrics["bytes"].sum);
        print("  Max latency: " + span.metrics["latency"].max);
      '
```

## Idle-Based Spans (Session Detection)

Close spans after a period of inactivity. Useful for session analysis and burst detection.

### Basic Session Detection

Detect user sessions with 5-minute timeout:

```bash
kelora -j app.log \
      --span-idle 5m \
      --span-close 'print("Session " + span.id + ": " + span.size + " events")'
```

Events arriving more than 5 minutes after the previous event start a new span.

### Error Burst Analysis

Group errors into bursts separated by quiet periods:

```bash
kelora -j app.log \
      -l error \
      --span-idle 10s \
      --span-close '
        print("Error burst:");
        print("  Count: " + span.size);
        print("  First: " + span.start);
        print("  Last: " + span.end);
      '
```

### Missing Timestamps

Events without timestamps are excluded from spans (marked `unassigned`):

```bash
kelora -j --span-idle 5m app.log
# Events without 'ts' are skipped
```

Use `--strict` to error on missing timestamps:

```bash
kelora -j --span-idle 5m --strict app.log
# Error: event missing required timestamp for --span-idle
```
```

### Phase 6: Tests

**File:** `tests/metrics_tracking_tests.rs` (or new `tests/span_variants_tests.rs`)

```rust
#[test]
fn test_field_span_basic() {
    let input = r#"{"request_id":"req-1","msg":"a"}
{"request_id":"req-1","msg":"b"}
{"request_id":"req-2","msg":"c"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f", "json",
            "--span", "request_id",
            "--span-close", "print(span.id + ':' + span.size.to_string());",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("req-1:2"));
    assert!(stdout.contains("req-2:1"));
}

#[test]
fn test_field_span_missing_field_lenient() {
    let input = r#"{"request_id":"req-1","msg":"a"}
{"msg":"b"}
{"request_id":"req-2","msg":"c"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f", "json",
            "--span", "request_id",
            "--span-close", "print(span.id + ':' + span.size.to_string());",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    // Event without field stays in req-1 span
    assert!(stdout.contains("req-1:2"));
    assert!(stdout.contains("req-2:1"));
}

#[test]
fn test_field_span_missing_field_strict() {
    let input = r#"{"msg":"test"}"#;

    let (_stdout, stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--span", "request_id", "--strict"],
        input,
    );

    assert_eq!(exit_code, 1);
    assert!(stderr.contains("missing required field 'request_id'"));
}

#[test]
fn test_field_span_first_event_missing() {
    let input = r#"{"msg":"a"}
{"request_id":"req-1","msg":"b"}
{"request_id":"req-1","msg":"c"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f", "json",
            "--span", "request_id",
            "--span-close", "print(span.id + ':' + span.size.to_string());",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("(unset):1"));
    assert!(stdout.contains("req-1:2"));
}

#[test]
fn test_idle_span_basic() {
    let input = r#"{"ts":"2025-01-15T10:00:00Z","msg":"a"}
{"ts":"2025-01-15T10:00:03Z","msg":"b"}
{"ts":"2025-01-15T10:00:10Z","msg":"c"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f", "json",
            "--span-idle", "5s",
            "--span-close", "print(span.size.to_string());",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("2")); // First two events in one span
    assert!(stdout.contains("1")); // Third event in new span (gap > 5s)
}

#[test]
fn test_idle_span_missing_timestamp_lenient() {
    let input = r#"{"ts":"2025-01-15T10:00:00Z","msg":"a"}
{"msg":"b"}
{"ts":"2025-01-15T10:00:03Z","msg":"c"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f", "json",
            "--span-idle", "5s",
            "--span-close", "print(span.size.to_string());",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    // Event without timestamp is unassigned, not counted
    assert!(stdout.contains("2"));
}

#[test]
fn test_idle_span_missing_timestamp_strict() {
    let input = r#"{"msg":"test"}"#;

    let (_stdout, stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--span-idle", "5s", "--strict"],
        input,
    );

    assert_eq!(exit_code, 1);
    assert!(stderr.contains("missing required timestamp"));
}

#[test]
fn test_span_auto_detection() {
    // Integer -> count
    let (_stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--span", "5"],
        r#"{"a":1}"#,
    );
    assert_eq!(exit_code, 0);

    // Duration -> time
    let (_stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--span", "5m"],
        r#"{"ts":"2025-01-15T10:00:00Z","a":1}"#,
    );
    assert_eq!(exit_code, 0);

    // Identifier -> field
    let (_stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--span", "request_id"],
        r#"{"request_id":"req-1","a":1}"#,
    );
    assert_eq!(exit_code, 0);
}
```

## Error Handling

Both modes follow the existing `--strict` pattern:

### Field-Based Spans

**Without `--strict` (default/lenient):**
- Missing field → continue current span
- First event with missing field → open span with ID `(unset)`
- Event still processed normally

**With `--strict`:**
- Missing field → fatal error: "event missing required field 'X' for --span"
- Pipeline aborts (exit code 1)

**Matches:** Time-based span behavior for missing timestamps (src/pipeline/span.rs:301-303)

### Idle-Based Spans

**Without `--strict` (default/lenient):**
- Missing timestamp → mark as `SpanStatus::Unassigned`
- Event not included in any span
- Event still processed/output

**With `--strict`:**
- Missing timestamp → fatal error: "event missing required timestamp for --span-idle"
- Pipeline aborts (exit code 1)

**Matches:** Time-based span behavior exactly (src/pipeline/span.rs:301-303)

## Edge Cases

### Field-Based

1. **Field value is empty string:** Span ID = `""` (valid)
2. **Field value contains special chars:** Used as-is in span ID
3. **Field is not a string:** Convert to string via `to_string()`
4. **Field is array/object:** String representation (e.g., `"[1,2,3]"`, `"{"a":1}"`)
5. **All events missing field:** Single span with ID `(unset)`
6. **Field exists but is null:** Treat as distinct value, span ID = `"null"`

### Idle-Based

1. **Events arrive out of order:** Gap calculated from last *seen* event (not sorted); single-active-span model keeps interleaving simple
2. **First event missing timestamp:** Span not opened until first valid timestamp
3. **Gap exactly equals timeout:** New span opened (gap > timeout, not >=)
4. **Very large timeout (years):** Supported, stored as i64 milliseconds
5. **Negative timestamp gaps:** Ignored for closing; spans only close on forward-time gaps

## Migration Path

### Backward Compatibility

✅ **No breaking changes:**
- Existing `--span <N>` and `--span <DURATION>` still work
- Auto-detection tries count/time parsing first
- Field names start with alphabetic characters in Kelora logs, so collisions with integers/durations are not expected

⚠️ **Potential ambiguity:**
- If a nonstandard field name resembled a duration/integer it would still be parsed as time/count
- Could add explicit `--span-field` flag if needed, but not required with current field naming convention

### Configuration Files

Field/idle spans work in `.kelora.ini`:
```ini
[default]
span = request_id
span-close = print(span.id + ": " + span.size);

[sessions]
span-idle = 5m
span-close = print("Session: " + span.size);
```

## Implementation Checklist

- [ ] Add `Field` and `Idle` variants to `SpanMode` enum
- [ ] Implement auto-detection in `parse_span_config()`
- [ ] Add `--span-idle` CLI flag
- [ ] Add `last_event_timestamp` to `ActiveSpan` struct
- [ ] Implement field span logic in `prepare_event()`
- [ ] Implement idle span logic in `prepare_event()`
- [ ] Add `new_field()` and `new_idle()` constructors
- [ ] Add `open_field_span()` and `open_idle_span()` helpers
- [ ] Update existing constructors for new field
- [ ] Write unit tests (10+ test cases)
- [ ] Update `--span` documentation
- [ ] Add `--span-idle` documentation
- [ ] Add tutorial examples
- [ ] Run `just check` (fmt + lint + test)
- [ ] Manual testing with real logs

## Estimated Effort

- **Implementation:** 5-7 hours
- **Tests:** 2-3 hours
- **Documentation:** 1-2 hours
- **Total:** 8-12 hours

## Future Extensions (Out of Scope)

Not included in this spec:

1. **Marker-based spans** (`--span-marker EXPR`): Close when Rhai condition is true
2. **Byte-size spans** (`--span-size 1MB`): Close at byte threshold
3. **Combined spans** (`--span request_id --span-idle 5m`): Multiple criteria
4. **Sliding windows**: Multiple overlapping spans (architectural change)
5. **Custom span IDs** (`--span-id-format`): User-defined ID templates

These can be added later if needed.

## Open Questions

None. Design approved by user.
