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
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec[ansi] mkdocs serve --watch docs --watch mkdocs.yml --livereload

# Build documentation (for local testing)
docs-build:
    cargo build --release
    mkdir -p {{justfile_directory()}}/.uv/cache {{justfile_directory()}}/.uv/data {{justfile_directory()}}/.uv/tools
    PATH="{{justfile_directory()}}/target/release:${PATH}" \
    UV_CACHE_DIR={{justfile_directory()}}/.uv/cache \
    UV_DATA_DIR={{justfile_directory()}}/.uv/data \
    UV_TOOL_DIR={{justfile_directory()}}/.uv/tools \
    uvx --with mkdocs-material --with mike --with markdown-exec[ansi] mkdocs build

# Run JSON parser fuzzing locally (requires cargo-fuzz + nightly toolchain)
fuzz-json *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! cargo fuzz --help >/dev/null 2>&1; then
        echo "error: cargo-fuzz is not installed. Install with 'cargo install cargo-fuzz'." >&2
        exit 1
    fi
    cargo +nightly fuzz run json_parser "$@"

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
