# Resilient-Mode Exit Codes: Investigation and Proposal

**Date:** 2026-05-03
**Status:** Analysis — pending decision
**Triggered by:** `just docs-build` warnings on `intro-to-rhai.md` examples

## Summary

Kelora's resilient mode was originally designed (July 23, 2025) to absorb runtime errors gracefully — broken field access in filters skips events, exec errors trigger atomic rollback. One day later (July 24, 2025) a separate change made the process exit `1` whenever any runtime error occurred during processing, even in resilient mode.

These two designs are in tension. The first says "errors on heterogeneous data are normal — the pipeline absorbs them." The second says "any error during processing means the run failed." The result is friction: the documented "natural filtering" and "progressive enhancement" patterns now require defensive `e.has(...)` guards on every operation, and even the diagnostic message tells users to opt out of the resilience model.

This document records the investigation, compares Kelora's behavior with peer tools, lays out the compatibility implications of restoring the original design, and proposes a narrow fix.

## Background

### The triggering symptom

`docs/tutorials/intro-to-rhai.md` Step 3 and Step 7 contain this example:

```bash
kelora -j examples/basics.jsonl \
    -e 'e.duration_s = e.duration_ms / 1000' \
    --filter 'e.duration_s > 1.0' \
    -k timestamp,service,duration_ms,duration_s
```

Most events in `examples/basics.jsonl` lack `duration_ms`. Kelora's default (resilient) behavior:

- The exec rolls the event back when `duration_ms` is missing — no `duration_s` is set
- The subsequent filter sees the rolled-back event without `duration_s` and drops it silently
- The single event with `duration_ms` (line 4) flows through correctly
- A diagnostic prints: `Exec errors: 5 total`
- **Process exits 1.**

`mkdocs-material`'s `markdown-exec` extension treats exit 1 as a build warning. The user-visible output is correct, but every doc build emits this warning.

### Two paths to "fix" this

Either change the example to add `e.has("duration_ms")` guards (canonical Kelora style today), or fix the underlying tension between resilient mode and exit codes. The first paint-by-numbers approach was started before pausing to investigate the deeper question.

## Investigation: original resiliency model vs. the exit-code change

### July 23, 2025 — original design

Two commits established the resiliency model:

**`c49678224` — "Add new --strict and --verbose flags for resiliency design"**

> - `--strict`: Exit on first error (replaces `--on-error=abort`)
> - `--verbose/-v`: Show detailed error information

**`209c6c411` — "Implement new resiliency model for filters and exec stages"**

The commit message states the design intent explicitly:

> **FilterStage Changes:**
> - Filter errors now evaluate to false (Skip) instead of propagating errors
> - In `--strict` mode, filter errors still propagate as before
> - This enables natural filtering where broken field access simply excludes events
>
> **ExecStage Changes:**
> - Implements atomic execution with rollback behavior
> - Work on event copy, only commit changes on success
> - On failure, return original event unchanged (no partial modifications)
> - In `--strict` mode, exec errors still propagate as before
> - Enables progressive enhancement where each stage adds what it can

Two key phrases: **natural filtering** and **progressive enhancement**. The whole design thesis is that heterogeneous logs are normal. Authors should not have to defensively guard every operation.

### July 24, 2025 — `cbf227b62` "Implement proper exit codes based on error detection"

One day later:

> - Exit 0: No errors occurred during processing
> - Exit 1: Parse errors or Rhai runtime errors occurred (both strict and resilient modes)
> - Exit 2: CLI/config errors (unchanged)
> - Signal codes: 130+ for interruptions (unchanged)
>
> Exit codes work consistently across all modes (sequential/parallel, verbose/quiet/normal)
> and provide reliable indicators for automation and CI/CD pipelines.

This added a `had_errors` check in `src/main.rs` that runs `has_errors_in_tracking()` against `__kelora_error_count_*` keys populated during processing. Any non-zero count → exit 1.

### The tension

