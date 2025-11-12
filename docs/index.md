# Kelora

**Scriptable log processor for the command line.**

Parse messy logs into structured events, then filter, transform, and analyze them with embedded [Rhai](https://rhai.rs) scripting (a Rust-based scripting language with JavaScript-like syntax).

!!! note "Development Status"
    Pre-1.0 tool generated entirely by AI agents. Validated by a large test suite and Rust security tools; see [Development Approach](#development-approach) and the [Security Policy](https://github.com/dloss/kelora/blob/main/SECURITY.md) before relying on it in production. APIs might change without notice before v1.0.

## Quick Examples

**Detect problems** - Filter server errors and slow requests. Each log line becomes an event (`e`) you can query with expressions like `e.status >= 500`:

=== "Command/Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f logfmt examples/traffic_logfmt.log \
      --filter 'e.status.to_int() >= 500 || e.latency_ms.to_int() > 1200'
    ```

=== "Log Data"

    ```bash exec="on" result="ansi"
    cat examples/traffic_logfmt.log
    ```

**Parse custom formats and extract structured data** - Describe column structure instead of writing regex, then extract nested key-value pairs:

=== "Command/Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f 'cols:ts level service request_id *message' examples/release_pipe.log \
      --cols-sep '|' \
      --levels warn,error \
      --exec 'e.absorb_kv("message")' \
      -F json
    ```

=== "Log Data"

    ```bash exec="on" result="ansi"
    cat examples/release_pipe.log
    ```

**Enrich events with recent context** - Use sliding windows to add context from recent events:

=== "Command/Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -j examples/api_errors.jsonl \
      --window 5 \
      --exec 'let err = window.filter(|x| x.level == "ERROR");
              if err.len() > 0 { e.ctx = err[0].error; }' \
      --keys timestamp,endpoint,ctx \
      -F logfmt
    ```

=== "Log Data"

    ```bash exec="on" result="ansi"
    cat examples/api_errors.jsonl
    ```

## Why Kelora

Choose Kelora when you need **programmable log processing** in one streaming pipeline—filtering, transforming, and analyzing structured events with embedded scripts.

**Reach for Kelora when:**

- You need multi-stage transformations (parse → filter → enrich → filter)
- Your logs mix formats or use custom delimiters
- You want windowed analysis (error bursts, sliding metrics)
- You're chaining grep + awk + jq + custom scripts

**Use specialized tools for:**

- Simple text search: `grep`/`rg` (50-100× faster)
- Basic field extraction: `awk` (faster for simple splits)
- Pure JSON queries: `jq` (ubiquitous, similar speed)
- Interactive exploration: `lnav` (TUI with SQL)
- CSV analytics: `qsv`/`miller` (faster for stats)

Kelora trades raw speed for expressiveness. See [Performance Comparisons](concepts/performance-comparisons.md) for benchmarks and the full decision matrix.

## Integrate With

Pipe Kelora's output to: **jq** (JSON transforms) · **lnav** (interactive TUI) · **SQLite/DuckDB** (SQL queries) · **qsv/miller** (CSV analytics) · **rare** (visualizations)

See [Integrate Kelora with External Tools](how-to/integrate-external-tools.md) for 18 tools and usage patterns.

## Get Started

**[→ Quickstart (5 minutes)](quickstart.md)** - Install and run your first commands

**[→ Tutorial: Basics (30 minutes)](tutorials/basics.md)** - Learn input formats, filtering, and output

**[→ Troubleshooting](troubleshooting.md)** - Fix common issues and debug effectively

**[→ How-To Guides](how-to/index.md)** - Solve specific problems

For deeper understanding, see [Concepts](concepts/index.md). For complete reference, see [Glossary](glossary.md), [Functions](reference/functions.md), [Formats](reference/formats.md), and [CLI options](reference/cli-reference.md).

## License

Kelora is open source software licensed under the [MIT License](https://github.com/dloss/kelora/blob/main/LICENSE).

## Development Approach

Kelora is an experiment in agentic AI development. Claude Code (Sonnet 4.5) and Codex CLI (gpt-5-codex) generate all implementation and tests; I steer requirements but do not write or review code. Validation relies on the automated test suite plus `cargo audit` and `cargo deny`, so please inspect the code yourself before relying on it in production and expect the API to evolve until 1.0. This is a spare-time, single-developer project, so support and updates are best-effort.
