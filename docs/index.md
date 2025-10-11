# Kelora

**Scriptable log processor for the command line.**

Parse messy logs into structured events, then filter, transform, and analyze them with embedded [Rhai](https://rhai.rs) scripting.

!!! warning "Experimental Tool"
    Kelora is a [vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding) experimental tool under active development. APIs and behaviour may change without notice.

## Quick Start

```bash
# Find errors across JSON logs
kelora -f json examples/simple_json.jsonl --levels error

# Enrich logs - calculate derived fields on the fly
kelora -f json examples/simple_json.jsonl \
  --exec 'e.duration_s = e.get_path("duration_ms", 0) / 1000' \
  --keys timestamp,service,duration_s \
  --take 5

# Analyze web server failures - add custom fields with Rhai
kelora -f combined examples/web_access_large.log.gz \
  --exec 'e.error_type = if e.status >= 500 { "server" } else { "client" }' \
  --filter 'e.status >= 400' --take 3

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

Kelora processes logs through a streaming pipeline:

```
Input → Parse → Filter → Transform → Output
  ↓       ↓        ↓         ↓           ↓
Files   JSON    --levels  --exec      logfmt
stdin   syslog  --filter  --begin     JSON
.gz     custom  --since   --end       CSV
```

Each stage transforms data and passes it forward. Read the [Pipeline Model](concepts/pipeline-model.md) to understand how stages work together.

## Key Features

- **Resilient Processing** - Skip bad lines automatically, continue processing
- **Parallel Mode** - Process large archives using all CPU cores with `--parallel`
- **Sliding Windows** - Analyze events in context with `--window`
- **100+ Functions** - Rich built-in library for transformation and analysis
- **Format Conversion** - Read any format, write any format
- **Metrics Tracking** - Built-in counters, sums, averages, distributions

## Install

**[Download from GitHub Releases](https://github.com/dloss/kelora/releases)** (macOS, Linux, Windows) or:

```bash
cargo install kelora
```

## Learn More

- **[Quickstart](quickstart.md)** - 5-minute tour
- **[Tutorials](tutorials/parsing-custom-formats.md)** - Step-by-step guides
- **[How-To](how-to/find-errors-in-logs.md)** - Solve specific problems
- **[Concepts](concepts/pipeline-model.md)** - Understanding how Kelora works
- **[Reference](reference/functions.md)** - Functions, formats, CLI options

Run `kelora --help` for comprehensive CLI docs, or `kelora --help-functions` for all built-in Rhai functions.

## Works Well With

Kelora focuses on normalising noisy logs. Pair it with complementary CLI tools when you need deeper analysis:

- **jq/jaq** — slice JSONL output for downstream scripts or human-readable TSV
- **qsv** — crunch CSV exports with lightning-fast aggregations
- **DuckDB/sqlite** — query exported data with SQL for complex analysis
- **miller** — reshape and aggregate tabular data

## Need Help?

- **CLI help**: Run `kelora --help` for comprehensive CLI reference
- **Function reference**: `kelora --help-functions` for all built-in Rhai functions
- **Examples**: `kelora --help-examples` for common patterns
- **GitHub**: [Report issues or request features](https://github.com/dloss/kelora/issues)

## License

Kelora is open source software licensed under the [MIT License](https://github.com/dloss/kelora/blob/main/LICENSE).
