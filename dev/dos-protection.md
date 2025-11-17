# DoS Protection for Rhai Execution

## Problem
- Current Rhai execution (`--exec`, `--begin`, `--end`, filters, span hooks) has no resource guards. An infinite loop or unbounded push can peg CPU and exhaust memory until the process is killed, as the security reviewer demonstrated.
- Production deployments that accept untrusted scripts (or trusted operators making mistakes) are exposed to trivial denial of service.

## Goals
- Provide safe-by-default execution budgets for all Rhai code paths.
- Make limits configurable via CLI/config, with explicit opt-out for trusted/bench scenarios.
- Fail fast with clear diagnostics and exit code 1 when a guard trips.
- Keep defaults high enough that normal scripts/examples are unaffected.

## Non-Goals
- Full multitenant sandboxing—focus on stopping runaway CPU/memory from Rhai itself.
- OS-level cgroups or job objects; this spec stays within the process and Rhai engine hooks.

## Guardrails to Add (per Engine instance)
1) **Operation budget**
   - Use `Engine::set_max_operations(Some(N))` with a sensible default (e.g., 5_000_000).
   - Applies to every script stage (filters, exec, begin/end, span hooks) and each worker in `--parallel`.
   - Hitting the limit raises `ErrorTooManyOperations` → map to a fatal diagnostic and exit code 1.

2) **Wall-clock budget**
   - Use Rhai progress hook (`Engine::on_progress` or equivalent) to check elapsed time since the script started and terminate with `ErrorTerminated` when over the limit.
   - Default: 2s for sequential mode; 1s per task in `--parallel` to keep work-stealing healthy.

3) **Memory caps**
   - Set `set_max_string_size`, `set_max_array_size`, and `set_max_map_size` to cap runaway allocations.
   - Add a coarse total allocation cap via a shared counter checked in the progress hook; abort with `ErrorDataTooLarge` when exceeded.
   - Default suggestion: 128 MiB total across all Rhai values, strings ≤ 16 MiB, arrays/maps ≤ 100_000 elements.

4) **Depth limits**
   - Set `set_max_call_levels(Some(depth))` to stop stack explosions (default 64).

5) **Side-effect controls**
   - Already gated by `--allow-rhai-io`; keep those unchanged. Limits above still apply whether or not IO is allowed.

## Configuration Surface
- CLI flags (and matching config keys):
  - `--script-max-ops <u64>` / `script.max_ops`
  - `--script-max-wall <duration>` / `script.max_wall`
  - `--script-max-mem <bytes>` / `script.max_mem`
  - `--script-max-str <bytes>` / `script.max_str`
  - `--script-max-array <len>` / `script.max_array`
  - `--script-max-map <len>` / `script.max_map`
  - `--script-max-depth <u32>` / `script.max_depth`
  - `--script-unlimited` (explicit opt-out; disables all of the above)
- Defaults baked into config layer; flags override config; `--script-unlimited` wins.
- Show current limits in `--help-rhai` and `--help-functions` intro.

## Behavior
- Limits are applied when constructing each `RhaiEngine` (both main and per-thread clones) so parallel mode inherits them.
- When a limit is exceeded:
  - Emit a clear diagnostic naming the tripped guard (e.g., “Rhai limit hit: exceeded max operations (5,000,000)”).
  - Respect emoji/no-emoji output conventions; exit code 1.
  - No partial output flush beyond whatever was already emitted before the error.
- Scripts compiled before limits change keep using the active limits of their `Engine`; cloning must copy the guard settings.

## Testing
- Add integration tests:
  - Infinite loop under `--exec` exits with TooManyOperations within a short wall time.
  - Tight loop with `while true { arr.push(0); }` fails with DataTooLarge under default mem cap.
  - Recursive call chain hits max call depth.
  - Normal scripts/examples still succeed under defaults.
  - `--script-unlimited` allows the above to run without early termination (but mark as slow/ignored in CI).
- Add a quick smoke test in `--parallel` mode to ensure per-worker limits apply.

## Migration & Rollout
- Implement limits with conservative defaults, document in CHANGELOG, and note that behavior is stricter (breaking change acceptable).
- Guard flags are backwards compatible; only opt-out users need to update scripts (`--script-unlimited` or bump values).

## Open Questions
- Precise defaults—tune after measuring typical example workloads.
- Whether to enforce limits during `--begin` separately (single shot) versus per-event; current plan: per-engine per-run.
