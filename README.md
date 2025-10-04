# Kelora

<p align="center">
  <img src="kelora-logo.svg" alt="Kelora Logo" width="300">
</p>

Scriptable log processor for the command line. Treats logs as structured events and lets you filter, transform, and analyze them using embedded [Rhai](https://rhai.rs) scripts with 40+ built-in functions.

> [!WARNING]
> Experimental tool. [Vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding). APIs may change without notice.

## Table of Contents
- [Overview](#overview)
- [Getting Started](#getting-started)
  - [First Commands](#first-commands)
  - [Quick Reference](#quick-reference)
  - [Installation](#installation)
- [Everyday Use](#everyday-use)
  - [Core Concepts](#core-concepts)
  - [CLI Feature Tour](#cli-feature-tour)
  - [Example Pipelines](#example-pipelines)
- [Deep Dives](#deep-dives)
  - [Parsers & Formats](#parsers--formats)
  - [Rhai Building Blocks](#rhai-building-blocks)
  - [Configuration & Defaults](#configuration--defaults)
  - [Troubleshooting](#troubleshooting)
- [Reference](#reference)
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

## Getting Started

### First Commands

```bash
# Filter error-level events from the logfmt sample
kelora -f logfmt --level error examples/simple_logfmt.log

# Focus on database events and surface slow queries
kelora -j examples/simple_json.jsonl \
  --filter 'e.service == "database"' \
  --exec 'e.duration_s = e.get_path("duration_ms", 0) / 1000' \
  --keys timestamp,message,duration_s

# Parse Apache/Nginx access logs, keep key fields, and surface stats
kelora -f combined examples/web_access_large.log.gz \
  --keys ip,status,request_time,request \
  --stats

# Show login-related events with surrounding context (like grep -A/-B/-C)
kelora -j examples/simple_json.jsonl \
  --filter 'e.message.contains("login")' \
  --after-context 2 --before-context 1
```

### Quick Reference

#### Pipeline at a Glance

| Stage | Key switches | Purpose |
| --- | --- | --- |
| Input | `kelora [FILES]`, `--parallel`, `--file-order` | Select sources and decide sequential vs. parallel ingestion |
| Parse | `-f/--input-format`, `--extract-prefix`, `-M/--multiline` | Turn raw text into structured events |
| Filter | `--filter`, `--level`, `--since/--until` | Keep only the events you care about |
| Transform | `-e/--exec`, `--begin`, `--window` | Enrich, fan out, and compute stateful metrics |
| Format | `-F/--output-format`, `-k/--keys`, `--stats` | Control output shape and statistics |

#### Workload Recipes

- Streaming tail: `tail -f examples/simple_json.jsonl | kelora -f json --filter 'e.level != "DEBUG"' --stats`
- Archive crunching: `kelora -f combined --parallel examples/web_access_large.log.gz --stats`
- Focused drill-down: `kelora -j examples/simple_json.jsonl --filter 'e.service == "auth"' -k timestamp,message`

> [!TIP]
> The fixtures in `examples/` map to the categories in [examples/README.md](examples/README.md#file-categories). Start there before pointing Kelora at production data. Need a fast reminder of the core flags? Run `kelora --help-quick`.



### Installation

#### Binary releases (recommended)

1. Download the archive for your platform from the [GitHub releases](https://github.com/dloss/kelora/releases) page (macOS, Linux, and Windows builds are provided).
2. Unpack the archive and move `kelora` or `kelora.exe` somewhere on your `PATH`.
3. Run `kelora --help` to ensure the binary starts up correctly.

If you prefer to manage the build yourself, Kelora targets stable Rust; keep your toolchain fresh (`rustup update`) for best results.

```bash
# Install from crates.io
cargo install kelora

# Build from source
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```





## Everyday Use

### Core Concepts

- **Events** - Every log line becomes a structured map. Fields are accessible as `e.level`, `e["user-agent"]`, or via helper functions like `e.get_path("payload.id")`.
- **Pipeline** - Logs flow through `Input -> Parse -> Filter -> Transform -> Format -> Output`. Mix and match parsers, filters, exec scripts, and formatters freely.
- **Scripts** - Rhai expressions drive filtering (`--filter`), transformations (`--exec`/`--exec-file`), initialization (`--begin`), and teardown (`--end`).
- **Windows** - `--window N` exposes recent events for sequence detection via helpers like `window_values("field")`.

### CLI Feature Tour

#### Input & Parsing

- `-f, --input-format <FORMAT>` selects a parser (`json`, `logfmt`, `syslog`, `combined`, `cols:<spec>`, ...); `-j` is shorthand for JSON.
- `--file-order {cli|name|mtime}` controls multi-file processing order.
- `--skip-lines`, `--keep-lines`, and `--ignore-lines` filter raw input before parsing.
- `-M, --multiline <strategy>` enables multi-line reconstruction (see [Multiline Strategies](#multiline-strategies)).
- `--extract-prefix FIELD` + `--prefix-sep SEP` slices service prefixes before parsing.
- `--cols-sep SEP` provides a custom delimiter when using column specs.

#### Filtering & Selection

- `--filter 'expression'` runs boolean Rhai expressions; chain multiple occurrences.
- `--level` and `--exclude-levels` gate standard log levels.
- `--since`, `--until`, and `--take` trim by time range or limit output volume.
- `-A/--after-context`, `-B/--before-context`, and `-C/--context` show surrounding lines around matches (requires filtering).
- `--keep-lines`/`--ignore-lines` pair well with `--strict` to enforce hygiene.
- `--allow-fs-writes` enables Rhai file helpers (`mkdir`, `truncate_file`, `append_file`) so scripts can shard or persist results; without it these functions return `false` and perform no I/O.

#### Transformations & State

- `-e/--exec` and `-E/--exec-file` mutate events (`e`), emit new ones (`emit_each`), or track metrics.
- `--begin` seeds global read-only configuration through the `conf` map; `--end` performs final aggregation or reporting.
- `--window N` exposes `window` helpers for sliding analyses; combine with `window_values(...)`, `window_numbers(...)`, or custom loops.
- Default mode is resilient—errors skip the offending event. Use `--strict` for fail-fast pipelines.
- Dial verbosity with `-v` / `-vvv` for debugging or `-q` / `-qqq` for quiet pipelines.
- `-I/--include` prepends Rhai files to define reusable functions for `--exec`, `--begin`, and `--end` stages.

#### Output & Reporting

- `-F/--output-format` controls formatting (`default`, `json`, `logfmt`, `inspect`, `levelmap`, CSV/TSV variants, or `none`). `-J` is shorthand for JSON.
- `-k/--keys` and `-K/--exclude-keys` shape the output payload; `-c` keeps only core fields; `-b` switches to brief mode.
- `--convert-ts` converts timestamp fields to RFC3339 format (modifies event data); `-z/--show-ts-local` and `-Z/--show-ts-utc` format display timestamps (default formatter only).
- `--stats`, `--metrics`, and `-S/--stats-only` expose processing telemetry; `--metrics-file` writes JSON metrics to disk.
- `--mark-gaps` inserts visual separators when time jumps exceed a duration; `--no-emoji` disables emoji prefixes.
- Context lines are marked with visual prefixes: `*` for matches, `/` for before-context, `\` for after-context.

#### Performance & Reliability

- `--parallel` and `--threads` control concurrency; pair with `--batch-size` (lines per worker batch, default 1000) and `--batch-timeout` (max ms to flush a partial batch, default 200) to balance parallel throughput and latency.
- `--unordered` relaxes output ordering for faster parallel flushes.
- Sequential mode (default) shines for streaming sources; `--parallel` excels at log archives.
- Combine with `--config-file` or aliases for repeatable pipelines at scale.

## Deep Dives

### Parsers & Formats

Kelora defaults to `-f line`, which trims trailing newline/CR characters and exposes the result as `e.line`. Reach for `-f raw` when you need a byte-for-byte copy (including trailing newlines or escape markers), and reserve `-f 'cols:<spec>'` for bespoke formats that the built-in parsers do not cover. All parsers expect UTF-8 text; binary or other encodings will raise input errors.

| Format | Fields Produced | Typical Source |
| --- | --- | --- |
| `line` (default) | `line` | Newline-delimited text where trimming the trailing newline is acceptable |
| `raw` | `raw` | Exact text preservation (newline-sensitive data, continuation markers, binary artifacts) |
| `json` | Original JSON keys | JSONL or JSON arrays |
| `logfmt` | Key-value pairs | Logfmt structured logs |
| `syslog` | `timestamp`, `host`, `facility`, `message`, ... | RFC3164/RFC5424 syslog |
| `cef` | Header fields + extension map | ArcSight/Common Event Format |
| `csv` / `tsv` | Column headers as fields (strings by default) | Delimited datasets |
| `csv:<spec>` / `tsv:<spec>` | Column headers with type conversions | Typed CSV/TSV data |
| `combined` | `ip`, `status`, `method`, `path`, `request`, `request_time`, ... | Apache/Nginx access logs |
| `cols:<spec>` | Named fields defined by your spec (`ts`, `level`, `*rest`, ...) | Custom or proprietary log formats |

All parsers auto-detect gzip compression (files and stdin) by magic bytes—no extra flags required.

#### Format Recipes

##### Raw vs Line

Choose the right baseline for text pre-processing. `-f line` is the default: it trims the trailing newline/CR and gives you a tidy `e.line` field for downstream filters. `-f raw` keeps every byte (including trailing delimiters and escape markers) in `e.raw`, which is invaluable when you need to preserve continuation characters, feed the data into another parser verbatim, or re-emit the original payload.

```bash
# Preserve every byte (newline-sensitive analyses)
kelora -f raw examples/simple_line.log \
  --exec 'e.byte_len = e.raw.len()'

# Treat each line as plain text for simple filtering
kelora -f line examples/simple_line.log \
  --filter 'e.line.contains("ERROR")'
```

##### Prefix Extraction

Strip infrastructure prefixes before parsing structured payloads.

```bash
cat examples/prefix_docker.log | \
  kelora --extract-prefix container --prefix-sep " | " --filter 'e.container == "web_1"'
```

##### Type Annotations

Type annotations allow you to convert string fields to specific types during parsing. Supported for CSV, TSV, and cols formats.

```bash
# CSV with typed columns (status, bytes, and duration_ms as integers)
kelora -f "csv status:int bytes:int duration_ms:int" examples/simple_csv.csv

# TSV with type conversions - space-separated field specs
kelora -f "tsv: user_id:int success:bool" examples/simple_tsv.tsv

# Cols format with multi-column capture
kelora -f "cols:email ts status ip" examples/cols_mixed.log

# Cols with count specifiers and custom separator
kelora -f "cols:date(2) level *msg:string" examples/cols_fixed.log

# Strict mode: fail on conversion errors instead of falling back to strings
kelora -f "csv status:int" --strict examples/errors_csv_ragged.csv

# Works in pipes and with compressed data
gzip -dc examples/web_access_large.log.gz | kelora -f combined --stats
```

**Supported types:**
- `int` - Convert to 64-bit integer
- `float` - Convert to 64-bit float
- `bool` - Convert to boolean (recognizes `true/false`, `yes/no`, `1/0`, case-insensitive)
- (no annotation) - Keep as string (default)

**Error handling:**
- Resilient mode (default): Invalid conversions fall back to original string value
- Strict mode (`--strict`): Invalid conversions abort processing with error

**Format availability:**
- CSV/TSV: Type annotations work with headers (`csv`, `tsv`). Headerless formats (`csvnh`, `tsvnh`) use auto-generated column names (c1, c2, ...) and don't support type annotations.
- Cols: Type annotations can be embedded directly in the column spec (e.g., `status:int`, `ts(2):string`, `*msg:bool`)

##### Column Specs (`cols:<spec>`)

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

### Rhai Building Blocks

Kelora exposes the full Rhai language plus domain-specific helpers.

- **Text & parsing** - `extract_re`, `parse_logfmt`, `parse_cols`, `mask_ip`, `encode_*` / `decode_*`.
- **Arrays & maps** - `sorted`, `sorted_by`, `array.flatten`, `map.get_path`, `map.flatten`, `emit_each`.
- **Hashing & anonymization** - `bucket` (fast sampling), `hash` (multi-algo), `anonymize` (salted SHA-256), `pseudonym` (short IDs).
- **Metrics** - `track_count`, `track_avg`, `track_bucket`, `track_unique` power `--metrics` and `--end` reports.
- **Datetime** - `to_datetime`, `to_duration`, `now_utc`, formatting helpers, arithmetic.
- **Environment & control** - `get_env`, `read_file`, `read_lines`, `exit`.

Example pipeline with shared configuration and sliding logic:

```bash
kelora -j examples/simple_json.jsonl \
  --begin 'conf.error_levels = ["ERROR", "FATAL"]; conf.retry_threshold = 2' \
  --filter 'conf.error_levels.contains(e.level)' \
  --exec 'if e.get_path("retry", 0) >= conf.retry_threshold { track_count("retries"); }' \
  --window 1 \
  --exec 'let comps = window_values("component"); if comps.len() > 1 && comps[0] == comps[1] { e.context = "repeat_component"; }' \
  --metrics
```

See `kelora --help-rhai` for syntax essentials and `kelora --help-functions` for the complete catalog.

### Multiline Strategies

Kelora now offers four explicit multiline modes:

- `timestamp` — detect leading timestamps using the adaptive parser. Add
  `:format=<chrono>` when you need to seed a specific layout (`%b %e %H:%M:%S`, etc.).
- `indent` — treat any line that begins with indentation as a continuation of the
  current event.
- `regex:match=<REGEX>[:end=<REGEX>]` — provide your own record headers (and
  optional terminators) when you need full control.
- `all` — buffer the entire input as a single event when you already have
  pre-chunked payloads.

The option stays off unless you pass `-M/--multiline`. Detection always runs before
parsing, so pair the strategy with the input format you expect (for example, `-f raw`
before handing events to a JSON parser). Buffering continues until the next detected
start (or end regex) arrives; if you run with `--parallel`, tune `--batch-size` or
`--batch-timeout` to keep memory bounded. Remember that `--multiline all` keeps the
entire stream in memory until it flushes.

### Configuration & Defaults

Define repeatable pipelines in `~/.config/kelora/config.ini`:

```ini
# Defaults applied to every run
defaults = --stats --parallel --input-tz UTC

[aliases]
errors = --level error --since 1h --stats
warnings = --filter 'e.level == "WARN" || e.level == "WARNING"'
slow-queries = --filter 'e.duration > 1000' --exec 'e.slow = true' --keys timestamp,query,duration
```

Usage:

```bash
kelora examples/simple_json.jsonl            # Uses defaults
kelora --config-file custom.ini examples/simple_line.log  # Swap configuration files
kelora --no-stats examples/simple_line.log                # Override a default
kelora -a errors examples/simple_json.jsonl  # Run the alias
kelora --show-config                        # Inspect the merged configuration
kelora --level error --stats --save-alias errors # Save current command as alias
```

Pair configs with `--ignore-config` for hermetic runs or CI pipelines.

### Example Pipelines

```bash
# Monitor access logs for server/client errors and count them
kelora -f combined examples/web_access_large.log.gz \
  --exec 'let status = e.status.to_int(); e.class = if status >= 500 { "server_error" } else if status >= 400 { "client_error" } else { "ok" };' \
  --filter 'e.class != "ok"' \
  --exec 'track_count("errors"); if e.class == "server_error" { eprint("🚨 " + e.status + " " + e.request); }' \
  --metrics

# Authentication watch with sliding windows and unique counters
kelora -f syslog examples/simple_syslog.log \
  --filter '"msg" in e && e.msg.contains("Failed login")' \
  --window 4 \
  --exec 'let count = 0; for msg in window_values("msg") { if msg.contains("Failed login") { count += 1; } } if count >= 3 { eprint("🚨 repeated failures: " + e.msg.extract_ip()); track_unique("alert_ips", e.msg.extract_ip()); }' \
  --metrics

# Convert syslog to structured JSON and redact sensitive fields
kelora -f syslog examples/simple_syslog.log \
  --exec 'e.severity_label = if e.severity <= 3 { "critical" } else if e.severity <= 4 { "error" } else { "info" }; e.host = e.host.mask_ip(1);' \
  -J

# Anonymize sensitive fields while keeping sessions linkable
kelora -j examples/security_audit.jsonl \
  --exec 'e.user_hash = e.user.hash("xxh3"); e.ip_masked = e.ip.mask_ip(1)' \
  --filter 'e.event == "login"' \
  --keys timestamp,user_hash,ip_masked,event
```

### Advanced Pipelines

```bash
# Track slow requests and bucket them by severity
kelora -f logfmt examples/simple_logfmt.log \
  --filter '"duration" in e && e.duration.to_int_or(0) >= 100' \
  --exec 'track_count("slow_requests"); e.slow_bucket = if e.duration.to_int_or(0) >= 2000 { "very_slow" } else { "slow" }' \
  --metrics

# Expand nested arrays into individual events
kelora -j examples/json_arrays.jsonl \
  --exec 'emit_each(e.get_path("users", []))' \
  --keys id,name,score

# Rolling average over windowed metrics
kelora -j examples/window_metrics.jsonl \
  --window 5 \
  --metrics \
  --exec 'let values = window_numbers("value"); if values.len() == 5 { let total = 0.0; for v in values { total += v; } e.moving_avg = total / values.len(); }'
```

### Troubleshooting

- **No events printed**: run with `--verbose` to surface Rhai errors, or temporarily drop filters (`--filter`) to confirm parsing succeeds.
- **Timestamp parsing failures**: confirm the field name via `-F json`, then add `--ts-field`/`--ts-format` from [`kelora --help-time`](#documentation-shortcuts). In resilient mode failed timestamps are dropped silently; add `--strict` to fail fast.
- **Rhai script panics**: wrap risky lookups with helpers like `e.get_path("field", ())` or use `to_int_or` to coerce strings safely. `kelora --help-rhai` documents the available guards.
- **Performance dips with `--parallel`**: trim `--window` sizes, avoid heavy per-event printing, and tune `--batch-size`/`--batch-timeout` as described in [Performance & Reliability](#cli-feature-tour).
- **Unexpected empty fields**: inspect raw input using `-F inspect` or `-F logfmt` to ensure the parser chosen in `-f/--input-format` matches the data.

### Learning Path

1. **Events** - Practice accessing and mutating `e.field` values on JSON or logfmt samples.
2. **Parsing** - Try multiple formats (`-f json`, `-f combined`, `-f 'cols:...'`) and experiment with `--extract-prefix`.
3. **Basic Scripts** - Layer `--filter`, `--exec`, and `--keys` for simple transformations.
4. **Metrics & Stats** - Introduce `track_count`, `track_avg`, `--metrics`, and `--stats`.
5. **Pipelines** - Chain multiple `--filter`/`--exec` stages, add `--begin` configuration, and export results.
6. **Output Control** - Switch between `-F` modes, apply `-k`/`-K`, and format timestamps.
7. **Windows** - Explore `--window`, `window_values`, and `window_numbers` for sequence detection.
8. **Multi-stage Workloads** - Combine `--parallel`, config aliases, and `--end` summarization for production-style jobs.

Each milestone builds on the previous one; you can be productive early, then layer in advanced concepts as needed.

> [!TIP]
> See [examples/README.md](examples/README.md) for 37 working examples covering all formats and complexity levels—from basic filtering to advanced pipelines.

## Reference

### Documentation Shortcuts

```bash
kelora --help-quick      # One-screen cheat sheet of common flags and recipes
kelora --help            # Full CLI reference grouped by stage
kelora --help-rhai       # Rhai syntax and scripting patterns
kelora --help-functions  # Built-in functions grouped by domain
kelora --help-multiline  # Multiline strategy reference
kelora --help-time       # Timestamp parsing and formatting guide
https://github.com/dloss/kelora/blob/main/docs/COOKBOOK.md     # Cookbook of Rhai and pipeline patterns
https://github.com/dloss/kelora/blob/main/examples/README.md   # Catalogue of sample log files
```

### When to Reach for Kelora

- **Use Kelora when** you need programmable filtering, enrichment, aggregation, or custom metrics directly in the terminal.
- **Pair with other tools**: pipe results into `jq`, `ripgrep`, or `lnav`—Kelora focuses on transformation, not visualization.
- **Prefer other tools when** you need interactive browsing (`lnav`), raw text search (`ripgrep`), heavy-duty JSON querying (`jq`), dashboards (`Grafana`), or centralized log shipping (`Fluentd`).

See also: [angle-grinder](https://github.com/rcoh/angle-grinder), [pq](https://github.com/iximiuz/pq), [Miller](https://github.com/johnkerl/miller).

### License

[MIT](LICENSE) - see the license file for full details.
