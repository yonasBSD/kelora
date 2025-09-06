# Kelora Resiliency Design Implementation Guide

## Overview

This document provides comprehensive guidance for implementing the new resiliency design in Kelora, as outlined in `/dev/resiliency.md`. The new design represents a fundamental paradigm shift from the current error handling approach to a more resilient, context-aware system.

## Philosophy Change

**Old Approach**: Configure error strategies globally (abort/skip/quarantine) with separate reporting modes.

**New Approach**: Context-specific error handling with three distinct behaviors:
1. **Input Parsing**: Skip unparseable lines automatically
2. **Filtering**: Errors evaluate to `false` (natural filtering)  
3. **Transformations**: Atomic stages with rollback on failure

## Current Implementation Analysis

### Current Error Handling System (TO BE REMOVED)

**File Locations:**
- `src/config.rs:133-138` - `ErrorStrategy` enum (Abort/Skip/Quarantine)
- `src/config.rs:57-68` - `ErrorReportConfig` and `ErrorReportStyle` enum
- `src/cli.rs:142-158` - CLI parsing for `--on-error` and `--error-report` flags
- `src/config.rs:625-631` - Error strategy defaults logic
- `src/pipeline/stages.rs:35-45` - Current FilterStage error propagation
- `src/pipeline/stages.rs:75-80` - Current ExecStage error propagation

**Current Behavior:**
- Filters propagate errors as `ScriptResult::Error`
- Exec stages propagate errors as `ScriptResult::Error`
- Pipeline handles errors based on global strategy (abort/skip/quarantine)
- Quarantine mode exposes broken events via `meta` object
- Error reporting can be configured separately (off/summary/print)

### Key Data Structures

**Configuration Structure (src/config.rs:70-88):**
```rust
pub struct ProcessingConfig {
    pub on_error: ErrorStrategy,  // TO BE REMOVED
    pub error_report: ErrorReportConfig,  // TO BE REMOVED
    // ... other fields remain
}
```

**Pipeline Context (src/pipeline/mod.rs:118-125):**
```rust
pub struct PipelineContext {
    pub config: PipelineConfig,
    pub tracker: HashMap<String, Dynamic>,
    pub window: Vec<Event>,
    pub rhai: RhaiEngine,
    pub meta: MetaData,  // Currently used for quarantine
}
```

**Script Result Types (src/pipeline/mod.rs:96-103):**
```rust
pub enum ScriptResult {
    Skip,
    Emit(Event),
    EmitMultiple(Vec<Event>),
    Error(String),  // Current error propagation mechanism
}
```

## New Design Implementation Requirements

### 1. CLI Interface Changes

**Remove These Flags:**
- `--on-error` (src/cli.rs:142-150)
- `--error-report` (src/cli.rs:153-154)  
- `--error-report-file` (src/cli.rs:157-158)

**Add These Flags:**
- `--strict` - Exit on first error (replaces `--on-error=abort`)
- `-v/--verbose` - Show detailed error information (replaces `--error-report=print`)

**Default Behavior:**
- Summary statistics shown by default (equivalent to current `--error-report=summary`)
- No strict mode by default (resilient processing)

### 2. Context-Specific Error Handling

#### Input Parsing Context
**Location**: All parsers in `src/parsers/` (json.rs, csv.rs, syslog.rs, etc.)

**Current Behavior**: Parsers return `Result<Event>`, errors handled by pipeline strategy

**New Behavior**: 
- Skip unparseable lines automatically
- Count skipped lines for summary statistics
- Continue processing remaining lines
- Only fail in `--strict` mode

**Implementation**: 
- Update parser trait to never return errors in normal mode
- Add line counting for skipped/failed parses
- Modify pipeline to collect skip statistics

#### Filtering Context  
**Location**: `src/pipeline/stages.rs:19-46` (FilterStage)

**Current Behavior**: 
```rust
match result {
    Ok(result) => if result { ScriptResult::Emit(event) } else { ScriptResult::Skip },
    Err(e) => ScriptResult::Error(format!("Filter error: {}", e)),
}
```

**New Behavior**:
```rust
match result {
    Ok(result) => if result { ScriptResult::Emit(event) } else { ScriptResult::Skip },
    Err(_) => ScriptResult::Skip,  // Errors become false - natural filtering
}
```

**Key Changes:**
- Remove error propagation from filters completely
- Errors in filter expressions evaluate to `false` 
- No error reporting for filter failures (expected behavior)
- Only fail in `--strict` mode

#### Transformation Context
**Location**: `src/pipeline/stages.rs:60-81` (ExecStage)

**Current Behavior**: Exec errors propagate as `ScriptResult::Error`

**New Behavior**: Atomic stages with rollback
- Each `--exec` stage either completes fully or leaves event unchanged
- No partial modifications on failure
- Multiple stages can succeed/fail independently
- Progressive enhancement model

