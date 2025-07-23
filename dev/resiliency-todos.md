# Kelora Resiliency Implementation Todo List

This document tracks all tasks required to implement the new resiliency design for Kelora. Tasks are organized by priority and implementation phase.

## High Priority Tasks (Phase 1: Core Architecture Changes)

### ✅ 1. Create comprehensive design documentation in dev/resiliency-design.md
**Status**: Completed  
**Description**: Document the complete implementation plan, current system analysis, and new design requirements.

### 2. Remove ErrorStrategy enum from src/config.rs and src/cli.rs
**Status**: Pending  
**Priority**: High  
**Description**: Remove the ErrorStrategy enum (Abort/Skip/Quarantine) from both config and CLI modules. This is a fundamental breaking change.
**Files**: `src/config.rs:133-138`, `src/cli.rs:40-44`

### 3. Remove --on-error flag and related CLI parsing logic
**Status**: Pending  
**Priority**: High  
**Description**: Remove the --on-error CLI flag and all associated parsing logic. Update help text.
**Files**: `src/cli.rs:142-150`

### 4. Remove --error-report flag and ErrorReportStyle enum
**Status**: Pending  
**Priority**: High  
**Description**: Remove ErrorReportStyle enum and --error-report flag. Clean up associated configuration logic.
**Files**: `src/config.rs:57-68`, `src/cli.rs:153-154`

### 5. Add --strict flag to CLI for fail-fast behavior
**Status**: Pending  
**Priority**: High  
**Description**: Add new --strict flag that replaces --on-error=abort functionality. Should cause immediate exit on any error.
**Files**: `src/cli.rs`, `src/config.rs`

### 6. Add -v/--verbose flag for detailed error information
**Status**: Pending  
**Priority**: High  
**Description**: Add verbose flag for detailed error reporting with hints and suggestions. Replaces --error-report=print.
**Files**: `src/cli.rs`, `src/config.rs`

### 7. Modify FilterStage::apply() to return false on errors instead of propagating
**Status**: Pending  
**Priority**: High  
**Description**: Change filter error handling so errors evaluate to false instead of propagating as ScriptResult::Error.
**Files**: `src/pipeline/stages.rs:35-45`

### 8. Implement atomic ExecStage with rollback - preserve original event on failure
**Status**: Pending  
**Priority**: High  
**Description**: Redesign ExecStage to work on event copy and only commit changes on success. On failure, return original unchanged event.
**Files**: `src/pipeline/stages.rs:60-81`

### 9. Update Rhai engine integration to support new error semantics
**Status**: Pending  
**Priority**: High  
**Description**: Modify Rhai engine methods to support the new error handling approach for filters and exec stages.
**Files**: `src/engine.rs`

## Medium Priority Tasks (Phase 2-3: New Functionality)

### 10. Remove quarantine logic from pipeline processing
**Status**: Pending  
**Priority**: Medium  
**Description**: Clean up all quarantine-related code from the pipeline processing logic.
**Files**: Various pipeline files

### 11. Update input parsers to skip unparseable lines and count them
**Status**: Pending  
**Priority**: Medium  
**Description**: Modify all parsers to automatically skip unparseable lines and track counts for statistics.
**Files**: `src/parsers/*.rs`

### 12. Implement summary statistics tracking for skipped lines
**Status**: Pending  
**Priority**: Medium  
**Description**: Extend the statistics system to track and report skipped lines and stage errors.
**Files**: `src/stats.rs`

### 13. Add get_path() safety function to Rhai functions
**Status**: Pending  
**Priority**: Medium  
**Description**: Implement `get_path(event, "path.to.field", default_value)` for safe field access with defaults.
**Files**: `src/rhai_functions/safety.rs` (new file)

### 14. Add to_number() safety function to Rhai functions
**Status**: Pending  
**Priority**: Medium  
**Description**: Implement `to_number(value, default)` for safe type conversion with fallback.
**Files**: `src/rhai_functions/safety.rs`

### 15. Add to_bool() safety function to Rhai functions
**Status**: Pending  
**Priority**: Medium  
**Description**: Implement `to_bool(value, default)` for safe boolean conversion with fallback.
**Files**: `src/rhai_functions/safety.rs`

### 16. Add has_path() safety function to Rhai functions
**Status**: Pending  
**Priority**: Medium  
**Description**: Implement `has_path(event, "path.to.field")` for existence checking.
**Files**: `src/rhai_functions/safety.rs`

