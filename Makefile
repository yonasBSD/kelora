.PHONY: fmt lint check test test-unit test-integration bench bench-quick bench-update

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
	cargo audit

deny-check:
	cargo deny check

check: fmt lint audit deny-check test

bench:
	./benchmarks/run_benchmarks.sh

bench-quick:
	./benchmarks/run_benchmarks.sh --quick

bench-update:
	./benchmarks/run_benchmarks.sh --update-baseline
