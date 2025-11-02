# Array Field Extraction API (`pluck`)

## Overview

Replace window-specific extraction functions with generic array methods that work on any array of maps.

## Current State (Before)

Three mechanisms for accessing window data:

```rhai
// 1. Raw window variable
let level = window[0].level;

// 2. Extract strings (converts everything to strings)
let levels = window_values(window, "level");  // ["ERROR", "WARN"]

// 3. Extract numbers (parses as f64)
let times = window_numbers(window, "response_time");  // [0.15, 0.23]
```

**Problems:**
- Confusing: 3 different mechanisms
- Window-specific: Can't use on user arrays or nested JSON
- Verbose: `window_values(window, "field")`
- Name ambiguity: "values" doesn't indicate string conversion

## New Design (After)

Two generic array methods that work on ANY array of maps:

```rhai
// 1. Extract field values (preserves original types)
let data = [#{x: 42}, #{x: "text"}, #{x: 3.14}];
data.pluck("x")                // → [42, "text", 3.14] (types preserved)

// 2. Extract field values AS numbers (parse/convert to f64)
let logs = [#{status: "200"}, #{status: "404"}, #{status: "500"}];
logs.pluck_as_nums("status")   // → [200.0, 404.0, 500.0] (strings parsed to numbers)
```

## Function Specifications

### `pluck(array, field_name) -> Array`

**Purpose:** Extract field from array of maps, preserving original types.

**Behavior:**
- Extracts specified field from each map in the array
- Returns values **as-is** (no type conversion)
- Skips elements that:
  - Are not maps
  - Don't have the specified field
  - Have unit `()` values (null/missing)
- **Result array may be shorter** than input if elements are skipped

**Type Preservation:**
- Integer → Integer (i64)
- Float → Float (f64)
- String → String
- Boolean → Boolean
- Mixed types in result array are OK (Rhai Dynamic handles this)

**Examples:**
```rhai
// Window usage
let levels = window.pluck("level");  // ["ERROR", "WARN", "INFO"]
let codes = window.pluck("status");   // [200, 404, 500] (ints preserved)

// User array usage
let rows = [#{name: "alice", age: 30}, #{name: "bob", age: 25}];
let names = rows.pluck("name");       // ["alice", "bob"]
let ages = rows.pluck("age");         // [30, 25] (ints)

// Skipping missing fields
let data = [#{x: 1}, #{y: 2}, #{x: 3}];
let vals = data.pluck("x");           // [1, 3] (skipped middle element)
```

---

### `pluck_as_nums(array, field_name) -> Array<f64>`

**Purpose:** Extract field from array of maps, parsing/converting to f64 for math operations.

**Primary Use Case:** Parse string numbers from JSON/logs (e.g., `"200"` → `200.0`)

**Behavior:**
- Extracts specified field from each map in the array
- Parses/converts all values to f64
- Skips elements that:
  - Are not maps
  - Don't have the specified field
  - Have unit `()` values (null/missing)
  - Cannot be converted to numbers
- **Result array may be shorter** than input if elements are skipped

**Type Conversion Rules:**
- String → f64 (parse, e.g., `"123"` → `123.0`) **← PRIMARY USE CASE**
- Integer → f64 (`42` → `42.0`)
- Float → f64 (already correct type)
- Boolean → f64 (`true` → `1.0`, `false` → `0.0`)
- Non-numeric strings skipped (`"text"` → not included)

**Examples:**
```rhai
// Parse string numbers from JSON logs (COMMON CASE)
let data = [#{status: "200"}, #{status: "404"}, #{status: "500"}];
let codes = data.pluck_as_nums("status");  // [200.0, 404.0, 500.0]

// Extract for math operations
let times = window.pluck_as_nums("response_time");
let avg = times.reduce(|sum, v| sum + v, 0.0) / times.len();
let p95 = times.percentile(95);

// Handles mixed types (string numbers + actual numbers)
let data = [#{v: "42"}, #{v: 3.14}, #{v: "100"}];
let nums = data.pluck_as_nums("v");  // [42.0, 3.14, 100.0]

// Skip non-numeric
let mixed = [#{v: "42"}, #{v: "text"}, #{v: 3.14}];
let nums = mixed.pluck_as_nums("v");  // [42.0, 3.14] ("text" skipped)
```

## Use Case Guidelines

**Use `pluck()` when:**
- Extracting string fields (log levels, messages, usernames)
- Preserving original types matters
- Working with mixed-type data
- Output will be filtered/processed further

**Use `pluck_as_nums()` when:**
- Parsing string numbers from JSON/logs (`"200"` → `200.0`)
- Performing math operations (sum, average, percentile)
- Need guaranteed numeric array for calculations
- Fields might be strings, ints, or floats - need unified f64

**Use raw array access when:**
- Need multiple fields from same event
- Complex filtering logic across fields
- Accessing event metadata (line numbers, filenames)

```rhai
// Multiple fields → use raw access
if window[0].level == "ERROR" && window[0].code >= 500 {
    e.critical = true;
}
```

## Implementation Notes

### Location
- Implement in `src/rhai_functions/arrays.rs` (NOT window.rs)
- These are generic array utilities, not window-specific

### Registration
```rust
// In src/rhai_functions/arrays.rs
pub fn register_functions(engine: &mut Engine) {
    // ... existing array functions ...
    engine.register_fn("pluck", pluck);
    engine.register_fn("pluck_as_nums", pluck_as_nums);
}
```

Rhai automatically makes these available as methods:
- `pluck(array, "field")` → `array.pluck("field")`
- `pluck_as_nums(array, "field")` → `array.pluck_as_nums("field")`

### Migration Path

**Remove from `window.rs`:**
- `window_values()` → replaced by `pluck()`
- `window_numbers()` → replaced by `pluck_as_nums()`

**Keep in `window.rs`:**
- `percentile()` → still useful as array utility

**Update documentation:**
- `src/rhai_functions/docs.rs`
- `--help-rhai` windowing section
- All examples in `examples/` and `docs/`

**Breaking changes acceptable** per project policy.

## Testing

**Test coverage needed:**
1. Mixed-type arrays (verified working ✓)
2. Type preservation in `pluck()`
3. Type conversion/parsing in `pluck_as_nums()`
4. String number parsing (primary use case)
5. Skipping non-maps, missing fields, non-numeric values
6. Empty arrays
7. Usage on window, user arrays, nested JSON

**Confirmed via testing:**
- Rhai arrays support mixed types ✓
- `pluck()` preserves types correctly ✓
- `pluck_as_nums()` converts to f64 and skips non-numeric ✓
- Math operations work on results ✓