| Question | Original design says | Exit-code change says |
|----------|----------------------|----------------------|
| Is missing-field-during-derivation an error? | No — the pipeline absorbs it | Yes — exit 1 |
| What is `--strict` for? | Opt into fail-fast | One of two things that produce exit 1 |
| What is the canonical CI gate? | `--strict` and `--assert` | The default, plus `--strict` for fail-fast within a run |

The original design had a clean two-mode story: default = resilient (absorb), strict = fail fast. The exit-code change added a third de-facto mode: "resilient at runtime, strict at exit," which collapses the meaning of the two original modes. The Kelora diagnostic message itself reflects this: it tells users `Use e.has("...")`, i.e. opt out of resilience for every operation.

## Comparison with peer tools

Each tool was given the same operation: derive a field, filter on it, against a JSON-lines input where most records lack the source field.

| Tool | On missing field arithmetic | Exit code | Style |
|------|------------------------------|-----------|-------|
| **Kelora today** | exec error, atomic rollback, diagnostic at end | **1** | Strict-by-noisy-default |
| **Kelora proposed** | exec error, atomic rollback, diagnostic at end | **0** | Resilient with diagnostics |
| **jq** | `null + arithmetic` is a runtime error per record | 5 | Loud; `//` operator for null defaulting |
| **mlr** | absent → absent → drop | 0 | Silently graceful |
| **awk** | missing → 0 / empty | 0 | Silently graceful |

Observations:

- Every other tool exits 0 on heterogeneous-data noise. They differ in loudness, not in exit code.
- mlr and awk silently drop. jq prints per-record errors and emits a distinct exit code (5), but it is structurally different — each invocation is a single transformation, not a streaming pipeline of named stages.
- Only Kelora today couples the diagnostic loudness to exit code 1 in default mode.

This isn't a normative argument ("everyone else does X so we should too"), but it shows that Kelora's current default is unusual for the class of tool.

## Simulated cases

Run against the current binary (Kelora 1.5.0, `examples/basics.jsonl`).

### Case 1: Heterogeneous data, naive script (the docs problem)

```bash
kelora -j examples/basics.jsonl \
    -e 'e.duration_s = e.duration_ms / 1000' \
    --filter 'e.duration_s > 1.0' \
    -k service,duration_s
```

**Today:** correct output, diagnostic `Exec errors: 5 total`, **exit 1**.
**Proposed:** identical output, identical diagnostic, **exit 0**.

### Case 2: Typo (real script bug)

```bash
kelora -j examples/basics.jsonl \
    -e 'e.upper_level = e.levle.to_upper()' \
    -k service,upper_level
```

**Today:** every event errors, diagnostic `Exec errors: 6 total`, **exit 1**.
**Proposed:** identical output, identical diagnostic, **exit 0**. Bug is visible interactively but does not fail CI by default.

This is the actual cost of the proposal. Mitigation: `--strict` and `--assert` are the canonical CI gates and do catch this. See cases 3 and 4.

### Case 3: Same typo with `--strict`

```bash
kelora -j examples/basics.jsonl --strict \
    -e 'e.upper_level = e.levle.to_upper()' \
    -k service,upper_level
```

Aborts on first event with a precise error pointer:

```
kelora: Pipeline error: exec error
  At 1:25 in exec script
  1 | e.upper_level = e.levle.to_upper()
    |                         ^
  Rhai: Function not found: to_upper (()) (line 1, position 25)
```

**Today and proposed:** **exit 1**, unchanged.

### Case 4: `--assert` as the documented data-quality gate

```bash
kelora -j examples/basics.jsonl --assert 'e.has("user_id")' -k service
```

Each event without `user_id` triggers an assertion failure with a labeled summary.

**Today and proposed:** **exit 1**, unchanged.

### Case 5: Parse error

```bash
printf '{"valid":"json"}\n{invalid json}\n' | kelora -j
```

**Today and proposed:** **exit 1**, unchanged. Parse errors are genuine "we cannot read the data" failures.

## Proposal

Narrow the conditions under which resilient mode produces exit 1.

### Behavior change

