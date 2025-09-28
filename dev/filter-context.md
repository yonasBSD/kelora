# Context Options (-A, -B, -C) Implementation Specification

## Overview

This document specifies the implementation of grep-style context options for Kelora, allowing users to see surrounding lines for matched events. The implementation leverages Kelora's existing window feature and maintains the lock-free parallel processing architecture.

## User Interface

### CLI Arguments

- `-A N` / `--after-context N`: Show N lines after each match
- `-B N` / `--before-context N`: Show N lines before each match
- `-C N` / `--context N`: Show N lines before and after each match (equivalent to `-A N -B N`)

### Usage Examples

```bash
# Show 2 lines after each error
kelora -f json app.log --filter 'e.level == "error"' -A 2

# Show 3 lines before each match
kelora -f json app.log --filter 'e.status >= 400' -B 3

# Show 2 lines before and after each match
kelora -f json app.log --filter 'e.user == "admin"' -C 2

# Context with other features
kelora -f json app.log --filter 'e.level == "error"' -A 2 --window 10 --parallel
```

### Validation Rules

- Context values must be non-negative integers
- `-C N` sets both before and after context to N
- Context options require active filtering (--filter, --since, --until, etc.)
- Error if context requested without any filtering expressions

## Architecture

### Match Definition

An event is considered a "match" if it:
1. Passes through all filtering stages (--filter, --since, --until, etc.)
2. Would normally be output without context options
3. Survives emit_each() fan-out processing

### Pipeline Integration

```
Input → Parse → Filter → [Context Stage] → Format → Output
                   ↓
               Match Events
```

The context stage is inserted between filtering and formatting:
- Receives stream of matched events
- Uses window to access recent event history
- Applies context logic to determine which events to output
- Adds context metadata to events for formatting

### Window Integration

Context processing leverages the existing `--window` feature:

**Window Size Calculation:**
```rust
effective_window_size = user_window_size.max(before_context + after_context + 1)
```

**Window Contents:**
- `window[0]`: Current event being processed
- `window[1..before_context+1]`: Recent events for before-context
- Look-ahead buffer: Next `after_context` events for after-context

## Context Processing Algorithm

### Sequential Mode

**State Management:**
```rust
struct ContextState {
    before_buffer: VecDeque<Event>,  // Size: before_context
    after_buffer: VecDeque<Event>,   // Size: after_context
    pending_after: usize,            // Countdown for after-context
    recent_matches: VecDeque<usize>, // Track match positions
}
```

**Processing Flow:**
1. **Event Arrival:**
   - Add event to after_buffer
   - If buffer full, process oldest event for context decision

2. **Context Decision:**
   - Check if current event is a match
   - If match: mark surrounding events in buffers
   - Update pending_after counter
   - Output events with context metadata

3. **Before Context:**
   - When match found, mark last N events in before_buffer as "Before"
   - Overlap handling: merge overlapping context ranges

4. **After Context:**
   - When match found, set pending_after = after_context
   - Mark next N events as "After" while decrementing counter

### Parallel Mode

**Worker-Level Processing:**
- Each worker processes its batch independently
- Context decisions made within batch boundaries only
- No cross-batch context coordination (acceptable trade-off for lock-free design)
- Window mechanism already works correctly in parallel

**Batch Context Limitations:**
- First few events in batch may lack complete before-context
- Last few events in batch may lack complete after-context
- This is acceptable for most log analysis use cases

## Event Context Metadata

### ContextType Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextType {
    None,    // Regular event, no context
    Match,   // Event that matched filters
    Before,  // Before-context for a match
    After,   // After-context for a match
    Both,    // Overlapping before/after context
}
```

### Event Integration

```rust
pub struct Event {
    // ... existing fields ...
    pub context_type: ContextType,
}
```

## Output Formatting

### Context Markers

Following klp's visual conventions:
- `*` prefix: Actual matches (`ContextType::Match`)
- `/` prefix: Before-context lines (`ContextType::Before`)
- `\` prefix: After-context lines (`ContextType::After`)
- `|` prefix: Overlapping context lines (`ContextType::Both`)
- No prefix: Regular events (`ContextType::None`)

### Color Integration

Context markers use existing theme system:
```rust
// In THEMES configuration
"context_prefix": {
    "before": "blue",      // / prefix
    "match": "bright_magenta", // * prefix
    "after": "blue",       // \ prefix
    "overlap": "cyan",     // | prefix
}
```

### Formatter Changes

Default formatter output examples:
```
timestamp=2024-01-15T10:00:01Z level=info msg="normal message"
/ timestamp=2024-01-15T10:00:02Z level=debug msg="before context"
* timestamp=2024-01-15T10:00:03Z level=error msg="ERROR OCCURRED"
\ timestamp=2024-01-15T10:00:04Z level=info msg="after context"
\ timestamp=2024-01-15T10:00:05Z level=debug msg="more after context"
| timestamp=2024-01-15T10:00:06Z level=info msg="overlapping context"
```

## Streaming Behavior

### Latency Considerations

**After-Context Delay:**
- Streaming output delayed by `after_context` events
- Trade-off: completeness vs real-time output
- Users can choose A=0 for real-time, A>0 for completeness

**Buffer Management:**
- Look-ahead buffer size = after_context
- Memory usage: O(after_context * average_event_size)
- Reasonable for typical context sizes (1-10)

### End-of-Stream Handling

**Buffer Flush:**
- When input ends, flush remaining after_buffer contents
- Last few events may have incomplete after-context
- Mark partial context appropriately in output

## Configuration Integration

### KeloraConfig Changes

```rust
#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub before_context: usize,
    pub after_context: usize,
    pub enabled: bool,
}

