# Kelora

[![CI](https://github.com/dloss/kelora/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/dloss/kelora/actions/workflows/ci.yml) [![Crates.io](https://img.shields.io/crates/v/kelora.svg)](https://crates.io/crates/kelora) [![Documentation](https://img.shields.io/badge/docs-kelora.dev-blue)](https://kelora.dev)

**Turn messy logs into structured data.**

Kelora is a scriptable log processor for the command line. Parse mixed formats, filter with complex logic, and analyze streams using embedded [Rhai](https://rhai.rs) scripting with 150+ built-in functions. Handles JSON, logfmt, syslog, CSV/TSV, gzip, with sequential or `--parallel` execution and built-in metrics.

## Quick Example

```bash
kelora -f syslog examples/syslog_multiline.log --filter 'e.prog == "nginx"' -F json
```

**Input (mixed syslog):**
```
Oct 25 08:15:23 webserver01 nginx[1234]: 192.168.1.100 - - [25/Oct/2024:08:15:23 +0000] "GET /api/users HTTP/1.1" 200 1432
Oct 25 08:15:25 appserver01 backend[5678]: INFO: Processing order #12345
Oct 25 08:15:32 webserver01 nginx[1234]: 192.168.1.102 - - [25/Oct/2024:08:15:32 +0000] "GET /api/health HTTP/1.1" 200 23
```

**Output (filtered & structured):**
```json
{"ts":"Oct 25 08:15:23","msg":"192.168.1.100 - - [25/Oct/2024:08:15:23 +0000] \"GET /api/users HTTP/1.1\" 200 1432","host":"webserver01","prog":"nginx","pid":1234}
{"ts":"Oct 25 08:15:32","msg":"192.168.1.102 - - [25/Oct/2024:08:15:32 +0000] \"GET /api/health HTTP/1.1\" 200 23","host":"webserver01","prog":"nginx","pid":1234}
```

## When to Use Kelora

Kelora trades speed for programmability—slower than grep/awk/jq, but adds stateful scripting for complex transformations. Use it when you have **mixed formats**, need **stateful logic** (counters, windowed metrics, lookup tables), or are **chaining multiple tools**. For simple text search use `grep`, for JSON queries use `jq`.

See [Power-User Techniques](https://kelora.dev/latest/how-to/power-user-techniques/) for JWT parsing, cryptographic pseudonymization, pattern normalization, and deterministic sampling.

## Installation

Download from **[GitHub Releases](https://github.com/dloss/kelora/releases)** (macOS, Linux, Windows) or:

```bash
cargo install kelora
```

## Documentation

Read the full documentation at **[kelora.dev](https://kelora.dev)**:

- [Quickstart](https://kelora.dev/latest/quickstart/)
- [Tutorials](https://kelora.dev/latest/tutorials/)
- [How-To Guides](https://kelora.dev/latest/how-to/)
- [Concepts](https://kelora.dev/latest/concepts/)
- [Reference](https://kelora.dev/latest/reference/)

## Examples

The [`examples/`](https://github.com/dloss/kelora/tree/main/examples) directory contains 60+ sample log files covering JSON, logfmt, syslog, CSV, and more. Use them to test filters, transformations, and edge cases.

For common patterns and usage recipes, run:
```bash
kelora --help-examples
```

## Development Status

⚠️ Pre-1.0 tool generated entirely by AI agents. Validated by a large test suite and Rust security tools, but **inspect the code before production use**. APIs may change before v1.0. See the [Security Policy](https://github.com/dloss/kelora/blob/main/SECURITY.md).

This is a spare-time solo project—responses and updates happen on a best-effort basis.

## License

Kelora is open source software licensed under the [MIT License](https://github.com/dloss/kelora/blob/main/LICENSE).