| Trigger | Today | Proposed |
|---------|-------|----------|
| Parse errors | Exit 1 | Exit 1 (unchanged) |
| File I/O failures | Exit 1 | Exit 1 (unchanged) |
| Assertion failures (`--assert`) | Exit 1 | Exit 1 (unchanged) |
| Filter errors in resilient mode | Exit 1 | **Exit 0**, diagnostic still printed |
| Exec errors in resilient mode | Exit 1 | **Exit 0**, diagnostic still printed |
| Anything in `--strict` mode | Exit 1 | Exit 1 (unchanged) |

The diagnostic infrastructure is preserved entirely — `Exec errors: N total` still appears at end of run, `--verbose` still shows per-error detail. Only the exit-code coupling changes.

### Why this works

- **Restores the original design intent.** "Natural filtering" and "progressive enhancement" stop requiring opt-out via `e.has(...)`.
- **Preserves CI signals that genuinely matter.** Parse errors, file I/O, assertion failures, and `--strict` runs all keep producing exit 1. Anyone running CI gates on data quality should already use `--assert`; the docs already recommend it.
- **`--strict` regains a clear purpose.** "Fail fast on any error" — distinct from the default rather than a refinement of it.
- **Aligns with peer tools.** mlr, awk, and (in spirit) jq do not couple "some records errored" to exit 1.
- **Eliminates the docs-build symptom.** The intro-to-rhai examples become correct as written.
- **Tutorial pedagogy improves.** `e.has(...)` becomes a tool for explicit data-quality logic (e.g., `e.has("required_id")` checks), not a defensive shield around every arithmetic operation.

### Code changes required

- `src/main.rs:381` — narrow `had_errors` to exclude exec/filter error counts. Keep parse errors, file I/O, assertion failures, plus all error categories in strict mode.
- Possibly a helper that distinguishes "real failures" from "absorbable runtime noise" in `has_errors_in_tracking()`.

### Test changes required

Audit of `tests/error_handling_tests.rs`:

- The vast majority of `assert_eq!(exit_code, 1, ...)` cases test parse errors (`invalid json line`, `{malformed json line}`). These are unaffected.
- **Exactly one test** asserts exit 1 specifically from exec errors: `test_exec_type_errors_are_reported_in_default_summary` (line 237). It runs `--exec 'e.level / 5'` against `{"level": "INFO"}` and asserts both the diagnostic and `exit_code != 0`.

Under the proposal, that test flips to assert "diagnostic appears, exit_code == 0" — the diagnostic is what matters; the exit code is now the user's explicit opt-in via `--strict`.

### Doc changes required

- `docs/reference/exit-codes.md` — remove "Filter errors" and "Exec errors" from the Exit 1 table. Add a callout: "For CI gating on script bugs, use `--strict`. For CI gating on data quality, use `--assert`."
- `CHANGELOG.md` — flag as a breaking change for users relying on default-mode exit codes; document the migration.
- Review `intro-to-rhai.md` — the originally-failing examples become correct as written. No `has()` rewrites needed.
- Consider revisiting `--help-rhai` / `--help-functions` text if either prescribes `e.has(...)` as the universal pattern (rather than as the explicit-validation tool it actually is).

## Compatibility break

### Who is affected

| Usage | Affected? |
|-------|-----------|
| Interactive CLI use | No — exit code irrelevant |
| Pipes (`kelora ... \| less`, `kelora ... \| grep`) | No |
| Scripts using `--strict` | No — `--strict` semantics unchanged |
| Scripts using `--assert` | No — `--assert` semantics unchanged |
| Tests in this repo | One test flips its assertion |
| `if kelora ...; then deploy` style CI (default mode) | **Yes** — won't block on filter/exec errors anymore |
| `kelora ... \|\| alert` cron jobs (default mode) | **Yes** — won't alert on filter/exec errors anymore |

### Migration path

The migration is documented and pre-existing:

- `kelora ...` → `kelora --strict ...` (fail fast on any error)
- `kelora ...` → `kelora --assert '<condition>' ...` (gate on data quality)

