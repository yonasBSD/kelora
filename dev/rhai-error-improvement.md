## Rhai Error UX Improvements — Plan

Goal: unify and upgrade Rhai error reporting (compile + runtime across all stages) with actionable hints, without worrying about backward compatibility. Execute in small, verifiable steps.

### Phase 1 — Unified formatter and coverage
- Introduce a single `RhaiDiagnostic` helper that formats: stage/name, line/col, snippet with caret, raw Rhai message, and optional emoji toggle.
- Route every Rhai `map_err` through it (compile + runtime) for filter/exec/begin/end/span-close/windowed variants.
- Keep existing `ErrorEnhancer` suggestions; ensure they’re invoked wherever runtime errors surface.
- Add tests for formatter (unit-level: snippet/caret, stage label, raw message included).

### Phase 2 — Smarter suggestions and stack context
- Expand suggestions: function/property not found → Levenshtein over registered functions/keys; remind about method sugar; type mismatch → expected vs got with coercion hints; filter → “must return bool”.
- Surface top 3 frames from Rhai call stack (gate full stack behind verbosity/flag).
- Cover property errors with available field names when scope has `e`.

### Phase 3 — Output modes and knobs
- Add `--error-format=plain|detailed|json` (or config) to control verbosity; default to detailed.
- Respect `--no-emoji` in formatter output.
- Keep JSON minimal (stage, pos, message, suggestions) for tooling.

### Phase 4 — Preflight/dry-run (optional)
- After compile, offer a preflight check that runs expressions against a dummy scope to catch missing vars/functions early; gate behind a flag/verbosity.

### Delivery/verification notes
- After each phase: run `just fmt`, `just lint`, `just test`.
- Phase boundaries are independent; we can ship Phase 1 alone if needed.

### Open questions
- Do we want `--error-format` or reuse existing verbosity flags? (lean toward explicit flag)
- Should preflight run by default or only with `-v`/flag? (propose opt-in)
