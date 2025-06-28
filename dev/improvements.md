# IMPROVEMENTS.md

## Overview

This document outlines a set of proposed architectural and usability improvements to Kelora. These changes aim to improve maintainability, performance, usability, and consistency across the tool. Each section includes rationale, benefits, and implementation guidance to support a clear development roadmap.

---

## 1. Refactor `main.rs`: Extract `run_sequential()` and `run_parallel()`

### Description

Separate the main orchestration logic into dedicated functions:

```rust
fn run_sequential(args: &Config, context: &ExecutionContext) -> Result<()>
fn run_parallel(args: &Config, context: &ExecutionContext) -> Result<()>
```

### Benefits

* Improves readability of `main()`
* Decouples CLI parsing/setup from execution
* Easier to test and profile each mode independently

### Implementation Notes

* Move Rhai engine and parser setup before dispatch
* Use an `ExecutionContext` struct for shared state (parser, engine, formatter, etc.)

---

## 2. Refine and Formalize the `Event` Data Model

### Description

Redesign the `Event` struct to establish a robust, extensible foundation for all parsed data and computed state. The new model will distinguish between raw field data, parsed/memoized metadata, and user-created fields from Rhai scripts. It also recognizes special fields like `timestamp`, `level`, and `msg`, regardless of their original names in the input.

### Implementation Plan

#### 1. Define a Rich `FieldValue` Enum

```rust
pub enum FieldValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}
```

Used for all `Event::fields` values to support type-aware scripting and filtering.

---

#### 2. Updated `Event` Struct

```rust
pub struct Event {
    pub fields: IndexMap<String, FieldValue>,
    pub raw_line: String,
    pub lineno: usize,

    // Memoized derived fields
    parsed_ts: Option<SystemTime>,
    parsed_level: Option<LogLevel>,
}
```

* `fields` stores all raw and derived data, ordered
* `raw_line` is preserved for fallback display/debugging
* `parsed_ts` and `parsed_level` cache expensive operations

---

#### 3. Recognize Special Fields

Add internal helpers like:

```rust
fn get_ts_field(&self) -> Option<&str> {
    for candidate in ["ts", "_ts", "timestamp", "at", "time"] {
        if self.fields.contains_key(candidate) {
            return Some(candidate);
        }
    }
    None
}
```

Do the same for:

* `level` → \[`level`, `lvl`, `severity`, `log_level`]
* `msg` → \[`msg`, `message`, `content`, `data`, `log`, `text`]

Allow user override via CLI:

```bash
--timestamp-field log_time --message-field content
```

---

#### 4. Memoized Accessors

```rust
pub fn get_parsed_timestamp(&mut self) -> Option<SystemTime> {
    if let Some(ts) = self.parsed_ts { return Some(ts); }
    let raw = self.get_special_field("ts")?;
    let parsed = parse_timestamp(raw)?;
    self.parsed_ts = Some(parsed);
    Some(parsed)
}
```

```rust
pub fn get_log_level(&mut self) -> LogLevel {
    if let Some(lvl) = self.parsed_level { return lvl; }
    let raw = self.get_special_field("level")?.to_lowercase();
    let parsed = match raw.as_str() {
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "info"  => LogLevel::Info,
        "warn" | "warning" => LogLevel::Warn,
        "error" => LogLevel::Error,
        "fatal" | "panic" => LogLevel::Fatal,
        _ => LogLevel::Unknown,
    };
    self.parsed_level = Some(parsed);
    parsed
}
```

---

#### 5. Optional Rhai Exposure

Expose parsed fields and typed data to Rhai via:

* Global: `event.get_ts_unix()`, `event.get_level()`
* Inject `FieldValue` variants directly if safe
* Add `to_unix_ts()` or `to_level_enum()` as built-in functions

---

### Benefits

* ✅ Performance: Expensive conversions like timestamp parsing are memoized
* ✅ Flexibility: User logs with arbitrary keys are handled intelligently
* ✅ Rhai integration: Scripting becomes type-safe and expressive
* ✅ Consistency: All formatters, filters, and processors share the same source of truth

---

### Deliverables

