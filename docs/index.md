# Kelora

**Scriptable log processor for the command line.**

Parse messy logs into structured events, then filter, transform, and analyze them with embedded [Rhai](https://rhai.rs) scripting.

!!! warning "Experimental Tool"
    Kelora is a [vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding) experimental tool under active development. APIs and behaviour may change without notice.

## Examples

```bash
# Parse multi-line stacktraces - keeps errors with their full traces
kelora examples/multiline_stacktrace.log --multiline timestamp \
  --filter 'e.line.contains("ERROR")'

# Fan out nested arrays - turn one event with 3 users into 3 separate events
kelora -j examples/json_arrays.jsonl \
  --exec 'if e.has_path("users") { emit_each(e.users) }' \
  --keys id,name,score

# Parse mixed formats - handles JSON, logfmt, and syslog in one file
kelora examples/nightmare_mixed_formats.log \
  --exec 'if e.line.starts_with("{") { e = e.line.parse_json() }
          else if e.line.contains("=") { e = e.line.parse_logfmt() }
          else { e = e.line.parse_syslog() }'

# Track errors by service - parses Docker container prefixes, counts by source
kelora examples/prefix_docker.log --extract-prefix service --prefix-sep '|' \
  --metrics --exec 'if e.line.contains("ERROR") { track_count(e.service) }'
```

See the [Quickstart](quickstart.md) for a step-by-step tour with full output.

## What It Does

- **Parse** JSON, logfmt, syslog, CSV/TSV, Apache/Nginx logs, or custom formats
- **Filter** with Rhai expressions - keep events matching your conditions
- **Transform** with 100+ built-in functions - enrich, redact, extract, restructure
- **Analyze** with built-in metrics - track counts, sums, averages, distributions
- **Output** as logfmt, JSON, or CSV

## How It Works

Kelora processes logs through a streaming pipeline with composable stages:

```
Input → Parse → --exec → --filter → --exec → --filter → ... → Output
  ↓       ↓         ↓         ↓         ↓         ↓              ↓
Files   JSON   transform  narrow   enrich    narrow        logfmt
stdin   syslog                                              JSON
.gz     custom                                              CSV
```

Each `--filter` and `--exec` runs in the order specified, passing events forward. Chain them in any sequence to build multi-stage processing logic. Read the [Pipeline Model](concepts/pipeline-model.md) for details.

## Key Features

- **Resilient Processing** - Skip bad lines automatically, continue processing
- **Parallel Mode** - Process large archives using all CPU cores with `--parallel`
- **Sliding Windows** - Analyze events in context with `--window`
- **100+ Functions** - Rich built-in library for transformation and analysis
- **Format Conversion** - Read any format, write any format
- **Metrics Tracking** - Built-in counters, sums, averages, distributions

## Install

Download from **[GitHub Releases](https://github.com/dloss/kelora/releases)** (macOS, Linux, Windows) or:

```bash
cargo install kelora
```

## Learn More

- **[Quickstart](quickstart.md)** - 5-minute tour
- **[Tutorials](tutorials/parsing-custom-formats.md)** - Step-by-step guides
- **[How-To](how-to/find-errors-in-logs.md)** - Solve specific problems
- **[Concepts](concepts/pipeline-model.md)** - Understanding how Kelora works
- **[Reference](reference/functions.md)** - Functions, formats, CLI options

Run `kelora --help` for comprehensive command-line help screens, or browse the [example files on GitHub](https://github.com/dloss/kelora/tree/main/examples).

## Works Well With

Kelora focuses on normalising noisy logs into structured data. Reach for it when you need programmable filtering, enrichment, or windowed analytics in one streaming pipeline—and pair it with the tools below when you need deeper visualisation or post-processing:

- **[jq](https://jqlang.github.io/jq/)** — process Kelora's JSON output for complex transformations, filtering, or reformatting
- **[lnav](https://lnav.org/)** — explore Kelora's output in an interactive TUI with live filtering, histograms, and ad-hoc SQL queries
- **[qsv](https://github.com/jqnatividad/qsv)** — analyze Kelora's CSV output with statistical operations, joins, and aggregations
- **[SQLite](https://www.sqlite.org/)/[DuckDB](https://duckdb.org/)** — load Kelora's CSV/JSON output into a database for SQL queries and reporting
- **[miller](https://github.com/johnkerl/miller)** — transform Kelora's CSV output for reshaping, aggregating, and format conversion

## License

Kelora is open source software licensed under the [MIT License](https://github.com/dloss/kelora/blob/main/LICENSE).
