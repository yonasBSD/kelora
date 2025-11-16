# Quiet/Output Control Redesign

## Rationale
- Current ladder (-q/-qq/-qqq) couples diagnostics suppression to event suppression and still allows metrics/script output under --silent, leading to surprises.
- Users often want short commands for common cases: "metrics only" or "stats+metrics without events"; mapping -q to "no events" is familiar (grep, curl) and keeps lines short.
- We allow breaking changes; goal is a logical, orthogonal set of toggles that override config defaults cleanly.

## Terminology
- Events: formatter output to stdout (or --output-file).
- Diagnostics: warnings/errors, summaries to stderr (non-script).
- Script output: Rhai print/eprint and side-effect warnings.
- Stats: --stats output to stderr.
- Metrics: table/json/file from track_*().

## Proposed CLI Semantics
- `-q`/`--quiet`: suppress events (equivalent to `--no-events` / `-F none`). Diagnostics, stats, metrics, script output remain unless further flags.
- `--no-events`: alias of -q.
- `--no-diagnostics`: suppress diagnostics and error summaries; does not affect events.
- `--silent`: suppress all stdout/stderr emitters (events, diagnostics, stats, terminal metrics, script output) while allowing non-terminal sinks (metrics files). Sets Rhai side-effect suppression. Emits exactly one concise fatal line on errors, even with `--no-diagnostics`. Dominates emitter flags/config; terminal sinks requested are suppressed without error, file sinks proceed.
- `--no-silent`: disable a silent default from config; restores normal output unless suppressed by other flags (e.g., `-q`).
- `--metrics-only`: suppress events and diagnostics, suppress stats; emit metrics (stderr/JSON/file) unless `--silent` is present (terminal metrics suppressed, metrics files still written). Implies `-F none`, `--no-diagnostics` (fatal line still allowed), `--no-stats`, and `--no-script-output`; does **not** imply `--silent`. On fatal errors, emit one concise diagnostic line and exit non-zero.
- `--stats-only`/`-S`: suppress events; emit stats to stderr; keep diagnostics on (fatal line still allowed), and imply `--no-script-output` for a clean stats channel. Does not imply `--silent`.
- No conflicts: `--silent` overrides/ignores terminal emitters (`--metrics*`, `--metrics-only`, `--stats`) from config or CLI. Suppress-only flags are allowed but redundant.
- `--no-script-output` (optional): suppress Rhai print/eprint without affecting diagnostics/stats/metrics; implied by --silent, `--metrics-only`, and `--stats-only`.
- `-F none`: explicit formatter choice for "no events"; identical effect to -q/--no-events for event stream. Keeps diagnostics unless --no-diagnostics or --silent.
- `--stats`/`-s` / `--no-stats`: as today, but honored by --silent.
- `--metrics` / `--no-metrics` / `--metrics-json` / `--metrics-file`: as today, but honored by --silent (terminal metrics suppressed; metrics files still written). Emit to all selected sinks except where existing conflicts remain (table vs stderr JSON): `--metrics` conflicts with `--metrics-json`; `--metrics-file` can combine with either.

## Effective Behavior Matrix
- Default: events on; diagnostics on; stats/metrics off unless requested; script output on.
- `-q`: events off; diagnostics/stats/metrics/script output unchanged.
- `-q --no-diagnostics`: events + diagnostics off; stats/metrics/script output unchanged.
- `-q -s -m`: stats and metrics emit; no events; diagnostics on.
- `--silent`: no stdout/stderr (events/diagnostics/stats/terminal metrics/script output); exit code only; metrics files still written.
- `--metrics-only`: no events, no diagnostics (except single fatal line), no stats, no script output; metrics emit (stderr/JSON/file) unless `--silent` present (terminal suppressed; files written).
- `--stats-only`: no events, stats emit, diagnostics on (fatal line allowed), no script output; `--silent` still suppresses the terminal stats.
- `-F none`: same as `-q` for events; can combine with other toggles as above.

