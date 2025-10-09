# Kelora

**Scriptable log processor for the command line.**

Parse messy logs into structured events, then filter, transform, and analyze them with embedded [Rhai](https://rhai.rs) scripting.

!!! warning "Experimental Tool"
    Kelora is a [vibe-coded](https://en.wikipedia.org/wiki/Vibe_coding) experimental tool under active development. APIs and behaviour may change without notice.

## Quick Start

```bash exec="on" source="above" result="ansi"
# Find errors across JSON logs
kelora -f json examples/simple_json.jsonl --levels error

# Enrich logs - calculate derived fields on the fly
kelora -f json examples/simple_json.jsonl \
  --exec 'e.duration_s = e.get_path("duration_ms", 0) / 1000' \
  --keys timestamp,service,duration_s

# Analyze web server failures with classification
kelora -f combined examples/web_access_large.log.gz \
  --exec 'e.error_type = if e.status >= 500 { "server" } else { "client" }' \
  --filter 'e.status >= 400' --take 3

# Track metrics from streaming logs
kelora -f json examples/simple_json.jsonl \
  --exec 'track_count(e.service)' --metrics
```

## What It Does

- **Parse** JSON, logfmt, syslog, CSV/TSV, Apache/Nginx logs, or custom formats
- **Filter** with Rhai expressions - keep events matching your conditions
- **Transform** with 100+ built-in functions - enrich, redact, extract, restructure
- **Analyze** with built-in metrics - track counts, sums, averages, distributions
- **Output** as logfmt, JSON, or CSV

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

## Documentation Structure

This documentation follows the [Diátaxis](https://diataxis.fr/) framework:

- **[Tutorials](tutorials/parsing-custom-formats.md)** - Learning-oriented lessons
- **[How-To Guides](how-to/find-errors-in-logs.md)** - Task-oriented solutions
- **[Reference](reference/functions.md)** - Information-oriented lookup
- **[Concepts](concepts/pipeline-model.md)** - Understanding-oriented explanations

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
