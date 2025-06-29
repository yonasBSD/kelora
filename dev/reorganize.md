# Code Organization Improvements

This document outlines potential improvements to Kelora's code organization following the successful pipeline refactor.

## Current State

After the trait-based pipeline refactor, Kelora has:
- **11 Rust files, 2,530 lines of code**
- **Clean trait-based architecture**
- **Zero clippy warnings**
- **All 51 tests passing**

## Large Modules Analysis

Modules over 400 lines that could benefit from reorganization:

| Module | Lines | Current Responsibility |
|--------|-------|----------------------|
| `pipeline.rs` | 686 | Pipeline struct, stages, builders, defaults |
| `formatters.rs` | 491 | JSON, text, and logfmt formatters |
| `parallel.rs` | 488 | Parallel processing, workers, tracking |

## Proposed Improvements

### 1. Split Pipeline Module ✅ COMPLETED

**Problem**: `pipeline.rs` (686 lines) handled multiple responsibilities
**Solution**: Split into focused submodules

```
src/pipeline/
├── mod.rs (250 lines)     # Main Pipeline struct and process_line logic
├── stages.rs (127 lines)  # ScriptStage implementations (FilterStage, ExecStage, etc.)
├── builders.rs (253 lines)# PipelineBuilder and CLI helper functions
└── defaults.rs (78 lines) # Default implementations (SimpleChunker, etc.)
```

**Benefits Achieved**:
- ✅ Single responsibility per file
- ✅ Easier navigation and maintenance  
- ✅ Cleaner git diffs
- ✅ 100% API compatibility maintained
- ✅ All 51 tests passing, zero clippy warnings

### 2. Centralized Error Handling ❌ NOT WORTH IT

**Problem**: Error handling scattered across modules
**Solution**: Centralized error hierarchy

**Decision**: Not worth implementing for a CLI tool like Kelora. The current `anyhow`-based error handling is simpler and more appropriate for command-line applications where users need readable error messages, not structured error hierarchies.

```rust
// src/errors.rs - NOT IMPLEMENTING
#[derive(thiserror::Error, Debug)]
pub enum KeloraError {
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("Script error: {0}")]
    Script(#[from] ScriptError),
    #[error("Pipeline error: {0}")]
    Pipeline(#[from] PipelineError),
}
```

### 3. Configuration Module (Medium Priority)

**Problem**: CLI args passed around everywhere
**Solution**: Centralized configuration

```rust
// src/config.rs
pub struct KeloraConfig {
    pub input: InputConfig,
    pub output: OutputConfig,
    pub processing: ProcessingConfig,
    pub performance: PerformanceConfig,
}
```

### 4. Additional Module Splits (Low Priority)

**Formatters**: Split into `formatters/{mod,json,text,logfmt}.rs`
**Parallel**: Split into `parallel/{mod,processor,worker,tracker}.rs`

## Modules to Keep As-Is

These modules are well-sized and focused:
- `engine.rs` (392 lines) - Rhai integration
- `event.rs` (140 lines) - Event data structure
- `parsers.rs` (370 lines) - Parser trait implementations
- `main.rs` (340 lines) - CLI orchestration

## Implementation Priority

1. ✅ **COMPLETED**: Split `pipeline.rs` module
2. ❌ **NOT IMPLEMENTING**: Centralized error handling (not worth it for CLI tools)
3. **Medium**: Add configuration module
4. **Low**: Split `formatters.rs` and `parallel.rs`

## Notes

The current codebase is actually well-organized for a project of this size. These improvements would enhance maintainability but are not urgent. The trait-based architecture achieved the main goals.

---

*Status*: Item #1 (Pipeline module split) completed successfully. Remaining items are optional future improvements.