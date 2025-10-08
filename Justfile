# Cross-platform build automation for Kelora

# Default recipe (list available recipes)
default:
    @just --list

# Format code
fmt:
    cargo fmt --all

# Run clippy linter
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
    cargo test -q

# Run unit tests only
test-unit:
    cargo test -q --bin kelora

# Run integration tests only
test-integration:
    cargo test -q --tests

# Run cargo audit
audit:
    cargo audit --no-fetch

# Run cargo deny checks
deny:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p .cargo-deny
    mkdir -p target
    if [ -d "$HOME/.cargo/advisory-dbs" ]; then
        rm -rf .cargo-deny/advisory-dbs
        cp -R "$HOME/.cargo/advisory-dbs" .cargo-deny/
    fi
    cargo metadata --format-version 1 > target/cargo-deny-metadata.json
    CARGO_HOME={{justfile_directory()}}/.cargo-deny \
    CARGO_DENY_HOME={{justfile_directory()}}/.cargo-deny \
    cargo deny check --disable-fetch --metadata-path target/cargo-deny-metadata.json

# Run all checks (fmt, lint, audit, deny, test)
check: fmt lint audit deny test

# Run benchmark suite
bench:
    ./benchmarks/run_benchmarks.sh

# Run quick benchmarks (10k dataset only)
bench-quick:
    ./benchmarks/run_benchmarks.sh --quick

# Update benchmark baseline
bench-update:
    ./benchmarks/run_benchmarks.sh --update-baseline

# Serve documentation locally
docs-serve:
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec mkdocs serve

# Build documentation
docs-build:
    cargo build --release
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    PATH="{{justfile_directory()}}/target/release:${PATH}" \
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec mkdocs build

# Deploy dev documentation
docs-deploy-dev:
    cargo build --release
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    PATH="{{justfile_directory()}}/target/release:${PATH}" \
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec mike deploy dev

# Set default documentation version
docs-set-default version:
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec mike set-default {{version}}

# Deploy release documentation (requires version tag)
docs-deploy-release version:
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec mike deploy --update-aliases {{version}} latest
