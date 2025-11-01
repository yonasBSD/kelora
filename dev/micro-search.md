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
- `str.match(pattern: string) -> bool`
- `str.imatch(pattern: string) -> bool`
- `e.has(key: string) -> bool`

### Quick examples
```bash
kelora -f json --filter 'e.msg.ilike("*timeout*")'
kelora -f json --filter 'e.msg.imatch("user\\s+not\\s+found")'
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

### `match` / `imatch`
- Use the `regex` crate (search semantics, not anchored).
- `imatch` compiles with case-insensitive flag.
- Inputs are treated as UTF-8; regex crate already processes Unicode scalars appropriately.
- Invalid regex patterns yield `false` in default mode; they must raise an error when Kelora is running with `--strict`.
- Compiled regexes should be cached (e.g., `once_cell` + `Mutex<HashMap<(pattern, ci), Regex>>`) to amortize compilation cost.

### `has`
- Checks only top-level keys present on the event/map.
- Returns `false` if the key is missing **or** the stored value is Rhai unit `()`, honoring the project-wide sentinel meaning of “intentionally empty”.
- Returns `true` for any other value, including empty strings, zero, empty arrays/maps, etc.
- For nested paths, users continue to use `e.has_path("foo.bar")`.

## Implementation Notes
- Introduce `src/rhai_functions/micro_search.rs` with:
  - UTF-8-aware `glob_like`.
  - Regex helper with cache.
  - Public `register(engine: &mut Engine)` that wires all five methods.
- Add `unicode-normalization` dependency (or `unicode_casefold`) to support case folding.
- Update `src/rhai_functions/mod.rs` and any engine bootstrapping code to register the new module.
- Leave room for future perf tuning by isolating hot loops.

## Documentation Requirements
- Update `src/rhai_functions/docs.rs` entries for `like`, `ilike`, `match`, `imatch`, `has`.
- Clarify `has` vs raw `"key" in e`, especially the `()` sentinel behavior.
- Mention Unicode guarantees and pattern limitations.
- Add CLI help snippets (`--help-functions`, `--help-examples`) mirroring the examples above.

## Testing Requirements
- Unit tests covering:
  - ASCII success/failure cases.
  - Unicode scalars (`"🚀"`, `"café"`, `"Straße"`).
  - `ilike` folding (`"straße".ilike("STRASSE")`).
  - `has` returning `false` for `()`, `true` for other values.
  - Invalid regex returning `false`, `imatch` case-insensitive search.
- Integration smoke tests in `tests/integration_tests.rs` that run the CLI against small JSON/logfmt fixtures to exercise each helper end-to-end.

## Performance & Safety
- Glob matcher operates on scalars without heap reallocations beyond initial `Vec<char>` conversion.
- Regex cache prevents repeated compilation; consider LRU if memory pressure is observed, but plain `HashMap` is acceptable to start.
- No additional allocations or syscalls in hot path beyond what Rhai already performs.

## Rollout Checklist
1. Implement module and register functions.
2. Add dependency for Unicode folding.
3. Write unit + integration tests.
4. Update documentation and CLI help.
5. Run `just fmt`, `just lint`, `just test`.
6. Verify new helpers appear in `--help-functions` output.