* `FieldValue` enum and new `Event` struct
* Memoized getters for `ts`, `level`, `msg`
* Default heuristics + user override mechanism
* Safe Rhai exposure of structured fields
* Unit tests for timestamp/level parsing, override resolution


## 3. Unify Rhai Variable Injection

### Description

Replace dual injection (`event`, `fieldname`) with a hybrid model:

* Inject valid Rhai identifiers directly
* Fallback to `event["non_ident"]` for others

### Benefits

* Consistent and predictable Rhai scope
* Reduced redundancy
* Simpler scope cloning logic

---

## 4. Internal Helpers for DefaultFormatter

### Description

Extract quoting, escaping, and coloring logic into private helpers within `default.rs` or `formatter_utils.rs`:

* `escape_if_needed()`
* `color_for_level()`
* `format_kv()`

### Benefits

* Simplifies formatter logic
* Improves testability
* Avoids future inconsistency between styles

### Note

Other formatters (logfmt, json) remain minimal and don't need to share this logic.

---

## 5. Add `--input-format auto`

### Description

Automatically detect format per line:

* JSON if starts with `{`
* Logfmt if mostly `key=value` pairs

### Benefits

* More user-friendly CLI
* Removes need for users to remember `-f logfmt` vs `-f json`

### Implementation Notes

* Add format sniffing in input dispatcher
* Add `--format auto` option with fallback to JSON

---

## 6. Summary Table Output (`--summary`)

### Description

Format the final tracked state (`tracked`) as a readable table:

```text
Metric             Value
------------------ ------
errors             1223
max_response_time  874
unique_ips         388
```

### Benefits

* Better UX for CLI users
* More presentable output for dashboards and reports

### Implementation Notes

* Add `--summary` or `--end-format summary`
* Reuse tracked state (already aggregated)

---

## 7. Add `--group-by` for Built-in Aggregation

### Description

Support grouping and aggregation directly:

```bash
kelora --group-by status --aggregate count,avg(response_time)
```

### Benefits

* Removes need to script aggregation in Rhai
* Unlocks real analytics use cases with one flag

### Implementation Notes

* Use `IndexMap<K, Vec<Event>>`
* Aggregators: `count`, `min`, `max`, `avg`, `sum`

---

## 8. Visual Improvements: Auto TTY Formatting

### Description

Use `is_terminal::stdout()` to auto-format output:

* Colorize only if stdout is a terminal
* Default to JSON if piped to another tool

### Benefits

* Smarter defaults
* Less config needed from users

---

## 9. Internal Benchmarking with Criterion.rs

### Description

Use `Criterion` for micro-benchmarks:

* `Event::parse_json()`
* `LogfmtParser::parse()`
* `RhaiEngine::execute_filter()`

### Benefits

* High-precision profiling of hot functions
* Supports statistical outlier detection

### Implementation Notes

* Keep CLI throughput benchmarks (shell-based)
* Add `benches/parser.rs`, `benches/engine.rs`

---

## 10. Add Shell Completion and Config File Support

### Description

* Generate completions: `kelora.bash`, `kelora.zsh`
* Add `.kelorarc.toml` or YAML for reusable options

### Benefits

* Improved usability for frequent users
* Supports automation and repeatability

---

## 11. Fuzz and Robustness Testing

### Description

Use `cargo fuzz` to test:

* `parse_logfmt_line()`
* `Event::from_json()`
* `RhaiEngine::filter()`

### Benefits

* Hardens Kelora against malformed input
* Prevents panics/crashes in pipelines

---

## 12. Function Registry and Discovery

### Description

Add built-in function registry:

* Document functions like `track_count`, `to_int`, `matches`
* Support `--list-functions` to list all with examples

### Benefits

* Improves discoverability
* Enables docs, LSP support, and autocomplete in future

---

## Summary

This plan outlines architectural and usability improvements for Kelora that balance robustness, performance, and user experience. Each item is modular and can be tackled incrementally. Suggested order of implementation:

1. `main.rs` refactor + `run_parallel()`/`run_sequential()`
2. `Event`/parser refactor
3. Formatter cleanup
4. UX: auto-format, summary tables, input auto-detection
5. Benchmarking + fuzzing + CI polish