### 17. Add path_equals() safety function to Rhai functions
**Status**: Pending  
**Priority**: Medium  
**Description**: Implement `path_equals(event, "path.to.field", expected_value)` for safe equality checking.
**Files**: `src/rhai_functions/safety.rs`

### 18. Implement new error reporting system with summary statistics
**Status**: Pending  
**Priority**: Medium  
**Description**: Build new default error reporting that shows summary statistics (processed events, filtered events, parse failures, stage errors).
**Files**: New error reporting modules

### 19. Add verbose mode error reporting with hints and suggestions
**Status**: Pending  
**Priority**: Medium  
**Description**: Implement detailed error reporting for -v/--verbose mode with helpful hints and line numbers.
**Files**: Error reporting modules

### 21. Update integration tests for new error handling behavior
**Status**: Pending  
**Priority**: Medium  
**Description**: Modify existing integration tests to work with new error handling semantics.
**Files**: `tests/integration_tests.rs`

### 22. Add tests for atomic exec stages with rollback behavior
**Status**: Pending  
**Priority**: Medium  
**Description**: Create comprehensive tests for the new atomic exec stage behavior.
**Files**: Test files

### 23. Add tests for safety functions (get_path, to_number, etc.)
**Status**: Pending  
**Priority**: Medium  
**Description**: Test all new safety functions with various edge cases and data types.
**Files**: Test files

### 24. Test filter error-to-false conversion behavior
**Status**: Pending  
**Priority**: Medium  
**Description**: Verify that filter errors properly convert to false instead of causing pipeline failures.
**Files**: Test files

### 25. Validate strict mode behavior with comprehensive tests
**Status**: Pending  
**Priority**: Medium  
**Description**: Test that --strict mode properly terminates on first error across all contexts.
**Files**: Test files

## Low Priority Tasks (Phase 4: Polish & Documentation)

### 20. Remove error report file output functionality
**Status**: Pending  
**Priority**: Low  
**Description**: Clean up --error-report-file functionality since it's being removed.
**Files**: `src/cli.rs:157-158`, related code

### 26. Update help text and CLI documentation for new flags
**Status**: Pending  
**Priority**: Low  
**Description**: Update all help text, CLI documentation, and examples for the new flag structure.
**Files**: CLI help, documentation files

### 27. Update CLAUDE.md with new error handling approach
**Status**: Pending  
**Priority**: Low  
**Description**: Rewrite the error handling section in CLAUDE.md to reflect the new resiliency approach.
**Files**: `CLAUDE.md`

### 28. Create migration guide for users transitioning from old error handling
**Status**: Pending  
**Priority**: Low  
**Description**: Document how users should migrate from old --on-error/--error-report flags to new system.
**Files**: Migration documentation

### 29. Clean up unused error handling code and imports
**Status**: Pending  
**Priority**: Low  
**Description**: Remove any remaining unused code, imports, and references to the old error handling system.
**Files**: Various

### 30. Update benchmarks to ensure no performance regression
**Status**: Pending  
**Priority**: Low  
**Description**: Run performance benchmarks to validate that the new error handling doesn't introduce regressions.
**Files**: Benchmark files

## Implementation Strategy

### Phase 1: Remove Old System (Tasks 1-9)
Focus on removing the existing error handling infrastructure and implementing the core new CLI interface. This phase involves significant breaking changes.

### Phase 2: New Error Contexts (Tasks 10-12)  
Implement the three context-specific error handling behaviors: input parsing skip, filter error-to-false, and atomic exec stages.

### Phase 3: Safety & Reporting (Tasks 13-19)
Add the safety functions and new error reporting system that makes the resilient approach user-friendly.

### Phase 4: Testing & Documentation (Tasks 20-30)
Comprehensive testing, documentation updates, and polish to ensure the system is production-ready.

## Notes

- Tasks should generally be completed in priority order, but some can be done in parallel
- High priority tasks are blocking for the core functionality
- Each task should include appropriate unit and integration tests
- Consider creating feature branches for major changes to enable easier review
- Performance testing should be done throughout, not just at the end

## Breaking Changes Summary

This implementation introduces significant breaking changes:
- `--on-error` flag removed (use `--strict` for abort behavior)
- `--error-report` flag removed (use `-v` for verbose errors) 
- Filter errors no longer cause pipeline failure
- Exec stages now atomic (no partial modifications)
- Quarantine mode completely removed

Users will need to migrate their scripts and command-line usage.