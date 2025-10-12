# Kelora

**Scriptable log processor for the command line.**

Parse messy logs into structured events, then filter, transform, and analyze them with embedded [Rhai](https://rhai.rs) scripting.

!!! warning "Experimental Tool"
    Kelora is a [vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding) experimental tool under active development. APIs and behaviour may change without notice.

## Examples

```bash
# Find errors across JSON logs
kelora -f json examples/simple_json.jsonl --levels error

# Enrich logs - calculate derived fields on the fly
kelora -f json examples/simple_json.jsonl \
  --exec 'e.duration_s = e.get_path("duration_ms", 0) / 1000' \
  --keys timestamp,service,duration_s

# Analyze web server failures - add custom fields with Rhai
kelora -f combined examples/web_access_large.log.gz \
  --exec 'e.error_type = if e.status >= 500 { "server" } else { "client" }' \
  --filter 'e.status >= 400'

# Track metrics - suppress events, show only counts
kelora -f json examples/simple_json.jsonl \
  --exec 'track_count(e.service)' --metrics -F none
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

**Command-line help:** `kelora --help` (CLI reference) · `--help-functions` (Rhai functions) · `--help-examples` (common patterns)

**[Browse 37 example files on GitHub](https://github.com/dloss/kelora/tree/main/examples)** · [Report issues](https://github.com/dloss/kelora/issues)

## Works Well With

Kelora focuses on normalising noisy logs into structured data. Pipe or export Kelora's output to complementary tools for deeper analysis:

- **[jq](https://jqlang.github.io/jq/)** — process Kelora's JSON output for complex transformations, filtering, or reformatting
- **[lnav](https://lnav.org/)** — explore Kelora's output in an interactive TUI with live filtering, histograms, and ad-hoc SQL queries
- **[qsv](https://github.com/jqnatividad/qsv)** — analyze Kelora's CSV output with statistical operations, joins, and aggregations
- **[SQLite](https://www.sqlite.org/)/[DuckDB](https://duckdb.org/)** — load Kelora's CSV/JSON output into a database for SQL queries and reporting
- **[miller](https://github.com/johnkerl/miller)** — transform Kelora's CSV output for reshaping, aggregating, and format conversion

## License

Kelora is open source software licensed under the [MIT License](https://github.com/dloss/kelora/blob/main/LICENSE).
