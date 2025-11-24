# Field Access Warning System - TODOs

## Findings from Testing (2025-11-24)

### Finding 1: Filter mode limitation

**Issue**: Regular filter mode (without `-C`/`-A`/`-B`) does NOT trigger warnings. It only generates errors.

**Root Cause**: The warning system is only implemented in:
- Context-aware filter stage (src/pipeline/stages.rs:121-150)
- Exec stage (src/pipeline/stages.rs:442-471)

Regular filter mode (src/pipeline/stages.rs:287-322) only calls `track_error()`, not `track_warning()` for unit type errors.

**Impact**: Users running simple filters like `kelora -j --filter 'e.statu > 400'` will get errors instead of helpful warnings with suggestions.

**Recommended Solution**: ✅ **Implement warning tracking in non-context filter path**

This is straightforward and should be prioritized:

1. **Code Location**: src/pipeline/stages.rs:298-310 (non-context filter error handling)
2. **Implementation**: Add warning tracking logic similar to context-aware filter (lines 121-150):
   ```rust
   Err(e) => {
       let error_msg = format!("{}", e);

       // NEW: Detect unit type operations and track as warnings
       if !ctx.config.no_warnings
           && crate::rhai_functions::tracking::is_unit_type_error(&error_msg)
       {
           let field_name = crate::rhai_functions::tracking::extract_field_from_script(
               self.compiled_filter.source(),
           ).unwrap_or_else(|| "unknown".to_string());
           let operation = crate::rhai_functions::tracking::extract_operation(&error_msg);

           let mut available_fields: std::collections::BTreeSet<String> =
               event.fields.keys().cloned().collect();
           if let Some(dynamic_keys) = ctx.internal_tracker.get("__kelora_stats_discovered_keys") {
               // ... populate available_fields
           }

           crate::rhai_functions::tracking::track_warning(
               &field_name, operation.as_deref(),
               ctx.meta.line_num.unwrap_or(0), &available_fields
           );
       }

       // Handle error appropriately (track_error if not unit-type warning)
       if !ctx.config.strict
           && crate::rhai_functions::tracking::is_unit_type_error(&error_msg)
       {
           // Treat as warning only
       } else {
           crate::rhai_functions::tracking::track_error(...);
       }

       // Return error or Skip based on strict mode
       ...
   }
   ```

3. **Expected Outcome**: Users will see helpful suggestions for typos in regular filter mode
4. **Testing**: Run `echo '{"status_code":500}' | kelora -j --filter 'e.statu + 1 > 0'` → should show warning with suggestion

---

### Finding 3: Comparison operations don't trigger warnings

**Issue**: Simple comparisons like `e.missing > 400` don't generate errors, so warnings aren't triggered.

**Investigation Results** (confirmed):
- `e.missing > 400` → evaluates to `false` (no error)
- `e.missing == 400` → evaluates to `false` (no error)
- `e.missing + "text"` → evaluates to `"text"` (no error)
- `e.missing + 1` → **errors** with "Function not found: + ((), i64)" ✓

**Root Cause**: Rhai's unit type `()` is permissive:
- Comparisons with any type return `false` silently
- String concatenation treats `()` as empty string
- Only arithmetic operations (+ - * / with numbers) generate errors

**Impact**: Typos in filter expressions using comparisons silently evaluate to false, skipping events without warning.

**Example Problem**:
```bash
# User typo: 'statu' instead of 'status'
echo '{"status":500}' | kelora -j --filter 'e.statu > 400'
# Result: No output, no warning, no error - silently skips event
```

**Recommended Solutions** (in priority order):

**Option 1**: ❌ **Custom property getter with access tracking** (NOT FEASIBLE)

**Investigation Results**: After researching Rhai's API, this approach has significant blockers:

