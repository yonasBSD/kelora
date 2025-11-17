# Hardened and Sandbox Modes for Rhai Execution

## Baseline (unchanged)
- Default: no resource limits on Rhai execution; filesystem writes are blocked unless `--allow-rhai-io` is set. Filesystem reads remain allowed unless explicitly sandboxed.
- Goal: keep parity with jq/awk/Python so normal/local usage is not surprised by limits.

## Objectives
- Provide opt-in hardening knobs for users running untrusted scripts or shared automation.
- Keep behavior unchanged for trusted/local workflows unless flags/configs opt in.

## Modes
### `--hardened` (DoS resilience preset)
- Purpose: protect against runaway CPU/time/memory/depth in Rhai code paths.
- Effect: applies a preset of resource budgets to every Rhai engine (filters, `--exec`, `--begin`/`--end`, span hooks, per-worker in `--parallel`).
- Preset values (tunable constants in config layer):
  - Max operations: 100_000_000
  - Wall-clock: 60s per engine
  - Total allocation: 256 MiB
  - Max string: 32 MiB
  - Max array/map: 200_000 elements
  - Max call depth: 128
- All budgets remain configurable individually via flags/config (see Configuration).
- Diagnostics: terminate with exit code 1 and name the tripped guard (“Rhai limit hit: exceeded max operations (100,000,000)”); honor emoji/no-emoji.
- `--script-unlimited` still disables all budgets even if `--hardened` is present (mutual exclusion with a warning).

### `--sandbox` (capability restriction)
- Purpose: reduce data exfiltration/side effects from Rhai scripts.
- Effect:
  - Deny filesystem reads and writes from Rhai by default.
  - `--allow-rhai-io` re-allows both reads and writes explicitly (still blocked by OS perms).
  - Environment access remains as currently implemented; if we add env restrictions later, gate them here.
- Independent of `--hardened`; users can combine both.

## Configuration Surface
- New CLI flags (with config keys):
  - `--hardened` / `script.hardened = true`
  - `--sandbox` / `script.sandbox = true`
  - `--script-max-ops <u64>` / `script.max_ops`
  - `--script-max-wall <duration>` / `script.max_wall`
  - `--script-max-mem <bytes>` / `script.max_mem`
  - `--script-max-str <bytes>` / `script.max_str`
  - `--script-max-array <len>` / `script.max_array`
  - `--script-max-map <len>` / `script.max_map`
  - `--script-max-depth <u32>` / `script.max_depth`
  - `--script-unlimited` / `script.unlimited = true` (disables all budgets, even if `--hardened`)
- Precedence: CLI > project config > user config; `--script-unlimited` overrides others.
- Help/Docs: show current limits and whether sandbox is active in `--help-rhai` intro.

## Behavior
- Hardened budgets are applied when constructing each `RhaiEngine` and cloned into worker engines.
- Sandbox applies at IO gating points: deny FS reads/writes unless `--allow-rhai-io`; other side-effect gates unchanged.
- When a limit is exceeded: fail fast, emit guard-specific diagnostic, exit code 1; avoid partial output flushing beyond what already streamed.

## Testing
- Hardened:
  - Infinite loop under `--exec --hardened` hits TooManyOperations quickly.
  - Push loop hits DataTooLarge under default hardened mem cap.
  - Recursion hits max call depth.
  - Normal examples still pass under default (non-hardened) mode.
  - Parallel smoke: per-worker budget applies.
  - `--script-unlimited` allows above to run (mark slow/ignored in CI).
- Sandbox:
  - Rhai FS read/write fails under `--sandbox` without `--allow-rhai-io`.
  - FS operations succeed when `--allow-rhai-io` is combined with `--sandbox`.

## Migration & Rollout
- Defaults unchanged; features are opt-in.
- Document in CHANGELOG and `--help-rhai`; clarify that `--hardened` targets runaway scripts and `--sandbox` targets IO isolation.
