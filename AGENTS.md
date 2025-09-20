# Repository Guidelines

Kelora is a Rust log processor; this guide keeps contributions aligned with current workflows.

## Project Structure & Module Organization
- Core CLI and pipeline code live in `src/`; `main.rs` wires the Clap interface and `engine.rs` orchestrates parsing, filtering, and formatting stages.
- Parsers sit under `src/parsers/`, stream helpers in `readers.rs` and `decompression.rs`; extend these instead of introducing standalone binaries.
- Rhai built-ins and utilities are in `src/rhai_functions/`; update `docs/` or `dev/` notes whenever scripting behavior shifts.
- Integration coverage belongs in `tests/`, fixtures in `example_logs/`, and performance harnesses in `benchmarks/`.

## Build, Test, and Development Commands
- `cargo build --release` or `cargo run -- <flags>` produce local binaries for smoke testing.
- `make test`, `make test-unit`, and `make test-integration` wrap targeted `cargo test` runs for faster iteration.
- `make fmt`, `make lint`, and `make check` enforce formatting plus `cargo clippy -- -D warnings`.
- `make bench-quick` samples hot paths; reserve `make bench` for updates to the stored baseline.

## Coding Style & Naming Conventions
- Default to `cargo fmt` output: four-space indentation, trailing commas, module/file snake_case; structs and enums in PascalCase, constants in SCREAMING_SNAKE_CASE.
- Handle fallible CLI paths with `anyhow::Result` and `?`; choose slices or references over owned allocations inside tight loops.
- Add concise Rustdoc for new flags or Rhai functions so `--help`, `help-screen.txt`, and docs stay synchronized.

## Testing Guidelines
- Place focused unit tests in-module behind `#[cfg(test)]`; add scenario coverage in `tests/` named `<feature>_integration_test.rs`.
- Run `cargo test -q` or `make test` before submitting changes.
- When output shifts, regenerate `help-screen.txt` via `cargo run -- --help > help-screen.txt` and refresh sample fixtures in `example_logs/` as needed.
- Keep tests deterministic by leaning on `tempfile` or bundled fixtures instead of randomness.

## Commit & Pull Request Guidelines
- Follow existing history: imperative commit subjects under ~72 characters, optional bodies explaining rationale, and `Fixes #123` lines for linked issues.
- PRs need a summary of behavior changes, testing evidence, and notes about new flags, Rhai APIs, or performance considerations.
- Attach before/after CLI snippets when formatting shifts and mention any updated docs or benchmarking results.
- Coordinate major parser or engine adjustments with maintainers early, especially if they affect `benchmarks/` expectations or default outputs.

## No Backwards Compatiblity

Do not care for backwards compatiblity.