impl ContextConfig {
    pub fn is_active(&self) -> bool {
        self.enabled && (self.before_context > 0 || self.after_context > 0)
    }

    pub fn required_window_size(&self) -> usize {
        if self.is_active() {
            self.before_context + self.after_context + 1
        } else {
            0
        }
    }
}
```

### CLI Integration

```rust
// In cli.rs
#[derive(Args)]
pub struct ContextArgs {
    /// Show N lines after each match
    #[arg(short = 'A', long = "after-context", value_name = "N")]
    pub after_context: Option<usize>,

    /// Show N lines before each match
    #[arg(short = 'B', long = "before-context", value_name = "N")]
    pub before_context: Option<usize>,

    /// Show N lines before and after each match
    #[arg(short = 'C', long = "context", value_name = "N")]
    pub context: Option<usize>,
}
```

## Error Handling

### Resilient Mode (Default)

- Context processing errors don't break pipeline
- Failed context calculations: continue without context
- Malformed events in context: skip gracefully
- Warn about context limitations in parallel mode

### Strict Mode

- Context processing errors cause pipeline failure
- More aggressive validation of context state
- Immediate failure on inconsistent context metadata

### Error Messages

```rust
// Example error scenarios
"Context options require active filtering (use --filter, --since, etc.)"
"Context buffer overflow: consider reducing context size or batch size"
"Warning: Parallel mode may have incomplete context at batch boundaries"
```

## Performance Considerations

### Memory Usage

**Sequential Mode:**
- Before buffer: `before_context * sizeof(Event)`
- After buffer: `after_context * sizeof(Event)`
- Total overhead: `O(context_size * event_size)`

**Parallel Mode:**
- Per-worker context buffers
- Total: `workers * context_size * event_size`
- Typically negligible for reasonable context sizes

### CPU Overhead

- Context decision: O(1) per event
- Buffer management: O(1) amortized
- Window integration: reuses existing infrastructure
- Minimal impact on overall pipeline performance

### Optimization Opportunities

- Lazy context marker formatting
- Circular buffer reuse
- Event reference sharing for context

## Testing Strategy

### Unit Tests

**Context Algorithm:**
- Before/after context marking correctness
- Overlapping context range merging
- Buffer management edge cases
- End-of-stream handling

**Configuration:**
- CLI argument parsing and validation
- Window size calculation
- Context enablement logic

### Integration Tests

**Pipeline Integration:**
- Context stage placement in pipeline
- Interaction with existing stages
- Window feature compatibility

**Output Formatting:**
- Context marker application
- Color theme integration
- Various output formats (JSON, CSV, etc.)

### Performance Tests

**Streaming Behavior:**
- Latency measurements with different context sizes
- Memory usage profiling
- Throughput impact assessment

**Parallel Processing:**
- Context behavior across batch boundaries
- Worker independence verification
- Load balancing with context buffers

### End-to-End Tests

**Real-world Scenarios:**
```bash
# Error analysis with context
kelora -f json app.log --filter 'e.level == "error"' -C 3

# Performance debugging
kelora -f json perf.log --filter 'e.response_time > 1000' -A 5

# Security analysis
kelora -f syslog auth.log --filter 'e.failed_login' -B 2 -A 1

# Streaming monitoring
tail -f app.log | kelora -f json --filter 'e.severity == "critical"' -A 2
```

## Implementation Files

### Core Implementation

1. **src/cli.rs**: Add context CLI arguments
2. **src/config.rs**: Add ContextConfig structure
3. **src/event.rs**: Add ContextType enum to Event
4. **src/pipeline/stages.rs**: Implement ContextStage
5. **src/pipeline/builders.rs**: Integrate context stage
6. **src/formatters.rs**: Add context prefix formatting

### Supporting Changes

7. **src/pipeline/mod.rs**: Update PipelineContext for context state
8. **src/main.rs**: Wire up context configuration
9. **tests/integration_tests.rs**: Add context feature tests
10. **help-screen.txt**: Document new options

## Migration and Compatibility

### Backward Compatibility

- No breaking changes to existing APIs
- Context options are opt-in
- Default behavior unchanged
- All existing features continue to work

### Configuration Migration

- No existing configuration files affected
- New context options purely additive
- Window size auto-adjusted when context enabled

### Performance Impact

- Zero overhead when context disabled
- Minimal overhead when context enabled
- No impact on non-filtering use cases
- Maintains existing parallel processing benefits

## Future Enhancements

### Advanced Context Features

- **Time-based context**: `--context-time 30s` for time-window context
- **Smart context**: Reduce duplicate context when matches overlap
- **Context filtering**: `--context-filter` to apply different filters to context lines
- **Context limits**: `--max-context-events` to cap total context output

### Output Improvements

- **Context separators**: Visual separators between context groups
- **Context numbering**: Line numbers for context organization
- **Context highlighting**: Highlight matching terms in context lines

### Performance Optimizations

- **Lazy context evaluation**: Defer context decisions until output
- **Context caching**: Cache context decisions for repeated patterns
- **Streaming optimizations**: Reduce memory usage for large context sizes

This specification provides a comprehensive blueprint for implementing context options that integrate seamlessly with Kelora's existing architecture while maintaining its performance characteristics.
