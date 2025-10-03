# Kelora

Scriptable log processor for the command line. Treats logs as structured events and lets you filter, transform, and analyze them using embedded [Rhai](https://rhai.rs) scripts with 40+ built-in functions.

> [!WARNING]
> Experimental tool. [Vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding). APIs may change without notice.

## Table of Contents
- [Overview](#overview)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Core Concepts](#core-concepts)
- [CLI Feature Tour](#cli-feature-tour)
- [Parsers & Formats](#parsers--formats)
- [Format Recipes](#format-recipes)
- [Rhai Building Blocks](#rhai-building-blocks)
- [Multiline Strategies](#multiline-strategies)
- [Configuration & Defaults](#configuration--defaults)
- [Example Pipelines](#example-pipelines)
- [Learning Path](#learning-path)
- [Documentation Shortcuts](#documentation-shortcuts)
- [When to Reach for Kelora](#when-to-reach-for-kelora)
- [License](#license)

## Overview

Kelora parses log streams into structured events and runs them through a programmable pipeline powered by Rhai scripting.

- Turns lines into structured events you can access as `e.field` or `e["field-name"]`.
- Embeds 40+ built-in Rhai helpers for parsing, enrichment, metrics, and windowed analysis.
- Speaks JSON, logfmt, syslog, CSV/TSV, column specs, and gzip-compressed inputs out of the box.
- Handles streaming or batch workloads with sequential and `--parallel` execution modes.
- Emits metrics and processing stats so you can observe pipelines while they run.

## Quick Start

> [!TIP]
> Use the fixtures in `example_logs/` when experimenting—no need to point at production logs.

### Basics

```bash
# Filter error-level events from the logfmt sample
kelora -f logfmt -l error example_logs/sample.logfmt

# Work with JSON logs and enrich the event before printing selected fields
kelora -j example_logs/sample.jsonl \
  --filter 'e.level == "ERROR"' \
  --exec 'e.retry_count = e.get_path("retry", 0)' \
  --keys timestamp,level,message,retry_count

# Parse Apache/Nginx access logs, keep key fields, and surface stats
kelora -f combined example_logs/sample.nginx \
  --keys ip,status,request_time,request \
  --stats

# Show errors with surrounding context (like grep -A/-B/-C)
kelora -j example_logs/sample.jsonl \
  --filter 'e.level == "ERROR"' \
  --after-context 2 --before-context 1
```

### Advanced Moves

```bash
# Count slow responses and surface metrics alongside real-time output
kelora -f logfmt example_logs/sample.logfmt \
  --filter 'e.duration.to_int() >= 1000' \
  --exec 'track_count("slow_requests"); e.bucket = if e.duration.to_int() >= 2000 { "very_slow" } else { "slow" }' \
  --metrics

# Sliding window alerting for login failures (stream from any source)
kubectl logs -f deploy/auth | \
  kelora -j --window 5 \
    --filter 'e.event == "login_failed"' \
    --exec 'let users = window_values("user"); if users.len() >= 3 { eprint("🚨 brute force detected for " + e.user); }'

# Run a scripted pipeline from disk (save your Rhai to pipelines/critical_filter.rhai first)
kelora -j example_logs/sample.jsonl \
  --begin 'conf.critical_components = ["database", "auth"]' \
  --exec-file pipelines/critical_filter.rhai \
  --output-file filtered.json
```

## Installation

Kelora targets stable Rust; keep your toolchain fresh (`rustup update`) for best results.

```bash
# Install from crates.io
cargo install kelora

# Build from source
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```

Pre-built binaries live in the [GitHub releases](https://github.com/dloss/kelora/releases). They're handy for CI or containers.

## Core Concepts

- **Events** - Every log line becomes a structured map. Fields are accessible as `e.level`, `e["user-agent"]`, or via helper functions like `e.get_path("payload.id")`.
- **Pipeline** - Logs flow through `Input -> Parse -> Filter -> Transform -> Format -> Output`. Mix and match parsers, filters, exec scripts, and formatters freely.
- **Scripts** - Rhai expressions drive filtering (`--filter`), transformations (`--exec`/`--exec-file`), initialization (`--begin`), and teardown (`--end`).
- **Windows** - `--window N` exposes recent events for sequence detection via helpers like `window_values("field")`.

## CLI Feature Tour

### Input & Parsing

- `-f, --input-format <FORMAT>` selects a parser (`json`, `logfmt`, `syslog`, `combined`, `cols:<spec>`, ...); `-j` is shorthand for JSON.
- `--file-order {cli|name|mtime}` controls multi-file processing order.
- `--skip-lines`, `--keep-lines`, and `--ignore-lines` filter raw input before parsing.
- `-M, --multiline <strategy>` enables multi-line reconstruction (see [Multiline Strategies](#multiline-strategies)).
- `--extract-prefix FIELD` + `--prefix-sep SEP` slices service prefixes before parsing.
- `--cols-sep SEP` provides a custom delimiter when using column specs.

### Filtering & Selection

- `--filter 'expression'` runs boolean Rhai expressions; chain multiple occurrences.
- `-l/--levels` and `-L/--exclude-levels` gate standard log levels.
- `--since`, `--until`, and `--take` trim by time range or limit output volume.
- `-A/--after-context`, `-B/--before-context`, and `-C/--context` show surrounding lines around matches (requires filtering).
- `--keep-lines`/`--ignore-lines` pair well with `--strict` to enforce hygiene.
- `--allow-fs-writes` enables Rhai file helpers (`mkdir`, `truncate_file`, `append_file`) so scripts can shard or persist results; without it these functions return `false` and perform no I/O.

### Transformations & State

- `-e/--exec` and `-E/--exec-file` mutate events (`e`), emit new ones (`emit_each`), or track metrics.
- `--begin` seeds global read-only configuration through the `conf` map; `--end` performs final aggregation or reporting.
- `--window N` exposes `window` helpers for sliding analyses; combine with `window_values(...)`, `window_numbers(...)`, or custom loops.
- Default mode is resilient—errors skip the offending event. Use `--strict` for fail-fast pipelines.
- Dial verbosity with `-v` / `-vvv` for debugging or `-q` / `-qqq` for quiet pipelines.
- `-I/--include` prepends Rhai files to define reusable functions for `--exec`, `--begin`, and `--end` stages.

### Output & Reporting

- `-F/--output-format` controls formatting (`default`, `json`, `logfmt`, `inspect`, `levelmap`, CSV/TSV variants, or `none`). `-J` is shorthand for JSON.
- `-k/--keys` and `-K/--exclude-keys` shape the output payload; `-c` keeps only core fields; `-b` switches to brief mode.
- `--convert-ts` converts timestamp fields to RFC3339 format (modifies event data); `-z/--show-ts-local` and `-Z/--show-ts-utc` format display timestamps (default formatter only).
- `--stats`, `--metrics`, and `-S/--stats-only` expose processing telemetry; `--metrics-file` writes JSON metrics to disk.
- `--mark-gaps` inserts visual separators when time jumps exceed a duration; `--no-emoji` disables emoji prefixes.
- Context lines are marked with visual prefixes: `*` for matches, `/` for before-context, `\` for after-context.

### Performance & Reliability

- `--parallel` and `--threads` control concurrency; pair with `--batch-size` (lines per worker batch, default 1000) and `--batch-timeout` (max ms to flush a partial batch, default 200) to balance parallel throughput and latency.
- `--unordered` relaxes output ordering for faster parallel flushes.
- Sequential mode (default) shines for streaming sources; `--parallel` excels at log archives.
- Combine with `--config-file` or aliases for repeatable pipelines at scale.

## Parsers & Formats

Kelora defaults to `-f line`, which trims trailing newline/CR characters and exposes the result as `e.line`. Reach for `-f raw` when you need a byte-for-byte copy (including trailing newlines or escape markers), and reserve `-f 'cols:<spec>'` for bespoke formats that the built-in parsers do not cover. All parsers expect UTF-8 text; binary or other encodings will raise input errors.

| Format | Fields Produced | Typical Source |
| --- | --- | --- |
| `line` (default) | `line` | Newline-delimited text where trimming the trailing newline is acceptable |
| `raw` | `raw` | Exact text preservation (newline-sensitive data, continuation markers, binary artifacts) |
| `json` | Original JSON keys | JSONL or JSON arrays |
| `logfmt` | Key-value pairs | Logfmt structured logs |
| `syslog` | `timestamp`, `host`, `facility`, `message`, ... | RFC3164/RFC5424 syslog |
| `cef` | Header fields + extension map | ArcSight/Common Event Format |
| `csv` / `tsv` | Column headers as fields | Delimited datasets |
| `combined` | `ip`, `status`, `method`, `path`, `request`, `request_time`, ... | Apache/Nginx access logs |
| `cols:<spec>` | Named fields defined by your spec (`ts`, `level`, `*rest`, ...) | Custom or proprietary log formats |

All parsers auto-detect gzip compression (files and stdin) by magic bytes—no extra flags required.

## Format Recipes

### Raw vs Line

Choose the right baseline for text pre-processing. `-f line` is the default: it trims the trailing newline/CR and gives you a tidy `e.line` field for downstream filters. `-f raw` keeps every byte (including trailing delimiters and escape markers) in `e.raw`, which is invaluable when you need to preserve continuation characters, feed the data into another parser verbatim, or re-emit the original payload.

```bash
# Preserve every byte (newline-sensitive analyses)
kelora -f raw example_logs/sample.log \
  --exec 'e.byte_len = e.raw.len()'

# Treat each line as plain text for simple filtering
kelora -f line example_logs/sample.log \
  --filter 'e.line.contains("ERROR")'
```

### Prefix Extraction

Strip infrastructure prefixes before parsing structured payloads.

```bash
docker compose logs | \
  kelora --extract-prefix container --prefix-sep " | " --filter 'e.container == "web_1"'
```

### Column Specs (`cols:<spec>`)

Declarative column parsing with skips, joins, and tail captures. This mode shines when your data has a repeatable column layout but no dedicated parser—think bespoke appliance logs, legacy flat files, or regex capture groups you want to map into fields.

Spec tokens are space-separated:

- `field` — consume one column into `field`.
- `field(n)` — consume `n ≥ 2` columns, joined together (whitespace joins in default mode, literal separator joins when you set one).
- `-` / `-(n)` — skip one or `n` columns with no output field.
- `*field` — capture the remaining text verbatim; must appear once and always at the end.

Whitespace is the default separator; add `--cols-sep "|"` (or another literal) when your columns are delimited. You can also feed pre-split arrays to `parse_cols` from Rhai (`let caps = e.line.extract_all_re(...); caps.parse_cols("ip user ts *msg");`). Missing data fills fields with `()` in resilient mode, while `--strict` turns shortages/extras into errors.

```bash
echo "2025-09-22 12:33:44 INFO alice login success" | \
  kelora -f 'cols:date time level user *message' \
    --exec 'e.timestamp = (e.date + " " + e.time).to_datetime()' \
    --keys timestamp,level,user,message
```

## Rhai Building Blocks

Kelora exposes the full Rhai language plus domain-specific helpers.

- **Text & parsing** - `extract_re`, `parse_logfmt`, `parse_cols`, `mask_ip`, `encode_*` / `decode_*`.
- **Arrays & maps** - `sorted`, `sorted_by`, `array.flatten`, `map.get_path`, `map.flatten`, `emit_each`.
- **Hashing & anonymization** - `bucket` (fast sampling), `hash` (multi-algo), `anonymize` (salted SHA-256), `pseudonym` (short IDs).
- **Metrics** - `track_count`, `track_avg`, `track_bucket`, `track_unique` power `--metrics` and `--end` reports.
- **Datetime** - `to_datetime`, `to_duration`, `now_utc`, formatting helpers, arithmetic.
- **Environment & control** - `get_env`, `read_file`, `read_lines`, `exit`.

Example pipeline with shared configuration and sliding logic:

```bash
kelora -j example_logs/sample.jsonl \
  --begin 'conf.error_levels = ["ERROR", "FATAL"]; conf.retry_threshold = 2' \
  --filter 'conf.error_levels.contains(e.level)' \
  --exec 'if e.get_path("retry", 0) >= conf.retry_threshold { track_count("retries"); }' \
  --window 1 \
  --exec 'let comps = window_values("component"); if comps.len() > 1 && comps[0] == comps[1] { e.context = "repeat_component"; }' \
  --metrics
```

See `kelora --help-rhai` for syntax essentials and `kelora --help-functions` for the complete catalog.

## Multiline Strategies

| Preset | Use When | Equivalent |
| --- | --- | --- |
| `stacktrace` | ISO or syslog timestamps leading each entry | `-M timestamp` |
| `docker` | Docker JSON logs (RFC3339 timestamps) | `-M timestamp:pattern=^\\d{4}-\\d{2}-\\d{2}T` |
| `syslog` | RFC3164/5424 headers (`Jan  2`, `<34>1 2024-01-01T...`) | `-M timestamp:pattern=^(<\\d+>\\d\\s+\\d{4}-\\d{2}-\\d{2}T|\\w{3}\\s+\\d{1,2})` |
| `combined` | Apache/Nginx access logs with remote host prefix | `-M start:^\\S+\\s+\\S+\\s+\\S+\\s+\\[` |
| `nginx` | Nginx error logs prefixed with `[dd/Mon/yyyy:` | `-M timestamp:pattern=^\\[[0-9]{2}/[A-Za-z]{3}/[0-9]{4}:` |
| `continuation` | Lines ending with `\\` continue the current event | `-M backslash` |
| `block` | `BEGIN` ... `END` sections form a single event | `-M boundary:start=^BEGIN:end=^END` |
| `whole` | Treat the entire input as one event (fixtures, preformatted payloads) | `-M whole` |

When you pick a timestamp-based strategy (the presets above or `-M timestamp`), you can also
provide `--ts-format=<chrono fmt>` so Kelora matches your exact timestamp prefix instead of relying
solely on the preset regex.

Build custom strategies with `timestamp:pattern=...`, `indent`, `start:REGEX`, `end:REGEX`, `boundary`, `backslash[:char=...]`, or `whole` (single mega-event). Remember that buffering happens until a boundary is found; when running with `--parallel`, lower `--batch-size` or `--batch-timeout` to keep long multi-line frames flushable, and `whole` will buffer the entire stream in memory.

## Configuration & Defaults

Define repeatable pipelines in `~/.config/kelora/config.ini`:

```ini
# Defaults applied to every run
defaults = --stats --parallel --input-tz UTC

[aliases]
errors = -l error --since 1h --stats
warnings = --filter 'e.level == "WARN" || e.level == "WARNING"'
slow-queries = --filter 'e.duration > 1000' --exec 'e.slow = true' --keys timestamp,query,duration
```

Usage:

```bash
kelora example_logs/sample.jsonl            # Uses defaults
kelora --config-file custom.ini example_logs/sample.log  # Swap configuration files
kelora --no-stats example_logs/sample.log                # Override a default
kelora -a errors example_logs/sample.jsonl  # Run the alias
kelora --show-config                        # Inspect the merged configuration
kelora -l error --stats --save-alias errors # Save current command as alias
```

Pair configs with `--ignore-config` for hermetic runs or CI pipelines.

## Example Pipelines

```bash
# Real-time nginx monitoring with enrichment, metrics, and alerting
tail -f /var/log/nginx/access.log | \
  kelora -f combined \
    --exec 'let status = e.status.to_int(); e.class = if status >= 500 { "server_error" } else if status >= 400 { "client_error" } else { "ok" };' \
    --filter 'e.class != "ok"' \
    --exec 'track_count("errors"); if e.class == "server_error" { eprint("🚨 " + e.status + " " + e.request); }' \
    --metrics

# Authentication watch with sliding windows and unique counters
kelora -f syslog example_logs/sample.syslog \
  --filter 'e.message.contains("Failed login")' \
  --window 4 \
  --exec 'let count = 0; for msg in window_values("message") { if msg.contains("Failed login") { count += 1; } } if count >= 3 { eprint("🚨 repeated failures: " + e.message.extract_ip()); track_unique("alert_ips", e.message.extract_ip()); }' \
  --metrics

# Convert syslog to structured JSON and redact sensitive fields
kelora -f syslog example_logs/sample.syslog \
  --exec 'e.severity_label = if e.severity <= 3 { "critical" } else if e.severity <= 4 { "error" } else { "info" }; e.host = e.host.mask_ip(1);' \
  -J

# Anonymize PII while maintaining linkability and sampling
kelora -j access.log --salt "$KELORA_SALT" \
  --exec 'e.user_pseudo = pseudonym(e.user_id, 8); e.ip_anon = anonymize(e.ip)' \
  --filter 'bucket(e.user_id) % 10 == 0' \
  --keys user_pseudo,ip_anon,action,status
```

## Learning Path

1. **Events** - Practice accessing and mutating `e.field` values on JSON or logfmt samples.
2. **Parsing** - Try multiple formats (`-f json`, `-f combined`, `-f 'cols:...'`) and experiment with `--extract-prefix`.
3. **Basic Scripts** - Layer `--filter`, `--exec`, and `--keys` for simple transformations.
4. **Metrics & Stats** - Introduce `track_count`, `track_avg`, `--metrics`, and `--stats`.
5. **Pipelines** - Chain multiple `--filter`/`--exec` stages, add `--begin` configuration, and export results.
6. **Output Control** - Switch between `-F` modes, apply `-k`/`-K`, and format timestamps.
7. **Windows** - Explore `--window`, `window_values`, and `window_numbers` for sequence detection.
8. **Multi-stage Workloads** - Combine `--parallel`, config aliases, and `--end` summarization for production-style jobs.

Each milestone builds on the previous one; you can be productive early, then layer in advanced concepts as needed.

## Documentation Shortcuts

```bash
kelora --help            # CLI overview, option catalog, examples
kelora --help-rhai       # Rhai syntax and scripting patterns
kelora --help-functions  # Built-in functions grouped by domain
kelora --help-multiline  # Multiline strategy reference
kelora --help-time       # Timestamp parsing and formatting guide
```

## When to Reach for Kelora

- **Use Kelora when** you need programmable filtering, enrichment, aggregation, or custom metrics directly in the terminal.
- **Pair with other tools**: pipe results into `jq`, `ripgrep`, or `lnav`—Kelora focuses on transformation, not visualization.
- **Prefer other tools when** you need interactive browsing (`lnav`), raw text search (`ripgrep`), heavy-duty JSON querying (`jq`), dashboards (`Grafana`), or centralized log shipping (`Fluentd`).

See also: [angle-grinder](https://github.com/rcoh/angle-grinder), [pq](https://github.com/iximiuz/pq), [Miller](https://github.com/johnkerl/miller).

## License

[MIT](LICENSE) - see the license file for full details.
