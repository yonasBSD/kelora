# Cross-platform build automation for Kelora

# Default recipe (list available recipes)
default:
    @just --list

# Extract package version from Cargo.toml (used by release workflow)
RELEASE_VERSION := `rg --max-count 1 --replace '$1' '^version\s*=\s*"([^"]+)"' Cargo.toml | tr -d '\r\n'`

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

# Generate coverage report (requires cargo-llvm-cov + llvm-tools-preview)
coverage *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
        echo "error: cargo-llvm-cov is not installed. Install with 'cargo install cargo-llvm-cov --locked'." >&2
        exit 1
    fi
    if ! rustup component list --installed | grep -qE "llvm-tools(-preview)?"; then
        echo "error: rustup component 'llvm-tools' is not installed. Install with 'rustup component add llvm-tools' (or llvm-tools-preview on older toolchains)." >&2
        exit 1
    fi
    if [[ "$#" -eq 0 ]]; then
        CARGO_TARGET_DIR=target cargo llvm-cov --workspace --all-features --html
        echo "Coverage HTML report: target/llvm-cov/html/index.html"
    else
        CARGO_TARGET_DIR=target cargo llvm-cov --workspace --all-features "$@"
    fi

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

# Generate comparison datasets for external tool benchmarks
bench-datasets:
    ./benchmarks/generate_comparison_data.sh

# Run external tool comparison benchmarks (requires grep, jq, etc.)
bench-compare:
    ./benchmarks/compare_tools.sh

# Run all benchmarks (internal + external comparisons)
bench-all: bench bench-compare

# Serve documentation locally with auto-reload
docs-serve:
    cargo build --release
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    PATH="{{justfile_directory()}}/target/release:${PATH}" \
    FORCE_COLOR=1 \
    COLUMNS=80 \
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec[ansi] mkdocs serve --watch docs --watch mkdocs.yml --livereload

# Build documentation (for local testing)
docs-build:
    cargo build --release
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    PATH="{{justfile_directory()}}/target/release:${PATH}" \
    FORCE_COLOR=1 \
    COLUMNS=80 \
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec[ansi] mkdocs build

# List published documentation versions (from gh-pages via mike)
docs-list-versions:
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec[ansi] mike list

# Delete one or more published documentation versions
docs-delete-version *versions:
    #!/usr/bin/env bash
    set -euo pipefail
    versions=( {{versions}} )
    if [[ "${#versions[@]}" -eq 0 ]]; then
        echo "error: provide at least one version to delete (e.g. 'just docs-delete-version 0.7.2')" >&2
        exit 1
    fi
    mkdir -p "{{justfile_directory()}}/.uv/cache" "{{justfile_directory()}}/.uv/data" "{{justfile_directory()}}/.uv/tools"
    UV_CACHE_DIR="{{justfile_directory()}}/.uv/cache" \
    UV_DATA_DIR="{{justfile_directory()}}/.uv/data" \
    UV_TOOL_DIR="{{justfile_directory()}}/.uv/tools" \
    uvx --with mkdocs-material --with mike --with markdown-exec[ansi] mike delete --push "${versions[@]}"

# Run JSON parser fuzzing locally (requires cargo-fuzz + nightly toolchain)
fuzz-json *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! cargo fuzz --help >/dev/null 2>&1; then
        echo "error: cargo-fuzz is not installed. Install with 'cargo install cargo-fuzz'." >&2
        exit 1
    fi
    cargo +nightly fuzz run json_parser "$@"

# Run Regex parser fuzzing locally (requires cargo-fuzz + nightly toolchain)
fuzz-regex *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! cargo fuzz --help >/dev/null 2>&1; then
        echo "error: cargo-fuzz is not installed. Install with 'cargo install cargo-fuzz'." >&2
        exit 1
    fi
    cargo +nightly fuzz run regex_parser "$@"

# Run multiline chunker fuzzing locally (requires cargo-fuzz + nightly toolchain)
fuzz-multiline *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! cargo fuzz --help >/dev/null 2>&1; then
        echo "error: cargo-fuzz is not installed. Install with 'cargo install cargo-fuzz'." >&2
        exit 1
    fi
    cargo +nightly fuzz run multiline_chunker "$@"

# Generate documentation screenshots using VHS
screenshots:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v vhs >/dev/null 2>&1; then
        echo "error: vhs is not installed. Install it from https://github.com/charmbracelet/vhs" >&2
        exit 1
    fi
    cargo build --release
    echo "Generating screenshots with VHS..."
    for tape in vhs/*.tape; do
        echo "  Processing $(basename "$tape")..."
        vhs "$tape"
    done
    echo "Screenshots generated in docs/screenshots/"

# Prepare a release: verify version, run checks, and create the tag (no pushes)
release-prepare:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION="{{RELEASE_VERSION}}"
    TAG="v${VERSION}"

    if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
        echo "error: Cargo.toml version '$VERSION' is not valid SemVer" >&2
        exit 1
    fi

    LATEST_TAG="$(git tag --list 'v*' --sort=-v:refname | head -n 1 || true)"

    if [[ -n "$LATEST_TAG" && "$LATEST_TAG" == "$TAG" ]]; then
        echo "error: Cargo.toml version '$VERSION' matches the latest release tag '$LATEST_TAG'. Update the version before releasing." >&2
        exit 1
    fi

    if [[ -n "$(git status --porcelain)" ]]; then
        echo "error: working tree is not clean. Commit or stash changes before releasing." >&2
        exit 1
    fi

    CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
    TARGET_BRANCH="${RELEASE_BRANCH:-$CURRENT_BRANCH}"
    REMOTE="${RELEASE_REMOTE:-origin}"

    echo "==> Running documentation build..."
    "{{just_executable()}}" docs-build

    echo "==> Running full check suite..."
    "{{just_executable()}}" check

    if git rev-parse "$TAG" >/dev/null 2>&1; then
        echo "error: git tag '$TAG' already exists" >&2
        exit 1
    fi

    git tag "$TAG"
    echo "==> Created tag $TAG"

    echo
    echo "Release tag created locally. Push when ready:"
    echo "  git push ${REMOTE} ${TARGET_BRANCH}"
    echo "  git push ${REMOTE} ${TAG}"

install:
    cargo install --path .
