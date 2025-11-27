# Kelora Performance Optimization Plan

## Overview

This document outlines behavior-preserving performance optimizations for kelora's log processing pipeline. All changes maintain identical output and test compatibility while targeting identified hot paths. Expected improvement: **15-30% throughput increase** for typical workloads.

## Hot Paths Identified

1. **JSON parsing** (serde_json deserialization) - Primary bottleneck for JSON workloads
2. **Rhai script execution** - Dynamic type conversions and field access per event
3. **Dynamic type conversions** - Between JSON and Rhai types (recursive for nested data)
4. **Parallel processing overhead** - Batch distribution and state merging
5. **String operations** - Line trimming, clone operations, allocations
6. **Regex operations** - Pattern compilation for dynamic patterns
7. **Output formatting** - Dynamic-to-JSON conversions and string escaping

## Recommended Optimizations

### Phase 1: High-Impact String Operations (8-12% improvement)

#### 1.1 Line Trimming Optimization ⭐ CRITICAL
**Impact:** 2-4% | **Risk:** Very Low | **Effort:** 1 day

**Problem:** Every parsed line calls `.trim_end_matches('\n').trim_end_matches('\r')` - two passes over string tail.

**Files to modify:**
- `src/parsers/json.rs:25`
- `src/parsers/logfmt.rs:136`
- `src/parsers/csv.rs:254`
- `src/parsers/syslog.rs:226`
- `src/parsers/combined.rs:296`
- `src/parsers/cef.rs:161`
- `src/parsers/line.rs:16`
- `src/rhai_functions/columns.rs:333, 356`

**Change:**
```rust
// BEFORE
let line = line.trim_end_matches('\n').trim_end_matches('\r');

// AFTER
let line = line.trim_end_matches(['\n', '\r']);
```

**Testing:**
- Run parser tests: `cargo test parsers::`
- Run benchmarks: `just bench`
- Verify output byte-for-byte identical with CRLF and LF line endings

---

### Phase 2: Memory Pre-allocation (4-8% improvement)

#### 2.1 Pre-allocate Map Capacity ⭐ HIGH PRIORITY
**Impact:** 3-5% | **Risk:** Very Low | **Effort:** 2 days

**Problem:** Creating Rhai Maps without capacity hints causes multiple reallocations per event.

**Files to modify:**
- `src/engine.rs:2396` - Event map conversion (hot path)
- `src/engine.rs:2403` - Metadata map
- `src/engine.rs:2456` - Window event maps
- `src/rhai_functions/strings.rs:52` - event_to_map helper
- `src/rhai_functions/span.rs:82` - span event_to_map

**Change example (engine.rs:2396):**
```rust
// BEFORE
let mut event_map = rhai::Map::new();
for (k, v) in &event.fields {
    event_map.insert(k.clone().into(), v.clone());
}

// AFTER
let mut event_map = rhai::Map::with_capacity(event.fields.len());
for (k, v) in &event.fields {
    event_map.insert(k.clone().into(), v.clone());
}
```

**Metadata map optimization:**
```rust
// Metadata typically has 2-7 fields
let mut meta_map = rhai::Map::with_capacity(8);
```

**Testing:**
- All tests must pass
- Benchmark parallel mode (hot path)
- Memory profiler to verify reduced allocations

---

#### 2.2 Optimize Window Event Conversion
**Impact:** 1-3% | **Risk:** Low | **Effort:** 1 day

**Problem:** Window events converted to maps without capacity hints.

**File:** `src/engine.rs:2449-2489`

**Change:**
```rust
let window_array: rhai::Array = window
    .iter()
    .map(|event| {
        let mut event_map = rhai::Map::with_capacity(event.fields.len() + 8);
        // ... rest unchanged
    })
    .collect();
```

**Testing:**
- Window-specific tests
- Benchmark with `--window` flag
- Verify multi-event window correctness

---

### Phase 3: Parser-Specific Optimizations (2-4% improvement)

#### 3.1 JSON Numeric Conversion Optimization
**Impact:** 1-2% | **Risk:** Low | **Effort:** 1 day

**Problem:** Numeric conversion tries i64, u64, f64 sequentially with unwrapping.

**File:** `src/event.rs:223-236` (json_to_dynamic function)

**Change:**
```rust
// BEFORE
serde_json::Value::Number(n) => {
    if let Some(i) = n.as_i64() {
        Dynamic::from(i)
    } else if let Some(u) = n.as_u64() {
        Dynamic::from(u)
    } else if let Some(f) = n.as_f64() {
        Dynamic::from(f)
    } else {
        Dynamic::from(n.to_string())
    }
}

// AFTER - check type characteristics first
serde_json::Value::Number(n) => {
    if n.is_i64() {
        Dynamic::from(n.as_i64().unwrap())
    } else if n.is_u64() {
        Dynamic::from(n.as_u64().unwrap())
    } else {
        Dynamic::from(n.as_f64().unwrap())
    }
}
```

**Testing:**
- Test positive/negative integers, large u64, floats
- Ensure all numeric types preserved correctly
- Benchmark with number-heavy JSON

---

### Phase 4: Collection Pre-allocation (1-3% improvement)

#### 4.1 Pre-allocate Known-Size Collections
**Impact:** 1-2% | **Risk:** Very Low | **Effort:** 1 day

**Files with opportunities:**
- `src/rhai_functions/absorb.rs:227` - data_map for JSON absorption
- `src/formatters.rs:150, 194` - JSON output objects

**Pattern:**
```rust
// When source size is known
let mut map = Map::with_capacity(source.len());
```

**Testing:** Existing tests, spot-check affected features

---

### Phase 5: Speculative Optimizations (OPTIONAL - Profile First)

