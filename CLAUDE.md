# Claude Code Context for Kelora

This file provides context for Claude Code when working on the Kelora log analysis tool.

## Project Overview

Kelora is a high-performance command-line log analysis tool written in Rust that uses the Rhai scripting engine for flexible log processing. It processes structured logs (JSON, CSV, etc.) and allows users to filter, transform, and analyze log data using embedded Rhai scripts.

## Key Commands

### Build and Test
```bash
# Build optimized release binary
cargo build --release

# Run performance tests
time ./target/release/kelora -f json <logfile> --filter "expression" --on-error skip > /dev/null

# Run lint and type checking
cargo clippy
cargo fmt --check
```

### Example Usage
```bash
# Filter high response times from JSON logs
./target/release/kelora -f json logs.jsonl --filter "response_time.sub_string(0,2).to_int() > 98"

# Count status codes and track metrics
./target/release/kelora -f json access.log --eval "track_count(tracked, status_class(status))" --end "print(tracked)"
```

## Architecture

### Core Components
- **`src/main.rs`** - CLI interface, argument parsing, main processing loop
- **`src/engine.rs`** - Rhai scripting engine wrapper with performance optimizations
- **`src/event.rs`** - Log event data structure and field management
- **`src/parsers/`** - Log format parsers (JSON, CSV, Apache, etc.)
- **`src/formatters/`** - Output formatters (JSON, text, CSV)

### Performance Design
Kelora follows a "compile once, evaluate repeatedly" model:
1. **Engine** - Built once at startup with registered functions
2. **AST compilation** - Expressions compiled to ASTs at startup
3. **Scope templates** - Reused and populated per log line
4. **Map pooling** - Reduces memory allocations for frequently used data structures

## Development Guidelines

### Performance Considerations
- Pre-compile all Rhai expressions to ASTs at startup
- Reuse scope templates instead of creating from scratch
- Pool frequently allocated data structures (Rhai Maps)
- Avoid string cloning in hot paths
- Use static methods to avoid borrow checker conflicts

### Code Style
- Follow standard Rust conventions
- Use descriptive variable names
- Add inline comments for complex logic
- Prefer `Result<T>` for error handling
- Use `anyhow` for error context

### 🔧 Rhai Optimization Alignment

Kelora is designed to leverage Rhai's built-in optimizations:

| Rhai Optimization | Kelora Usage |
|------------------|--------------|
| Pre-calculated variable offsets | Pre-declare common variables (line, event, meta, tracked) |
| AST compilation and reuse | Compile expressions once at startup |
| Cached function resolution | Use built-in and registered functions consistently |
| Contiguous variable storage | Reuse single scope, update variables in-place |

**Key Design Principle**: Trust Rhai's optimizations rather than implementing custom caching or offset management. Rhai is specifically designed for "compile once, evaluate repeatedly" scenarios.

### Testing
- Test with sample log files in `/tmp/` for development
- Use the 75k line test dataset for performance benchmarking
- Verify both correctness and performance impact of changes
- Test error handling with `--on-error` strategies

### Git Guidelines
- **NEVER use `git add .`** - Always add files explicitly by name
- Use `git add src/main.rs src/parallel.rs Cargo.toml` etc.
- Review each file individually before staging
- This prevents accidental inclusion of temporary files, editor backups, or unintended changes

## Testing Data

### Performance Test Command
```bash
time ./target/release/kelora -f json /Users/dloss/git/klp/myexamples/incident75k.jsonl \
  --filter "response_time.sub_string(0,2).to_int() > 98" --on-error skip > /dev/null
```

### Sample Test Data
- Create smaller samples: `head -n 1000 large_file.jsonl > /tmp/test_sample.jsonl`
- Use realistic filter expressions that exercise string operations
- Test various error handling strategies

## Documentation

- Main design document: `DESIGN.md`
- Rhai syntax and features documented here: https://rhai.rs/book/
- Rhai integration patterns and best practices documented