Both flags exist today, both are documented in `docs/reference/exit-codes.md`, both are arguably the *correct* tool for these use cases. The current default lets users get away with implicit reliance on a coupling that contradicts the documented design.

### Severity assessment

- **No known users rely on this.** No GitHub issues, no support requests, no public examples.
- The behavior has been documented for ~10 months (July 2025 → present), so it is a real public contract — but a narrow and recent one.
- The diagnostic is preserved, so interactive users still see problems.
- The change is detectable: anyone affected sees their CI start passing where it used to fail. The migration is short and clear.

## Alternatives considered

### A. Status quo + fix the docs with `has()` guards

Keep current exit-code behavior. Update tutorial examples to wrap operations in `e.has(...)`. Possibly reorder `intro-to-rhai.md` so Step 8 ("Checking if Fields Exist") moves earlier.

**Pros:** zero code change, zero compatibility risk, examples become defensive-by-default.
**Cons:** every tutorial pattern gets one extra layer of ceremony; `e.has(...)` becomes an idiomatic shield rather than an explicit-validation tool; the gap between the documented design and the actual ergonomics persists.

### B. Distinguish "all events errored" from "some events errored"

Heuristic: if `error_count == event_count`, treat as a real bug and exit 1. Otherwise treat as heterogeneous data and exit 0.

**Pros:** catches typo-style bugs while absorbing heterogeneous-data noise.
**Cons:** non-orthogonal to flags, hard to reason about (what if some events legitimately error?), surprising boundary behavior (1/100 vs. 100/100). Adds heuristic complexity to a previously simple model.

### C. Add a flag to control exit-on-runtime-errors

Introduce `--no-exit-on-runtime-errors` (or invert: a flag to opt in). Migrate via deprecation cycle.

**Pros:** reversible, no surprise breaks.
**Cons:** another flag in a CLI that already has `--strict` and `--assert` for adjacent purposes. Paradox of choice. Doesn't actually resolve the design tension — just lets users pick a side.

### D. Separate error-category exit codes (jq-style)

Use distinct exit codes (e.g., 4 for runtime errors, 5 for parse, 6 for assertion). `if kelora ...; then` still fails on any error, but operators can distinguish.

**Pros:** preserves the current "any error fails CI" guarantee while giving operators richer signal.
**Cons:** breaking change in the other direction (anyone matching exit 1 specifically). Doesn't solve the underlying ergonomic friction. Adds memorization burden.

### E. Conservative variant of the proposal: ship behind a flag for one release

Introduce `--no-exit-on-runtime-errors` as opt-in for one release, then flip default in next major.

**Pros:** maximum caution, telemetry/feedback opportunity.
**Cons:** drags out the resolution; the current behavior is already a deviation from the original design and not load-bearing for any known user.

## Open questions

1. Do we want `--verbose` interactions to change? Today `-v` shows per-error detail; under the proposal that is unchanged but its CI utility shifts (it remains a debugging tool, not a CI gate).
2. Should the diagnostic line ("Exec errors: N total") become more prominent or change wording, given that exit 0 means it is the only signal? Current wording is informational; might benefit from a stronger affordance, e.g., "5 events skipped due to exec errors. Use `--verbose` to see each, or `--strict` to fail on these."
3. Hint message in errors: today `Use e.has("...")` is suggested for every missing-field error. If this is no longer the canonical fix, the hint should be reworded — `e.has(...)` is for *explicit* validation; the diagnostic itself should not push users away from the resilient model.
4. Is there value in surfacing per-stage error counts (e.g., "Stage 1 of 3: 5 exec errors, Stage 2: 0, Stage 3: 0") so users can see *which* stage the noise is in?

## Decision

Pending. This document captures the analysis; the next step is to choose a path:

- **Recommended:** ship the narrow change (Section "Proposal").
- **Conservative:** Alternative A (status quo + `has()` guards in docs).
- **Maximally cautious:** Alternative E (flag for one release, flip in next major).
