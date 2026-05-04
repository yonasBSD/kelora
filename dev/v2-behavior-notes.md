# Kelora v2 Behavior Notes

These are candidate behavior changes to consider while moving Kelora to v2.0
for the new resiliency model. The goal is not to make Kelora less forgiving for
messy logs, but to remove places where it silently hides ambiguity, typos, or
data loss.

## High-Value Breaking Changes

### Enforce Typed Parser Conversions

Typed parser annotations should mean "this field has this type", not "try this
type and silently fall back to string".

Current behavior:
- `status:int` can become `"abc"` in resilient mode.
- Regex and cols typed captures can do the same.

Candidate v2 behavior:
- Type conversion failures become parse/recovered errors regardless of
  `--strict`.
- `--strict` controls whether the run aborts immediately.
- Resilient mode records the error and applies the standard parse-error policy.
- If fallback remains useful, expose it explicitly, e.g.
  `--type-error=string|skip|error`.

Rationale: type annotations are schema declarations. Silent type drift makes
downstream Rhai scripts and CSV/JSON consumers harder to reason about.

### Tighten CSV/TSV Column Shape

CSV currently uses flexible parsing and silently ignores fields beyond the
known header set. Headerless CSV/TSV also derives `c1`, `c2`, ... from the
first row, so later wider rows can lose data.

Candidate v2 behavior:
- Extra columns are recovered parse errors by default.
- Missing columns are represented explicitly, preferably as empty fields for
  CSV compatibility or `()` when the output format can preserve null-like
  values.
- Optional mode for exploratory work: capture extra columns into `_extra`.

Rationale: silent column loss is worse than a visible recovered error.

### Validate Config Files Strictly

Config parsing currently ignores unknown root keys and unknown sections.

Candidate v2 behavior:
- Unknown root keys, unknown sections, and malformed lines are config errors.
- Keep an explicit compatibility flag only if real users need it, e.g.
  `--config-lenient`.

Rationale: config typos should not silently change pipeline behavior.

### Error on Unused Includes

`--include` with no following script stage is currently ignored.

Candidate v2 behavior:
- Treat trailing or otherwise unused includes as CLI usage errors.

Rationale: a user who passed an include almost certainly expected it to affect
the run.

### Reject Invalid Timestamp Timezones

Invalid timestamp timezone configuration currently falls back to local time.

Candidate v2 behavior:
- Invalid `--input-tz` values fail during configuration validation.
- Runtime timestamp parsing should not substitute local time for an invalid
  explicit timezone.

Rationale: timezone fallback can shift time filters and span boundaries without
making the mistake visible.

### Revisit Missing Data in Spans

Current span behavior is intentionally forgiving, but two cases can produce
misleading grouping:
- Time spans mark missing timestamps as `unassigned`.
- Field spans continue the current span when the span field is missing.

Candidate v2 behavior:
- Missing time span timestamps remain `unassigned`, but count as recovered
  errors unless explicitly allowed.
- Missing field span keys should not continue the current span by default.
  Prefer `unassigned`, recovered error, or an explicit `--span-missing=...`
  policy.

Rationale: continuing a field span on missing keys can misattribute events to
the previous field value.

## Keep As Designed

### Exec Errors Roll Back to the Original Event

Do not change resilient `--exec` errors from rollback-and-emit to skip by
default without a separate design decision.

This behavior is intentional: some transformations are best-effort enrichments
that only work on events with certain fields. Emitting the original event keeps
heterogeneous logs usable without requiring every script to guard every field.

Possible future extension:
- Add an explicit policy flag such as `--on-exec-error=original|skip|tag|error`.
- Keep `original` as the resilient default unless user research shows this is
  surprising in practice.

## Auto-Format Detection Scoring

Yes: a scoring system is likely a better direction than the current first-line,
first-success heuristic, but it must not break Kelora's streaming contract.

### Problem

Current auto-detection is deterministic and cheap, but it can overfit one
sample line:
- `line` is always a fallback.
- CSV/TSV can be guessed from delimiter counts.
- A parser that succeeds on one line may not be the best parser for the stream.
- Streaming input cannot require multi-line lookahead without buffering and
  delaying output.

### Proposed Model

Use different detection strategies based on input capabilities.

Regular files:
- Score candidate parsers over a bounded sample window.
- Cap by lines, bytes, and startup time.
- Reuse the sampled bytes/lines for real processing so no input is lost.

Stdin, pipes, and live streams:
- Preserve immediate first-event detection by default.
- Do not block waiting for additional lines.
- Offer sampled detection only as an explicit buffering tradeoff, e.g.
  `--detect=sampled` or `--detect-window=N`.
- Prefer explicit cascade for mixed streams, e.g. `-f json,line`.

Candidate parser results should include success/failure kind, field count,
timestamp presence, level/message fields, schema stability when a sample window
exists, and parse confidence.

Suggested scoring dimensions:
- Parse success rate: strongest signal.
- Structured field richness: more useful fields beats `line`, but only after
  success rate is high.
- Schema stability: CSV/logfmt/syslog candidates get higher confidence when
  fields are consistent across samples.
- Timestamp extraction: valid timestamps increase confidence.
- Level/message recognition: common log fields increase confidence.
- Parser specificity: JSON/CEF/syslog/combined beat generic logfmt/CSV when
  scores are close.
- Lossiness penalties: penalize candidates that drop columns, produce only one
  low-information field, or parse suspiciously broad input.

Example outcome for regular files or explicitly sampled streams:
- `json`: 98% success, rich fields, timestamps present -> choose JSON.
- `logfmt`: 80% success, stable keys -> choose logfmt only if failures look
  like true malformed lines, otherwise suggest cascade.
- `csv`: delimiter-heavy but unstable column count -> avoid auto-selecting CSV.
- `line`: choose only when structured candidates are low confidence.

### Ambiguity Handling

If the top two candidates are close:
- In normal mode, choose the safer candidate and emit a diagnostic when
  diagnostics are enabled.
- With `--strict` or a future `--detect=strict`, fail with a message showing the
  top candidates and ask for `-f`.
- Suggest cascade when mixed structured/plain input is likely, e.g.
  `-f json,line`.
- For streaming first-event detection, ambiguity should prefer the current
  deterministic order and avoid delayed output unless the user opted into
  buffering.

### Implementation Notes

Keep this independent from parser internals:
- Add a `FormatDetector` that owns sampling and scoring.
- Candidate parsers should report lightweight `DetectionResult` metadata.
- Do not let detection mutate parser state used by the real pipeline.
- In `auto-per-file`, score each file independently using the same algorithm.
- Gate sampled scoring on input kind. Regular files can be sampled by default;
  stdin and live streams should stay immediate unless explicitly buffered.

This is a behavior change, but it is probably a net improvement for v2 because
it makes `auto` less magical while preserving convenience and the streaming
default.
