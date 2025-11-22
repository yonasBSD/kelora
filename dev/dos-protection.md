# Hardened and Sandbox Modes for Rhai Execution

## Baseline (unchanged)
- Default: no resource limits on Rhai execution; filesystem writes are blocked unless `--allow-rhai-io`. Filesystem reads remain allowed unless explicitly sandboxed.
- Goal: keep parity with jq/awk/Python so normal/local usage is not surprised by limits.

## Objectives
- Only two primary switches: resilience (`--hardened`) and capability reduction (`--sandbox`).
- Everything else is opt-in; no surprise limits for trusted workflows.

## Modes
### `--hardened` (DoS resilience preset)
- Purpose: protect against runaway CPU/time/memory/depth in Rhai code paths.
- Effect: applies a preset of resource budgets to every Rhai engine (filters, `--exec`, `--begin`/`--end`, span hooks, per-worker in `--parallel`).
- Preset values (tunable constants in config layer):
  - Max operations: 100_000_000
  - Wall-clock: 60s per engine
  - Max string: 32 MiB
  - Max array/map: 200_000 elements
  - Max call depth: 128
- All budgets remain configurable individually via config file (see Configuration).
- Diagnostics: terminate with exit code 1 and name the tripped guard ("Rhai limit hit: exceeded max operations (100,000,000)"); honor emoji/no-emoji.
- `--script-unlimited` still disables all budgets even if `--hardened` is present (mutual exclusion with a warning).

### `--sandbox` (capability restriction)
- Purpose: reduce data exfiltration/side effects from Rhai scripts.
- Effect:
  - Deny filesystem reads and writes from Rhai by default.
  - `--allow-rhai-io` re-allows both reads and writes explicitly (still blocked by OS perms).
  - Environment access remains as currently implemented; if we add env restrictions later, gate them here.
- Independent of `--hardened`; users can combine both.

## Configuration Surface

### CLI Flags
- **Primary modes:**
  - `--hardened` / `script.hardened = true` - Enable DoS protection preset
  - `--sandbox` / `script.sandbox = true` - Block filesystem access
  - `--script-timeout <duration>` / `script.timeout = "60s"` - Override wall-clock timeout (common need)
  - `--script-unlimited` / `script.unlimited = true` - Disable all budgets (overrides `--hardened`)

### Config File (`[script]` section)
Advanced users can tune individual limits in `.kelora.ini`:
```ini
[script]
hardened = true
sandbox = false
timeout = "120s"

# Individual limit overrides (power users)
max_ops = 100000000
max_str = "32MiB"
max_array = 200000
max_map = 200000
max_depth = 128
```

### Precedence & Help
- Precedence: CLI > project config > user config; `--script-unlimited` overrides others.
- Help/Docs: show current limits and whether sandbox is active in `--help-rhai` intro.

## Behavior
- Hardened budgets are applied when constructing each `RhaiEngine` and cloned into worker engines.
- Sandbox applies at IO gating points: deny FS reads/writes unless `--allow-rhai-io`; other side-effect gates unchanged.
- When a limit is exceeded: fail fast, emit guard-specific diagnostic, exit code 1; avoid partial output flushing beyond what already streamed.

## Implementation Notes

### Rhai Engine Limits
All proposed limits are directly supported by Rhai `Engine` API:
- `set_max_operations(u64)` - Operation budget
- `on_progress(callback)` - Wall-clock timeout via callback
- `set_max_string_size(usize)` - String size limit
- `set_max_array_size(usize)` - Array element limit
- `set_max_map_size(usize)` - Map element limit
- `set_max_call_levels(usize)` - Call stack depth

**Note on memory limits:** Rhai does not provide a single method to limit total memory allocation across all data structures. Memory protection is achieved through the combination of individual limits (strings, arrays, maps). For strict total memory caps, use OS-level controls (`ulimit`, cgroups).

## Testing
- **Hardened mode:**
  - Infinite loop under `--exec --hardened` hits TooManyOperations quickly.
  - Push loop hits DataTooLarge under default hardened mem cap.
  - Recursion hits max call depth.
  - Normal examples still pass under default (non-hardened) mode.
  - Parallel smoke: per-worker budget applies.
  - `--script-unlimited` allows above to run (mark slow/ignored in CI).
- **Sandbox mode:**
  - Rhai FS read/write fails under `--sandbox` without `--allow-rhai-io`.
  - FS operations succeed when `--allow-rhai-io` is combined with `--sandbox`.

## Migration & Rollout
- Defaults unchanged; features are opt-in.
- Document in CHANGELOG and `--help-rhai`; clarify that `--hardened` targets runaway scripts and `--sandbox` targets IO isolation.
