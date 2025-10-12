# Kelora

<p align="center">
  <img src="kelora-logo.svg" alt="Kelora Logo" width="300">
</p>

Scriptable log processor for the command line. Treats logs as structured events and lets you filter, transform, and analyze them using embedded [Rhai](https://rhai.rs) scripts with 40+ built-in functions.

> [!WARNING]
> Experimental tool. [Vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding). APIs and behaviour may change without notice.
>
> [!TIP]
> Looking for the full docs? Head to [kelora.dev](https://kelora.dev) for the complete quickstart, conceptual guides, tutorials, and references. Use this README for a fast overview before diving deeper.

## Table of Contents
- [Overview](#overview)
- [Documentation](#documentation)
- [Quickstart](#quickstart)
  - [First Commands](#first-commands)
- [Installation](#installation)
- [Highlights](#highlights)
- [Works Well With](#works-well-with)
- [License](#license)

## Overview

Kelora parses log streams into structured events and runs them through a programmable pipeline powered by Rhai scripting.

- Turns lines into structured events you can access as `e.field` or `e["field-name"]`.
- Embeds 40+ built-in Rhai helpers for parsing, enrichment, metrics, and windowed analysis.
- Speaks JSON, logfmt, syslog, CSV/TSV, column specs, and gzip-compressed inputs out of the box.
- Handles streaming or batch workloads with sequential and `--parallel` execution modes.
- Emits metrics and processing stats so you can observe pipelines while they run.

## Documentation

- [Quickstart](https://kelora.dev/quickstart/) — 5-minute tour with annotated output
- [Concepts](https://kelora.dev/concepts/pipeline-model/) — deep dive into the streaming pipeline
- [How-To Guides](https://kelora.dev/how-to/) — solutions for common tasks
- [Reference](https://kelora.dev/reference/) — CLI flags, Rhai functions, formats, and configuration
- [Tutorials](https://kelora.dev/tutorials/) — step-by-step guides for building custom pipelines

## Quickstart

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

More quick commands to copy-paste:

- Stream-level error watch: `tail -f examples/simple_json.jsonl | kelora -j --level warn,error --exec 'track_count(e.service)' --metrics`
- Fan out nested arrays: `kelora -j examples/json_arrays.jsonl --exec 'emit_each(e.get_path(\"users\", []))' --keys id,name,score`
- Alias sensitive fields: `kelora -j examples/security_audit.jsonl --exec 'e.user_alias = pseudonym(e.user, \"users\"); e.ip_masked = e.ip.mask_ip(1)' --keys timestamp,event,user_alias,ip_masked`

> [!TIP]
> The sample logs in `examples/` map to the categories in [examples/README.md](examples/README.md#file-categories). Start there before pointing Kelora at production data. Need a fast reminder of the core flags? Run `kelora --help-quick`.

### Installation

#### Prebuilt binaries (recommended)

1. Download the archive for your platform from the [GitHub releases](https://github.com/dloss/kelora/releases) page (macOS, Linux, and Windows builds are provided).
2. Unpack the archive and move `kelora` or `kelora.exe` somewhere on your `PATH`.
3. Run `kelora --help` to ensure the binary starts up correctly.

If you prefer to manage the build yourself, Kelora targets stable Rust; keep your toolchain fresh (`rustup update`) for best results.

#### Install from crates.io or source

```bash
# Install from crates.io
cargo install kelora

# Build from source
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```

## Highlights

Kelora is built for streaming-first log analysis with a programmable Rhai core.

- **Streaming pipeline** — Parse, filter, transform, and output logs in one flow. Read the [Pipeline Model](docs/concepts/pipeline-model.md) for a stage-by-stage breakdown.
- **Built-in Rhai toolbox** — 100+ helpers for enrichment, parsing, time-window analysis, and metrics. Scan the [Functions Reference](docs/reference/functions.md) for the full catalog.
- **Format flexibility** — JSON, logfmt, syslog, Apache/Nginx combined, CSV/TSV, column specs, and gzip. See [Input Formats](docs/reference/input-formats.md).
- **Powerful filtering** — Chain `--filter`, `--level`, `--since/--until`, and context flags to zero in on events. Walkthroughs in [Filtering How-To](docs/how-to/find-errors-in-logs.md).
- **Observability built in** — `--metrics`, `--stats`, and window helpers expose throughput and aggregations for live pipelines. Learn more in [Metrics & Telemetry](docs/concepts/metrics-and-telemetry.md).
- **Parallel or streaming** — Stay sequential for tailing or enable `--parallel` for archive crunching. Tuning guidance in [Parallel Processing](docs/how-to/tune-parallel-processing.md).

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