- **Cannot override Map indexers**: Rhai [explicitly disallows](https://rhai.rs/book/rust/indexers.html) overriding built-in Map indexers for performance reasons
- **on_var only tracks variables**: The [on_var callback](https://rhai.rs/book/engine/var.html) only intercepts top-level variable resolution (e.g., `e`), not property access (e.g., `e.field_name`)
- **Would require custom wrapper type**: Need to replace `rhai::Map` with a custom Rust struct that wraps event fields, then register property getters

**Why this is problematic**:
- Major refactoring: Every place that creates/uses `e` would need changes
- Breaking change: Would affect all existing scripts
- Performance overhead: Custom property getter called for EVERY field access
- Complexity: Need to maintain wrapper type that mimics Map behavior

**Verdict**: ❌ Not recommended unless we're willing to make major architectural changes

---

**Option 1a**: 🔍 **AST analysis with `internals` feature** (ALTERNATIVE APPROACH)

**How it works**:
1. Enable Rhai's [`internals` feature](https://rhai.rs/book/engine/ast.html) in Cargo.toml
2. After compiling filter/exec, walk the AST using `AST::walk()`
3. Find all property access nodes matching `e.field_name` patterns
4. Store list of accessed field names
5. At runtime, compare accessed fields against available event fields
6. Warn about fields that were accessed but don't exist

**Example implementation**:
```rust
// Enable in Cargo.toml:
// rhai = { version = "1.23", features = ["sync", "debugging", "internals"] }

use rhai::ast::{ASTNode, Expr};

fn extract_field_accesses(ast: &AST) -> HashSet<String> {
    let mut fields = HashSet::new();
    ast.walk(&mut |path| {
        if let Some(ASTNode::Expr(Expr::Property(box prop, ..))) = path.first() {
            // Extract field name from property access
            if prop.is_variable_access("e") {
                fields.insert(prop.get_property_name().to_string());
            }
        }
        true // Continue walking
    });
    fields
}
```

**Pros**:
- ✅ Catches ALL field accesses, including comparisons
- ✅ Zero runtime overhead (analysis happens once at compile time)
- ✅ No script execution needed to detect issues
- ✅ Non-invasive: Doesn't change existing event handling

**Cons**:
- ⚠️ Requires `internals` feature (potentially unstable API)
- ⚠️ AST structure may change between Rhai versions
- ⚠️ Need to handle dynamic field access patterns (e.g., `e[var_name]`)
- ⚠️ More complex implementation than simple error detection

**Performance implications**:
- One-time AST walk during script compilation (negligible cost)
- Memory: Need to store set of accessed field names per script
- Runtime: Simple HashSet lookup when event is processed

**Risk assessment**:
- **API stability**: The `internals` feature is documented but marked as internal-facing
- **Maintenance burden**: May need updates when Rhai upgrades
- **Complexity**: Medium - need to understand AST node types

**Verdict**: 🟡 Feasible but requires accepting Rhai internals API instability. Best paired with comprehensive tests to catch Rhai upgrade issues early.

**Recommended implementation strategy**:
1. Start with a prototype to validate AST walking approach
2. Add integration tests that verify AST analysis across Rhai versions
3. Document the Rhai version dependency clearly
4. Consider feature flag to disable if Rhai upgrade breaks it

---

**Option 2**: 📝 **Documentation + best practices** (IMMEDIATE WIN)
- Document that only arithmetic operations reliably trigger warnings
- Recommend using exec mode for development/debugging
- Provide examples of patterns that catch typos:
  ```rhai
  // Good: Will catch typo in 'statu'
  if e.has("status") && e.status > 400 { ... }

  // Bad: Typo silently returns false
  if e.statu > 400 { ... }
  ```
- Add to `--help-examples` and examples/README.md

**Option 3**: 🔧 **Strict field access mode** (future consideration)
- Add `--strict-fields` flag
- Make any access to non-existent field an error
- Would catch all typos but changes behavior significantly
- Could be opt-in for development workflows

---

## Summary and Recommendations

### Immediate Actions (Do Now)
1. ⚠️ **Fix Finding 1**: Add warning tracking to non-context filter mode
   - **Effort**: Low (30 lines of code)
   - **Impact**: Medium (partial improvement, see concerns below)
   - **Risk**: Low (behavior change from error to warning)

   **IMPORTANT CONCERN**: This fix has a subtle UX problem that needs evaluation:

   **Current behavior** (regular filter mode):
   ```bash
   echo '{"status_code":500}' | kelora -j --filter 'e.statu + 1 > 0'
   # → ERROR: "Function not found: + ((), i64)"
   # → Exit code 1

   echo '{"status_code":500}' | kelora -j --filter 'e.statu > 400'
   # → Silent skip, no output, no warning, no error
   # → Exit code 0
   ```

   **After fix** (regular filter mode with warnings):
   ```bash
   echo '{"status_code":500}' | kelora -j --filter 'e.statu + 1 > 0'
   # → WARNING: 'statu' missing (suggestion: status_code)
   # → Event skipped, continues processing
   # → Exit code 0

   echo '{"status_code":500}' | kelora -j --filter 'e.statu > 400'
   # → Still silent skip, no output, NO WARNING
   # → Exit code 0
   ```

   **The problem**:
   - Users get warnings for **arithmetic** typos (`e.field + 1`)
   - Users get **no warnings** for **comparison** typos (`e.field > 400`)
   - This inconsistency could be confusing: "Why did it warn about my typo here but not there?"
   - Users might assume the warning system is comprehensive when it's not

   **Arguments FOR the fix**:
   - ✅ Some warnings are better than none
   - ✅ Makes behavior consistent with exec mode and context-aware filters
   - ✅ Converts hard errors into graceful degradation (warnings)
   - ✅ Users who write `e.field + 1` patterns benefit immediately

   **Arguments AGAINST the fix**:
   - ❌ Creates false sense of security (warnings seem comprehensive but aren't)
   - ❌ Most filters use comparisons, not arithmetic (so many typos still undetected)
   - ❌ The inconsistency is subtle and hard to explain in documentation
   - ❌ Might be better to wait until we can catch ALL field access (via AST analysis)

   **Question for decision**: Should we implement this partial fix, or wait until we can solve the comparison problem comprehensively (via AST analysis or accept the limitation)?

   **CRITICAL OBSERVATION**: Exec mode and context-aware filter mode ALREADY have this exact same limitation!
   ```bash
   # Exec mode warns for arithmetic ✓
   kelora -j --exec 'let x = e.statu + 1'
   # → WARNING: 'statu' missing

   # Exec mode does NOT warn for comparisons ✗
   kelora -j --exec 'if e.statu > 400 { print("high") }'
   # → (silent, no warning)
   ```

   **This means**:
   - We've already shipped this limitation and accepted it as reasonable
   - Implementing the fix makes behavior **consistent across all modes**
   - The inconsistency concern applies equally to exec mode (which users already have)
   - If it's acceptable for exec, it should be acceptable for filter

   **Revised verdict**: ✅ **YES, implement the fix**
   - Makes behavior consistent across all modes (exec, context-filter, regular filter)
   - Users who already use exec mode understand this limitation
   - Some warnings are better than none (proven by exec mode adoption)
   - The alternative (only some modes have warnings) is more confusing

2. 📝 **Add Documentation** (Option 2)
   - **Effort**: Low (update help text and examples)
   - **Impact**: Medium (users understand limitations)
   - **Risk**: None

### Medium-term Evaluation (Next Sprint)
3. 🔍 **Prototype AST Analysis** (Option 1a)
   - **Goal**: Validate that AST walking can reliably detect field accesses
   - **Effort**: Medium (1-2 days for prototype)
   - **Decision point**: If prototype works and AST API is stable enough, implement fully
   - **Fallback**: If Rhai internals API is too unstable, document as limitation

### Long-term Consideration (Future)
4. 🔧 **Strict Field Access Mode** (Option 3)
   - Only pursue if AST analysis proves too fragile
   - Could be valuable for CI/testing workflows even if not default behavior

### Final Recommendation (Revised 2025-11-24)

**Priority 1**: ✅ **YES - Fix Finding 1**
- Exec mode already has the same limitation (arithmetic warns, comparisons don't)
- This fix makes behavior **consistent across all modes**
- Quick win with minimal risk
- Converts errors to helpful warnings with suggestions

**Priority 2**: Add documentation about the comparison limitation (applies to all modes)

**Priority 3**: Evaluate AST analysis approach with a prototype. If it works reliably:
- The benefits (catching ALL typos including comparisons) are significant
- Zero runtime overhead is attractive
- Risk can be mitigated with tests and feature flags

If AST analysis proves too fragile or complex, accept the limitation and document it clearly. The current warning system already provides significant value by catching arithmetic operations on missing fields.

---

## Feasibility Assessment: Custom Property Getter (2025-11-24)

**Investigated approaches for tracking ALL field accesses (including comparisons):**

### ❌ Approach 1: Override Map indexers
- **Result**: Not possible - Rhai [explicitly disallows](https://rhai.rs/book/rust/indexers.html) overriding built-in Map indexers
- **Reason**: Performance - built-in Map operations are optimized

### ❌ Approach 2: Use on_var callback
- **Result**: Not applicable - [on_var](https://rhai.rs/book/engine/var.html) only intercepts top-level variable names (e.g., `e`)
- **Limitation**: Does not intercept property access (e.g., `e.field_name`)

### ❌ Approach 3: Custom wrapper type with property getters
- **Result**: Technically possible but architecturally prohibitive
- **Issues**:
  - Requires replacing all `rhai::Map` usage with custom type
  - Breaking change for existing scripts
  - Significant refactoring effort
  - Runtime overhead on every field access

### 🟡 Approach 4: AST analysis with `internals` feature
- **Result**: FEASIBLE but with risks
- **How**: Walk compiled [AST](https://rhai.rs/book/engine/ast.html) to extract field access patterns
- **Pros**:
  - Zero runtime overhead (analysis at compile time)
  - Catches ALL field accesses including comparisons
  - Non-invasive implementation
- **Cons**:
  - Requires `internals` feature (potentially unstable API)
  - May need updates on Rhai version upgrades
  - Medium implementation complexity
- **Recommendation**: Worth prototyping to validate stability

### Conclusion
Direct property access tracking via Rhai APIs is not feasible with current architecture. AST analysis is the only viable path forward for comprehensive field access warnings.