**Implementation**:
```rust
fn apply(&mut self, event: Event, ctx: &mut PipelineContext) -> ScriptResult {
    let mut event_copy = event.clone();  // Work on copy for atomic behavior
    
    let result = ctx.rhai.execute_compiled_exec(&self.compiled_exec, &mut event_copy, &mut ctx.tracker);
    
    match result {
        Ok(()) => ScriptResult::Emit(event_copy),  // Success - use modified event
        Err(_) => ScriptResult::Emit(event),       // Failure - use original event (rollback)
    }
}
```

### 3. Safety Functions

**Location**: New module `src/rhai_functions/safety.rs`

**Required Functions:**
- `get_path(event, "path.to.field", default_value)` - Safe field access with default
- `to_number(value, default)` - Type conversion with fallback
- `to_bool(value, default)` - Boolean conversion with fallback
- `has_path(event, "path.to.field")` - Existence checking
- `path_equals(event, "path.to.field", expected_value)` - Safe equality check

**Integration**: Register with Rhai engine in `src/engine.rs`

### 4. New Error Reporting System

**Replace Current System:**
- Remove `ErrorReportConfig` and `ErrorReportStyle` 
- Remove error report file output
- Remove error strategy-based reporting defaults

**New System:**
- **Default**: Summary statistics (processed events, filtered events, parse failures, stage errors)
- **Verbose (-v)**: Detailed errors with hints and line numbers
- **Strict (--strict)**: Fail fast on first error

**Implementation Locations:**
- Summary statistics: Extend existing stats system in `src/stats.rs`
- Verbose reporting: New error collection and formatting system
- Strict mode: Early termination in pipeline processing

### 5. Variable Model Simplification

**Current**: Complex meta object with quarantine support, various injected fields

**New**: Only three variables in Rhai scripts:
- `e` - The current event with all fields
- `meta` - Simplified metadata (line, line_number) 
- `window` - Array of recent events (when `--window N` used)

**Changes:**
- Remove quarantine-specific meta fields
- Simplify meta object structure
- Ensure field access returns `()` (unit type) for missing fields

## Implementation Strategy

### Phase 1: Remove Old System (High Priority)
1. Remove `ErrorStrategy` enum and `--on-error` flag
2. Remove `ErrorReportStyle` enum and `--error-report` flag  
3. Clean up quarantine logic from pipeline
4. Add `--strict` and `-v/--verbose` flags

### Phase 2: Implement New Contexts (High Priority)
5. Modify FilterStage to return `false` on errors
6. Implement atomic ExecStage with rollback
7. Update parsers for automatic skip behavior
8. Update Rhai engine integration

### Phase 3: Safety & Reporting (Medium Priority)
9. Implement safety functions (`get_path`, `to_number`, etc.)
10. Build new summary statistics system
11. Add verbose error reporting with hints
12. Implement strict mode termination

### Phase 4: Testing & Documentation (Medium/Low Priority)
13. Update integration tests for new behavior
14. Add comprehensive tests for safety functions and atomic stages
15. Update documentation and help text
16. Create migration guide for existing users

## Breaking Changes and Migration

**Major Breaking Changes:**
- `--on-error` flag removed (use `--strict` for abort behavior)
- `--error-report` flag removed (use `-v` for verbose errors)
- Filter errors no longer cause pipeline failure
- Exec stages now atomic (no partial modifications)
- Quarantine mode completely removed

**Migration Path:**
- `--on-error=abort` → `--strict`
- `--on-error=skip` → default behavior (automatic)
- `--on-error=quarantine` → no direct equivalent (use safety functions)
- `--error-report=print` → `-v/--verbose`
- `--error-report=summary` → default behavior
- `--error-report=off` → no equivalent (summary always shown)

## Testing Strategy

**Critical Test Areas:**
1. Filter error conversion to `false` behavior
2. Atomic exec stage rollback on failure
3. Progressive enhancement across multiple exec stages
4. Safety function behavior with missing fields
5. Strict mode early termination
6. Parser skip behavior and statistics
7. Verbose error reporting format and hints

**Regression Testing:**
- Ensure existing scripts continue to work with new safety model
- Validate performance with new error handling overhead
- Test edge cases around unit type `()` handling in Rhai

## File Modification Checklist

**High Priority Changes:**
- [ ] `src/config.rs` - Remove ErrorStrategy, ErrorReportConfig, related logic
- [ ] `src/cli.rs` - Remove old flags, add --strict and -v
- [ ] `src/pipeline/stages.rs` - Modify FilterStage and ExecStage behavior
- [ ] `src/engine.rs` - Update Rhai integration for new error semantics

**Medium Priority Changes:**
- [ ] `src/rhai_functions/` - Add safety.rs module with safety functions
- [ ] `src/stats.rs` - Extend for new summary statistics  
- [ ] `src/parsers/*.rs` - Update for automatic skip behavior
- [ ] `src/main.rs` - Update configuration and pipeline setup

**Low Priority Changes:**
- [ ] `tests/integration_tests.rs` - Comprehensive test updates
- [ ] Documentation files - Help text, CLAUDE.md, README updates
- [ ] Benchmarks - Ensure no performance regression

This design document should provide sufficient detail for implementing the complete resiliency redesign while maintaining the core functionality and performance of Kelora.