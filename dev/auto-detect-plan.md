# Input Format Auto-Detect Plan (Draft)

Goal: switch the default input format to `auto` with guardrails so new users get the expected behavior while keeping escapes for edge cases.

Planned behavior
- Default `-f` to `auto`.
- Use the existing first-line detector. When it recognizes a format, print `Auto-detected format: <fmt>`.
- If detection fails and we fall back to `line`, print once: `Auto-detect unknown; using line. Use -f <fmt> to force.` (respect quiet/silent/--no-diagnostics/KELORA_NO_TIPS and only on TTY).
- If we chose a non-line format and parsing later fails heavily, emit a short hint suggesting an explicit `-f` (or `-f line`) so users can override.
  - Trigger: only when starting from `-f auto` that resolved to a non-line format. Use merged tracking counters so it works with compressed inputs and parallel mode.
  - Counters: `__kelora_error_count_parse` and `__kelora_stats_events_created` (fall back to 0 when absent).
  - Condition: compute `seen = events_created + parse_errors` (min 1). Fire once at end of run if either:
    - `parse_errors >= 10` **and** `parse_errors * 3 >= seen` (≥75% failures), or
    - `events_created == 0` **and** `parse_errors >= 5` (short runs that all failed).
  - Message: `Many parsing errors; try -f line or -f <fmt>.` Respect diagnostics/TTY suppressions; emit once.

Edge cases and impact
- False positives (we think it’s JSON/CSV/syslog but it isn’t): parsing may error/produce odd fields; follow-up hint nudges users to force `-f line`.
- First line not representative (headers/blanks/rotated fragments): might mis-detect or fail to detect; worst case we fall back to line with the short notice, matching current behavior.
- Behavior change for users relying on default `line`: auto may now pick a structured format; call out in CHANGELOG/help. Users can force `-f line` or set it in config if they want legacy behavior.
- Pipelines/non-TTY or suppressed diagnostics: the notices are suppressed, but auto/fallback still happens.

Open items
- Confirm wording for the fallback notice and the post-parse-error hint.

Testing
- Integration coverage for: auto-detected message on recognized formats; fallback notice on unknown; parse-error hint thresholds; suppression under quiet/silent/no-diagnostics/non-TTY.