## Config Interaction
- Config defaults are prepended then overridden by CLI (unchanged flow). Every default can be flipped: `--no-events`, `--no-diagnostics`, `--silent`, `--no-silent`, `--no-stats`, `--no-metrics`, `--no-script-output` all override config defaults.

## Edge Cases & Interaction Notes
- Metrics/stats: `--silent` suppresses terminal metrics (table/JSON) and stats/error summaries, but still permits metrics file writes. `-q` does not impact metrics/stats; no conflicts.
- `--silent` vs `--no-silent`: CLI args override config defaults; if both are present, last occurrence wins (familiar clap semantics).
- Script output: `--silent` (and `--no-script-output`, if present) suppress Rhai print/eprint and side-effect warnings. Builder must still set `set_suppress_side_effects(true)` in these modes.
- Error summary path: automatic summaries/stats fallbacks should honor `--silent` and `--no-diagnostics` (not tied to quiet ladder anymore).
- `--metrics-only` interaction: combining with `--silent` is allowed; terminal emitters are suppressed, metrics files still written. Combining with `--metrics/--metrics-file/--metrics-json` is redundant but allowed.
- Silent dominance: `--silent` may be combined with any emitter flag or config default; it wins for terminal sinks and suppresses stdout/stderr. Metrics files still write. Allow `--silent` with suppress-only flags (`-q`, `--no-events`, `--no-diagnostics`, `--no-script-output`).
- `stats-only` combos: `-S` forces `OutputFormat::None`; adding `-q` is redundant; `--silent` still suppresses stats; `--no-script-output` implied.
- Output file + no events: with `--output-file` + `-q`/`-F none`, we may produce an empty (or absent) event file; diagnostics remain unless suppressed by `--silent`/`--no-diagnostics`.
- `--no-input`: scripts that only emit metrics/stats should still be silenced by `--silent`; `-q` should not change behavior when no events exist.
- Env flags: `NO_EMOJI` unchanged; `KELORA_NO_TIPS` currently tied to quiet>0—decide whether `-q` or `--no-diagnostics` should also suppress tips (recommended: suppress tips under `-q` or `--silent`, leave `--no-diagnostics` opt-in).

## Help/Docs
- Update CLI help, --help-quick, --help-examples, and docs.rs references to quiet levels; remove ladder semantics. Clarify equivalence of `-q`, `--no-events`, and `-F none` for events.

## Implementation Notes
- CLI: repurpose `-q/--quiet` help text to "suppress events"; keep `--no-events` alias; add `--no-diagnostics`; add `--no-silent` to flip config defaults off; redefine `--silent` help to promise zero stdout/stderr while allowing metrics files; optional `--no-script-output` gate. No conflicts when combining `--silent` with emitters—silent dominates terminal sinks.
- Config derivation: swap quiet level ladder for booleans: `quiet_events` (from -q/--no-events or `-F none`), `suppress_diagnostics`, `silent`, `suppress_script_output` (silent or explicit). Keep `OutputFormat::None` respected independently but OR-ed with quiet for events.
- Pipeline builder: set Rhai side-effect suppression when `suppress_script_output` is true.
- Output decisions:
  - Events: off if `quiet_events` or `output_format == None`.
  - Diagnostics/stats/terminal metrics: skipped if `silent`; diagnostics also skipped if `suppress_diagnostics`, except emit the one-line fatal error.
  - Script output: skipped if `suppress_script_output` or `silent`.
  - Metrics file writes: allowed even if `silent`.

## Migration Guidance
- Old `-q` (diagnostics off) becomes `--no-diagnostics`.
- Old `-qq` (no events) becomes `-q` (or `-F none`/`--no-events`).
- Old `-qqq/--silent` (mostly silent but still metrics) becomes new `--silent` (truly silent).
- New `--metrics-only`: use when you want metrics emitted without events/diagnostics/stats; not available in the prior ladder.
