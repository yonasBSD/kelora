Proposed Example Log Files for Kelora (35 files)

  Basic Format Coverage (8 files)

  1. simple_json.jsonl - Basic JSON logs, 20 lines, mixed levels
  2. simple_line.log - Plain text logs, 15 lines for basic filtering
  3. simple_csv.csv - CSV with headers, 25 rows with status/bytes/duration
  4. simple_tsv.tsv - TSV format, 20 rows of tabular data
  5. simple_logfmt.log - Logfmt format, 30 lines of structured data
  6. simple_syslog.log - RFC3164/5424 syslog, 25 lines mixed priorities
  7. simple_combined.log - Apache/Nginx combined format, 40 access log entries
  8. simple_cef.log - Common Event Format (CEF), 15 security events

  Advanced Format Features (6 files)

  9. cols_fixed.log - Fixed-width columns for cols: parser testing
  10. cols_mixed.log - Mixed whitespace-separated columns with special chars
  11. csv_typed.csv - CSV with type annotations (status:int bytes:int)
  12. prefix_docker.log - Docker compose logs with container prefixes
  13. prefix_custom.log - Custom multi-char separator prefix extraction
  14. kv_pairs.log - Mixed key-value formats for parse_kv testing

  Multiline Scenarios (5 files)

  15. multiline_stacktrace.log - Java/Python stacktraces with timestamps
  16. multiline_json_arrays.log - Pretty-printed JSON events
  17. multiline_continuation.log - Lines with backslash continuation
  18. multiline_boundary.log - BEGIN/END block delimiters
  19. multiline_indent.log - Indented log entries (YAML-style)

  Complex Real-World Data (5 files)

  20. web_access_large.log - 1000+ combined format entries for parallel testing
  21. json_nested_deep.jsonl - Deeply nested JSON for get_path/has_path
  22. json_arrays.jsonl - Events with arrays for emit_each/sorted/unique
  23. security_audit.jsonl - Mixed IPs, JWTs, hashes for security functions
  24. timezones_mixed.log - Various timestamp formats and timezones

  Error Handling & Resilience (7 files)

  25. errors_json_mixed.jsonl - Valid JSON mixed with malformed (missing braces, trailing commas)
  26. errors_json_types.jsonl - Type mismatches for conversion function testing
  27. errors_empty_lines.log - Mix of empty lines, whitespace-only, valid entries
  28. errors_csv_ragged.csv - CSV with inconsistent column counts
  29. errors_unicode.log - Invalid UTF-8, mixed encodings, special chars
  30. errors_filter_runtime.jsonl - Data triggering Rhai filter errors (division by zero, null access)
  31. errors_exec_transform.jsonl - Data causing transformation failures for rollback testing

  Feature-Specific Testing (4 files)

  32. window_metrics.jsonl - Time-series data for window functions
  33. fan_out_batches.jsonl - Nested arrays for multi-level emit_each
  34. custom_timestamps.log - Non-standard timestamp formats requiring --ts-format
  35. sampling_hash.jsonl - High-volume data (500+ lines) for bucket() sampling demos

  The error handling files (25-31) will specifically demonstrate:
  - Resilient mode continuing despite parse errors
  - Strict mode aborting on first error
  - Verbose error reporting with context
  - Exit code behavior (0 vs 1)
  - Multi-level quiet mode suppression
  - Filter vs exec error handling differences
  - Type conversion failures and safe fallbacks