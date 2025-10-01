.PHONY: fmt lint audit deny check test test-unit test-integration bench bench-quick bench-update

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test -q

test-unit:
	cargo test -q --bin kelora

test-integration:
	cargo test -q --tests

audit:
	cargo audit --no-fetch

deny:
	mkdir -p .cargo-deny
	mkdir -p target
	@if [ -d "$$HOME/.cargo/advisory-dbs" ]; then \
		rm -rf .cargo-deny/advisory-dbs; \
		cp -R "$$HOME/.cargo/advisory-dbs" .cargo-deny/; \
	fi
	cargo metadata --format-version 1 > target/cargo-deny-metadata.json
	CARGO_HOME=$(PWD)/.cargo-deny \
	CARGO_DENY_HOME=$(PWD)/.cargo-deny \
	cargo deny check --disable-fetch --metadata-path target/cargo-deny-metadata.json

check: fmt lint audit deny test

bench:
	./benchmarks/run_benchmarks.sh

bench-quick:
	./benchmarks/run_benchmarks.sh --quick

bench-update:
	./benchmarks/run_benchmarks.sh --update-baseline
