# Micro Search Methods – Feature Specification

## Goals
- Provide first-class Rhai helpers for common string filtering and event-key checks inside `--filter`.
- Keep the API small, predictable, and Unicode-safe so beginners can reach for it without learning new DSLs.
- Avoid adding new CLI flags or secondary parsing modes.

## Non-Goals
- No new wildcards beyond `*` (zero or more scalars) and `?` (exactly one scalar).
- No path-style globbing (`[]`, `{}`, `**`) or regex substitutions.
- No deep field traversal; nested key discovery remains the job of `has_path`.
- No change to existing `contains`, `matches`, or core Rhai semantics.

## Surface API (Rhai)
- `str.like(pattern: string) -> bool`
- `str.ilike(pattern: string) -> bool`
- `e.has(key: string) -> bool`

### Quick examples
```bash
kelora -f json --filter 'e.msg.ilike("*timeout*")'
kelora -f json --filter 'e.msg.matches("user\\s+not\\s+found")'
kelora -f logfmt --filter 'e.has("user") && e.user.like("alice*")'
```

## Detailed Behavior

### `like` / `ilike`
- Match the **entire** string (implicit anchors).
- Wildcards: `*` matches zero or more Unicode scalar values; `?` matches exactly one scalar.
- `like` compares scalars exactly as stored.
- `ilike` applies Unicode simple case folding to both haystack and pattern:
  - Normalize with NFKC, then apply default case fold (requires `unicode-normalization` or equivalent).
  - Guarantees `"Straße".ilike("strasse") == true`, `"CAFÉ".ilike("café") == true`.
- Implementation iterates over `Vec<char>` (scalar values). No byte slicing.
- **Performance optimization**: Fast-path for ASCII-only patterns and haystacks using byte-level matching (10-100x faster than Unicode scalar iteration).

### `matches`
- Reuse the existing Rhai `matches` helper for regex search; no new variant is added.
- Inputs are treated as UTF-8; regex crate already processes Unicode scalars appropriately.
- Invalid regex patterns should raise errors (respecting `--strict` and quiet modes) instead of silently returning `false`.
- Compiled regexes should be cached using **thread-local storage** (`thread_local!` with `RefCell<LruCache>`) to avoid lock contention in `--parallel` mode.
- **LRU eviction**: Default to 1000 compiled patterns per thread to prevent unbounded memory growth with dynamic patterns.
- **ReDoS protection**: The `regex` crate provides bounded worst-case time (linear in input size), but complex patterns can still be expensive:
  - Use `RegexBuilder::size_limit()` and `dfa_size_limit()` to bound compilation cost.
  - Document warning against nested quantifiers like `(.*)*` which trigger worst-case behavior.

### `has`
- Checks only top-level keys present on the event/map.
- Returns `false` if the key is missing **or** the stored value is Rhai unit `()`, honoring the project-wide sentinel meaning of "intentionally empty".
- Returns `true` for any other value, including empty strings, zero, empty arrays/maps, etc.
- For nested paths, users continue to use `e.has_path("foo.bar")`.

## Function Comparison Table

| Function | Use Case | Pattern Type | Anchored | Case-Sensitive | Example |
|----------|----------|--------------|----------|----------------|---------|
| `contains()` | Substring search | Literal | No | Yes | `"foobar".contains("oba")` → true |
| `like()` | Simple wildcards | Glob (`*`, `?`) | Yes (full match) | Yes | `"foobar".like("foo*")` → true |
| `ilike()` | Case-insensitive wildcards | Glob (`*`, `?`) | Yes (full match) | No (Unicode fold) | `"FooBar".ilike("foo*")` → true |
| `matches()` | Complex patterns | Regex | No (search) | Yes | `"foobar".matches("ba.")` → true |
| `has()` | Key existence (non-unit) | N/A | N/A | N/A | `e.has("user")` → false if `()` |
| `has_path()` | Nested key existence | Dot-separated path | N/A | N/A | `e.has_path("user.id")` |

**Note**: `like()` and `ilike()` differ from SQL LIKE in that they match the **entire string** (anchored), not substrings.

## Implementation Notes
- Introduce `src/rhai_functions/micro_search.rs` with:
  - UTF-8-aware `glob_like` with ASCII fast-path.
  - Thread-local regex cache utilities (`thread_local!` with `RefCell<LruCache>`).
- Public `register(engine: &mut Engine)` that wires `like`, `ilike`, and `has`.
- Enhance existing `matches` helper to use the new thread-local regex cache.
- Add dependencies:
  - `unicode-normalization` (or `unicode_casefold`) for case folding.
  - `lru` crate for bounded cache implementation.
- Update `src/rhai_functions/mod.rs` and any engine bootstrapping code to register the new module.
- Leave room for future perf tuning by isolating hot loops.

## Documentation Requirements
- Update `src/rhai_functions/docs.rs` entries for `like`, `ilike`, `has`, and document the regex caching/error behavior for `matches`.
- Clarify `has` vs raw `"key" in e`, especially the `()` sentinel behavior.
- Mention Unicode guarantees and pattern limitations.
- Include the function comparison table showing `contains()` vs `like()` vs `ilike()` vs `matches()`.
- Add CLI help snippets (`--help-functions`, `--help-examples`) mirroring the examples above.
- **Document ReDoS best practices** in `--help-functions`:
  ```
  ⚠️  REGEX PATTERN TIPS
  - Avoid nested quantifiers: (.*)*  (triggers worst-case O(n·m) behavior)
  - Prefer: .*error  over  (.*)*error
  - Complex patterns compile once and are cached (1000 per thread)
  - Invalid regex patterns cause errors (not silent false returns)
  ```

## Testing Requirements
- Unit tests covering:
  - ASCII success/failure cases.
  - Unicode scalars (`"🚀"`, `"café"`, `"Straße"`).
  - `ilike` folding (`"straße".ilike("STRASSE")`).
  - `has` returning `false` for `()`, `true` for other values.
  - Invalid regex error propagation.
- Integration smoke tests in `tests/integration_tests.rs` that run the CLI against small JSON/logfmt fixtures to exercise each helper end-to-end.

## Performance & Safety
- Glob matcher operates on scalars without heap reallocations beyond initial `Vec<char>` conversion.
- **ASCII fast-path**: When both pattern and haystack are ASCII-only, use byte-level comparison instead of Unicode scalar iteration for 10-100x speedup.
- Regex cache uses **thread-local storage** with bounded LRU (1000 entries/thread) to prevent lock contention in `--parallel` mode and cap memory growth.
- No additional allocations or syscalls in hot path beyond what Rhai already performs.

## Rollout Checklist
1. Implement module and register functions.
2. Add dependency for Unicode folding and LRU cache.
3. Write unit + integration tests.
4. Update documentation and CLI help.
5. Run `just fmt`, `just lint`, `just test`.
6. **Run `just bench` to verify no performance regressions** (especially for `like`/`ilike` hot paths).
7. Verify new helpers appear in `--help-functions` output with comparison table and ReDoS guidance.
