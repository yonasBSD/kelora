# Kelora

<p align="center">
  <img src="kelora-logo.svg" alt="Kelora Logo" width="300">
</p>

Scriptable log processor for the command line. Treats logs as structured events and lets you filter, transform, and analyze them using embedded [Rhai](https://rhai.rs) scripts with 40+ built-in functions.

> [!WARNING]
> Experimental tool. [Vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding). APIs and behaviour may change without notice.

## Overview

Kelora parses log streams into structured events and runs them through a programmable pipeline powered by Rhai scripting.

- Turns lines into structured events you can access as `e.field` or `e["field-name"]`.
- Embeds 40+ built-in Rhai helpers for parsing, enrichment, metrics, and windowed analysis.
- Speaks JSON, logfmt, syslog, CSV/TSV, column specs, and gzip-compressed inputs out of the box.
- Handles streaming or batch workloads with sequential and `--parallel` execution modes.
- Emits metrics and processing stats so you can observe pipelines while they run.

## Quick Examples

```bash
# Parse embedded formats - extract logfmt from within syslog messages
kelora -f syslog examples/simple_syslog.log \
  --exec 'if e.msg.contains("=") { e += e.msg.parse_logfmt() }' \
  --keys timestamp,host,user,action,detail,message \
  -F json

# Keep full stacktraces together with case-insensitive search
kelora examples/multiline_stacktrace.log \
  --multiline timestamp \
  --filter 'e.line.lower().contains("valueerror")' \
  --before-context 1 --after-context 1

# Extract container prefixes, track log volume by source
kelora examples/prefix_docker.log --extract-prefix container \
  --exec 'e.level = e.line.between("[", "]")' \
  --metrics \
  --exec 'track_count(e.container); track_count(e.level)' \
  --keys container,level,line \
  -F csv

# Parse JWT tokens, mask IPs for privacy-safe log sharing
kelora -j examples/security_audit.jsonl \
  --exec 'if e.has_field("token") {
            let jwt = e.token.parse_jwt();
            e.role = jwt.get_path("claims.role", "guest")
          }' \
  --exec 'e.ip = e.ip.mask_ip(2)' \
  --keys timestamp,event,role,ip \
  -F json
```

More quick commands to copy-paste:

- Stream-level error watch: `tail -f examples/simple_json.jsonl | kelora -j --levels warn,error --exec 'track_count(e.service)' --metrics`
- Fan out nested arrays: `kelora -j examples/json_arrays.jsonl --exec 'emit_each(e.get_path(\"users\", []))' --keys id,name,score`
- Visual level distribution: `kelora -f logfmt examples/simple_logfmt.log -F levelmap`

> [!TIP]
> These examples use files in `examples/` — see [examples/README.md](examples/README.md#file-categories) for the full catalog. For a complete walkthrough with annotated output, visit the [Quickstart Guide](https://kelora.dev/latest/quickstart/).

## Installation

Download from **[GitHub Releases](https://github.com/dloss/kelora/releases)** (macOS, Linux, Windows) or:

```bash
cargo install kelora
```

## Documentation

Full documentation is available at **[kelora.dev](https://kelora.dev)**. Some quick links:

- [Quickstart](https://kelora.dev/latest/quickstart/) — 5-minute tour with annotated output
- [How-To Guides](https://kelora.dev/latest/how-to/) — solutions for common tasks
- [Tutorials](https://kelora.dev/latest/tutorials/) — step-by-step guides for building custom pipelines
- [Concepts](https://kelora.dev/latest/concepts/) — deep dive into the streaming pipeline
- [Reference](https://kelora.dev/latest/reference/) — CLI flags, Rhai functions, formats, and configuration

## Highlights

Kelora is built for streaming-first log analysis with a programmable Rhai core.

- **Streaming pipeline** — Parse, filter, transform, and output logs in one flow. Read the [Pipeline Model](https://kelora.dev/latest/concepts/pipeline-model/) for a stage-by-stage breakdown.
- **Built-in Rhai toolbox** — 100+ helpers for enrichment, parsing, time-window analysis, and metrics. Scan the [Functions Reference](https://kelora.dev/latest/reference/functions/) for the full catalog.
- **Format flexibility** — JSON, logfmt, syslog, Apache/Nginx combined, CSV/TSV, column specs, and gzip. See [Input Formats](https://kelora.dev/latest/reference/formats/).
- **Powerful filtering** — Chain `--filter`, `--level`, `--since/--until`, and context flags to zero in on events. Walkthroughs in [Filtering How-To](https://kelora.dev/latest/how-to/find-errors-in-logs/).
- **Span aggregations** — `--span` forms count- or time-based spans and triggers `--span-close` hooks for per-span summaries. See [CLI Reference](https://kelora.dev/latest/reference/cli/#processing-options) for usage patterns.
- **Observability built in** — `--metrics`, `--stats`, and window helpers expose throughput and aggregations for live pipelines. Learn more in [Metrics & Telemetry](https://kelora.dev/latest/concepts/metrics-and-telemetry/).
- **Parallel or streaming** — Stay sequential for tailing or enable `--parallel` for archive crunching. Tuning guidance in [Parallel Processing](https://kelora.dev/latest/how-to/tune-parallel-processing/).

```
Input → Parse → --exec → --filter → --exec → --filter → ... → Output
  ↓       ↓         ↓         ↓         ↓         ↓              ↓
Files   JSON   transform  narrow   enrich    narrow        logfmt
stdin   syslog                                              JSON
.gz     custom                                              CSV
```

## Works Well With

Kelora focuses on normalising noisy logs into structured data. Pipe or export Kelora's output to complementary tools for deeper analysis:

- **[jq](https://jqlang.github.io/jq/)** — process Kelora's JSON output for complex transformations, filtering, or reformatting
- **[lnav](https://lnav.org/)** — explore Kelora's output in an interactive TUI with live filtering, histograms, and ad-hoc SQL queries
- **[qsv](https://github.com/jqnatividad/qsv)** — analyze Kelora's CSV output with statistical operations, joins, and aggregations
- **[SQLite](https://www.sqlite.org/)/[DuckDB](https://duckdb.org/)** — load Kelora's CSV/JSON output into a database for SQL queries and reporting
- **[miller](https://github.com/johnkerl/miller)** — transform Kelora's CSV output for reshaping, aggregating, and format conversion

## License

Kelora is open source software licensed under the [MIT License](https://github.com/dloss/kelora/blob/main/LICENSE).
