# Kelora Documentation

**Scriptable log processor for the command line.**

Kelora parses log streams into structured events and lets you filter, transform, and analyze them using embedded [Rhai](https://rhai.rs) scripting with 40+ built-in functions.

!!! warning "Experimental Tool"
    Kelora is an experimental tool under active development. APIs may change without notice.

## What is Kelora?

Kelora turns log lines into structured events you can manipulate programmatically:

- **Parse** - JSON, logfmt, syslog, CSV/TSV, Apache/Nginx combined format, or custom column specs
- **Filter** - Keep only events that match your conditions
- **Transform** - Enrich, redact, extract, or restructure event data
- **Analyze** - Track metrics, compute aggregations, detect patterns
- **Output** - Default key=value format, JSON, CSV, or custom templates

## Quick Examples

### Find errors in application logs

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  --levels error \
  --keys timestamp,service,message
```

### Analyze web server traffic

```bash exec="on" source="above" result="ansi"
kelora -f combined examples/web_access_large.log.gz \
  --filter 'e.status >= 400' \
  --keys ip,timestamp,status,request \
  --take 3
```

### Track metrics from streaming logs

Process logs as they arrive (example with static file):

```bash exec="on" source="above" result="ansi"
kelora -f json examples/simple_json.jsonl \
  --exec 'track_count(e.service)' \
  --metrics
```

## Installation

### Prebuilt Binaries (Recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/dloss/kelora/releases):

- macOS (Intel and Apple Silicon)
- Linux (x86_64)
- Windows (x86_64)

Unpack the archive and move `kelora` to somewhere on your `PATH`.

### From Source

Requires Rust stable toolchain:

```bash
# Install from crates.io
cargo install kelora

# Or build from source
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```

## Getting Started

New to Kelora? Start here:

1. **[Quickstart](quickstart.md)** - Get up and running in 5 minutes
2. **[Tutorials](tutorials/parsing-custom-formats.md)** - Learn core skills step-by-step
3. **[How-To Guides](how-to/find-errors-in-logs.md)** - Solve real problems
4. **[Reference](reference/functions.md)** - Look up functions, formats, and CLI options

## Key Features

### Flexible Parsing

Built-in parsers for common formats:

- **JSON** (`-f json`) - Standard JSON logs
- **Logfmt** (`-f logfmt`) - key=value format
- **Syslog** (`-f syslog`) - RFC3164 and RFC5424
- **Combined** (`-f combined`) - Apache/Nginx access logs
- **CSV/TSV** (`-f csv`, `-f tsv`) - Tabular data
- **Custom** (`-f cols:<spec>`) - Define your own column-based format

### Powerful Scripting

Embedded Rhai scripting with 40+ built-in functions:

- String manipulation: `extract_re()`, `parse_json()`, `contains()`
- Type conversion: `to_int()`, `to_float()`, `to_bool()`
- Time handling: `parse_datetime()`, `format_datetime()`, `now_utc()`
- Metrics tracking: `track_count()`, `track_sum()`, `track_avg()`
- Array processing: `sorted()`, `unique()`, `emit_each()`
- Field operations: `get_path()`, `has_path()`, `path_equals()`

### Performance Options

- **Sequential mode** (default) - Maintains order, lower memory
- **Parallel mode** (`--parallel`) - Higher throughput, uses multiple cores
- **Streaming** - Process data as it arrives
- **Batch** - Process large archives efficiently

### Resilient Processing

- Skip unparseable lines by default
- Continue processing on script errors
- Strict mode (`--strict`) for fail-fast behavior
- Comprehensive error reporting

## Documentation Structure

This documentation follows the [Diátaxis](https://diataxis.fr/) framework:

- **[Tutorials](tutorials/parsing-custom-formats.md)** - Learning-oriented lessons
- **[How-To Guides](how-to/find-errors-in-logs.md)** - Task-oriented solutions
- **[Reference](reference/functions.md)** - Information-oriented lookup
- **[Concepts](concepts/pipeline-model.md)** - Understanding-oriented explanations

## Need Help?

- **CLI help**: Run `kelora --help` for comprehensive CLI reference
- **Function reference**: `kelora --help-functions` for all built-in Rhai functions
- **Examples**: `kelora --help-examples` for common patterns
- **GitHub**: [Report issues or request features](https://github.com/dloss/kelora/issues)

## License

Kelora is open source software licensed under the [MIT License](https://github.com/dloss/kelora/blob/main/LICENSE).