#### 5.1 Scope Creation Optimization (Speculative)
**Impact:** 1-2% (uncertain) | **Risk:** Medium | **Effort:** 2-3 days

**Problem:** `scope_template.clone()` called per event (engine.rs:2390).

**Constraint:** Scope must be unique per event (Rhai requirement).

**Approach:** Cache empty map instances to reduce Map::new() overhead:
```rust
struct ScopeCache {
    empty_e_map: rhai::Map,
    empty_meta_map: rhai::Map,
    empty_conf_map: rhai::Map,
}
// Clone from cached instances instead of creating fresh
```

**Important:** Profile `scope.clone()` cost BEFORE implementing. If negligible, skip.

---

#### 5.2 Lazy Metadata Construction (Speculative)
**Impact:** Low (uncertain) | **Risk:** Medium | **Effort:** 2 days

**Problem:** Meta map always built even if filter doesn't access `meta`.

**Approach:** Use AST analysis to detect `meta` access, only construct if needed.

**Concern:** Complex to implement, benefits unclear (metadata commonly used).

**Recommendation:** SKIP unless profiling shows metadata construction is a bottleneck.

---

## Implementation Sequence

### Week 1: High-Confidence Quick Wins
1. **Phase 1.1** - Line trimming (1 day)
2. **Phase 2.1** - Map pre-allocation (2 days)
3. Run benchmarks, validate (1 day)

### Week 2: Parser & Collection Improvements
1. **Phase 3.1** - JSON numeric path (1 day)
2. **Phase 2.2** - Window conversion (1 day)
3. **Phase 4.1** - Collection pre-allocation (1 day)
4. Final validation (1 day)

### Week 3: Optional/Speculative (If Pursuing)
1. Profile scope and metadata construction
2. Implement Phase 5.1/5.2 only if profiling justifies
3. Final benchmarking and validation

---

## Testing & Validation Strategy

### Per-Phase Checklist
1. ✅ `cargo fmt --all`
2. ✅ `cargo clippy --all-targets --all-features -- -D warnings`
3. ✅ `cargo test` - ALL tests must pass
4. ✅ `just bench` - compare to baseline
5. ✅ Spot-test with real log files
6. ✅ Output verification (byte-for-byte where applicable)

### Establish Baseline
```bash
# Before starting
just bench > baseline_results.txt
```

### After Each Phase
```bash
just bench > phase_N_results.txt
# Compare improvements
```

### Specific Test Commands
```bash
# Quick format-specific test
time ./target/release/kelora -f json benchmarks/bench_100k.jsonl \
  --filter "e.level == 'ERROR'" > /dev/null

# Memory profiling
cargo build --release
valgrind --tool=massif ./target/release/kelora -f json \
  benchmarks/bench_100k.jsonl > /dev/null
```

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking behavior | Comprehensive test suite, output validation |
| Performance regression | Benchmark each phase, revert if slower |
| Parallel mode issues | Extensive parallel tests in suite |
| Memory regression | Profile before/after with valgrind/massif |
| Test failures | Immediate rollback of changes |

---

## Expected Results

| Phase | Optimization | Expected Improvement | Risk Level |
|-------|--------------|---------------------|------------|
| 1.1 | Line trimming | 2-4% | Very Low ⭐ |
| 2.1 | Map pre-alloc (events) | 3-5% | Very Low ⭐ |
| 2.2 | Map pre-alloc (windows) | 1-3% | Low |
| 3.1 | JSON numerics | 1-2% | Low |
| 4.1 | Collection alloc | 1-2% | Very Low |
| 5.1 | Scope caching (optional) | 1-2% | Medium |
| **TOTAL** | **Cumulative** | **15-30%** | **Low-Medium** |

**Note:** Improvements are multiplicative. Conservative estimate: 15%, optimistic: 30%.

---

## Critical Files

These files are central to the hot paths and will need modification:

1. **`src/parsers/json.rs`** - JSON parsing entry point, line trimming (L25)
2. **`src/event.rs`** - Core json_to_dynamic conversion (L220-257), used throughout
3. **`src/engine.rs`** - Event-to-map hot paths (L2390-2489), executed per-event
4. **`src/parsers/logfmt.rs`** - Second most common format, line trimming (L136)
5. **`src/parallel.rs`** - Batch processing context (readonly, for understanding)

---

## Success Criteria

✅ All existing tests pass
✅ No clippy warnings
✅ Benchmark improvements match estimates (±3%)
✅ Output identical for representative workloads
✅ No memory regressions (verify with profiler)
✅ Code remains readable and maintainable

---

## Key Insights from Exploration

### What's Already Optimized
- ✅ AST compilation is properly cached (scripts compiled once)
- ✅ Regex caching with thread-local LRU (1000 entries)
- ✅ JSON parser pre-allocates HashMap with capacity
- ✅ Scope template reuse pattern (clone + modify)

### Biggest Bottlenecks Identified
- ❌ Per-event Map creation without capacity hints
- ❌ Line trimming with two passes
- ❌ Window event conversion multiplies Map allocation overhead
- ❌ Field value cloning between Rust and Rhai types

### Why These Optimizations Are Safe
- No change to output format or values
- No change to script semantics
- Pre-allocation doesn't change behavior, only performance
- Single-pass string operations produce identical results
- All changes are internal implementation details

---

## Notes

- **No backwards compatibility requirement:** Breaking API changes acceptable (though not needed here)
- **Conservative approach selected:** Capacity hints only, no aggressive refactoring
- **Speculative items clearly marked:** Profile before implementing Phase 5
- **Incremental validation:** Test after each phase, easy rollback
- **Honest benchmarking:** Use existing `just bench` suite with external tool comparisons
