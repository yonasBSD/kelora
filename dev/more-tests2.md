# Kelora Test TODOs (Follow-up)

- [ ] **Config file + alias flow**  
  Build integration tests that create temporary `.kelora.ini` files to verify precedence (project vs. user), alias expansion (`--alias`, `--save-alias`) and the `--ignore-config` / `--config-file` flags. None of the current suites exercise `src/config_file.rs`, so CLI coverage needs to prove defaults/aliases merge correctly and that invalid configs produce helpful errors.

- [ ] **`-f auto` format detection**  
  Add tests that feed representative single-line samples for JSON, syslog (RFC5424 + RFC3164), CEF, Apache combined, logfmt, CSV/TSV, and pure text into `kelora -f auto`. Confirm the auto-detected parser matches expectations and that malformed inputs fall back to `line`. Regressions in `src/parsers/auto_detect.rs` currently go unnoticed.

- [ ] **Multiline chunker edge cases**  
  Expand `tests/multiline_tests.rs` (and/or targeted unit tests) to cover `-M indent`, `-M timestamp[:format=...]`, regex start/end strategies, and invalid pattern handling. Also test how multiline interacts with filters, stats, and `--parallel` worker boundaries to validate `src/pipeline/multiline.rs`.

- [ ] **Reader + decompression coverage**  
  Create integration tests that read `.gz` and `.zst` inputs plus multi-file streams to exercise `PeekableLineReader`, `ChannelStdinReader`, and `DecompressionReader`. Include failure cases (unsupported `.zip`, truncated gzip) so the CLI plumbing around `src/decompression.rs` and `src/readers.rs` is verified end-to-end.

- [ ] **Signal/output resilience**  
  Add lightweight tests (probably behind `#[cfg(unix)]`) that simulate SIGUSR1/SIGTERM or invoke `SafeStdout`/`SafeFileOut` with broken pipes to ensure `src/platform.rs` continues to honor exit codes and prints stats. Even a mocked channel test would improve confidence.

- [ ] **`--allow-fs-writes` enforcement**  
  Integration coverage for Rhai file helpers: confirm append/truncate fail without the flag, succeed with it, and respect `--strict` when filesystem errors occur. This should drive the runtime switches in `src/rhai_functions/file_ops.rs` through the CLI rather than only unit tests.

- [ ] **Formatter/display flag variants**  
  Extend `tests/output_formatting_tests.rs` to assert behavior for `--wrap`/`--no-wrap`, `--expand-nested`, `--mark-gaps`, `--force-color` vs `--no-color`, and `--no-emoji`. Current tests only cover `--brief`, `--core`, quiet levels, and `--normalize-ts`, leaving the rest of `src/cli.rs` display options untested.

- [ ] **Parallel mode stress variants**  
  Add cases that mix `--parallel` with `--unordered`, tiny `--batch-timeout`, malformed events, or worker panics to ensure the coordinator propagates errors and honours ordering promises. Existing `tests/parallel_tests.rs` stay on the happy path.

- [ ] **Metrics/stats toggles**  
  Ensure `--metrics-json`, `--stats-only`, and conflicting combinations (`-m` with `--metrics-file`) are validated. Capture expected stderr/stdout output so regressions in `tests/metrics_tracking_tests.rs` get caught.

