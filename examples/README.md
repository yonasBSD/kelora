# Kelora Example Log Files

**37 example files** demonstrating Kelora's capabilities across formats, scenarios, and complexity levels.

## Quick Start

```bash
# Try some examples
kelora -j simple_json.jsonl -l ERROR
kelora -f combined web_access_large.log.gz --parallel --stats
kelora -j json_arrays.jsonl -e 'emit_each(e.users)' -k id,name,score
```

## File Categories

### Basic Formats
Simple examples of common formats:
- [simple_json.jsonl](simple_json.jsonl) - JSON logs (20 events)
- [simple_csv.csv](simple_csv.csv) - CSV with headers (25 rows)
- [simple_tsv.tsv](simple_tsv.tsv) - Tab-separated (20 rows)
- [simple_logfmt.log](simple_logfmt.log) - Logfmt key=value (30 lines)
- [simple_syslog.log](simple_syslog.log) - RFC3164 syslog (25 lines)
- [simple_combined.log](simple_combined.log) - Apache/Nginx logs (40 lines)
- [simple_cef.log](simple_cef.log) - Security CEF format (15 events)
- [simple_line.log](simple_line.log) - Plain text (15 lines)

### Advanced Formats
Specialized parsing features:
- [cols_fixed.log](cols_fixed.log) - Fixed-width columns
- [cols_mixed.log](cols_mixed.log) - Mixed whitespace columns
- [csv_typed.csv](csv_typed.csv) - CSV with type annotations (`status:int`)
- [prefix_docker.log](prefix_docker.log) - Docker container prefixes
- [prefix_custom.log](prefix_custom.log) - Custom separators (`>>>`)
- [kv_pairs.log](kv_pairs.log) - Key-value pairs

### Multiline
Events spanning multiple lines:
- [multiline_stacktrace.log](multiline_stacktrace.log) - Java/Python stacktraces
- [multiline_json_arrays.log](multiline_json_arrays.log) - Pretty-printed JSON
- [multiline_continuation.log](multiline_continuation.log) - Backslash continuation
- [multiline_boundary.log](multiline_boundary.log) - BEGIN/END blocks
- [multiline_indent.log](multiline_indent.log) - YAML-style indentation

### Complex Real-World
Production-like scenarios:
- [web_access_large.log.gz](web_access_large.log.gz) - 1200 access logs (gzipped, 65KB)
- [json_nested_deep.jsonl](json_nested_deep.jsonl) - Deeply nested JSON
- [json_arrays.jsonl](json_arrays.jsonl) - Arrays for fan-out
- [security_audit.jsonl](security_audit.jsonl) - IPs, JWTs, hashes
- [timezones_mixed.log](timezones_mixed.log) - Various timestamp formats

### Error Handling
Testing resilience:
- [errors_json_mixed.jsonl](errors_json_mixed.jsonl) - Valid + malformed JSON
- [errors_json_types.jsonl](errors_json_types.jsonl) - Type conversion challenges
- [errors_empty_lines.log](errors_empty_lines.log) - Empty lines, whitespace
- [errors_csv_ragged.csv](errors_csv_ragged.csv) - Inconsistent columns
- [errors_unicode.log](errors_unicode.log) - Unicode, special chars
- [errors_filter_runtime.jsonl](errors_filter_runtime.jsonl) - Runtime error triggers
- [errors_exec_transform.jsonl](errors_exec_transform.jsonl) - Transform failures

### Feature-Specific
Advanced capabilities:
- [window_metrics.jsonl](window_metrics.jsonl) - Time-series for window functions
- [fan_out_batches.jsonl](fan_out_batches.jsonl) - Multi-level nested arrays
- [custom_timestamps.log](custom_timestamps.log) - Non-standard formats
- [sampling_hash.jsonl.gz](sampling_hash.jsonl.gz) - 600 events for sampling (gzipped, 3.6KB)

### Nightmare Mode
Extremely challenging scenarios:
- [nightmare_mixed_formats.log](nightmare_mixed_formats.log) - JSON + logfmt + syslog in one file
- [nightmare_deeply_nested_transform.jsonl](nightmare_deeply_nested_transform.jsonl) - 4-6 levels of nesting

## Common Patterns

**Filter and select:**
```bash
kelora -j simple_json.jsonl -l ERROR -k timestamp,service,message
```

**Visual level distribution:**
```bash
kelora -f logfmt simple_logfmt.log -F levelmap
```
The `levelmap` formatter fills the available terminal width, prefixing each block with the first event's timestamp so you can spot bursts of specific levels at a glance.

**Safe nested access:**
```bash
kelora -j json_nested_deep.jsonl \
  -e 'e.theme = e.get_path("request.user.profile.settings.theme", "light")'
```

**Array fan-out:**
```bash
kelora -j json_arrays.jsonl -e 'emit_each(e.users)' -k id,name,score
```

**Multi-level fan-out:**
```bash
kelora -j fan_out_batches.jsonl \
  -e 'let ctx = #{batch_id: e.batch_id}; emit_each(e.orders, ctx)' \
  -e 'let ctx2 = #{batch_id: e.batch_id, order_id: e.order_id}; emit_each(e.items, ctx2)' \
  -k batch_id,order_id,sku,qty,price
```

**Metrics and aggregation:**
```bash
kelora -j simple_json.jsonl --metrics \
  -e 'track_count(e.service)' \
  --end 'for k in metrics.keys() { print(k + ": " + metrics[k]) }' \
  -F none
```

**Mixed formats in one file:**
```bash
kelora nightmare_mixed_formats.log \
  -e 'if e.line.starts_with("{") { e = e.line.parse_json() }
      else if e.line.contains("timestamp=") { e = e.line.parse_logfmt() }'
```

**Gzipped files (transparent decompression):**
```bash
kelora -f combined web_access_large.log.gz --parallel --stats
kelora -j sampling_hash.jsonl.gz --filter 'e.user_id.to_string().bucket() % 10 == 0'
```

**Multiline logs:**
```bash
kelora multiline_stacktrace.log --multiline timestamp --filter 'e.line.contains("ERROR")'
```

**Error handling modes:**
```bash
kelora -j errors_json_mixed.jsonl                # Resilient (default)
kelora -j errors_json_mixed.jsonl --strict       # Fail-fast
kelora -j errors_json_mixed.jsonl --verbose      # Show each error
```

## See Also

- `kelora --help-rhai` - Rhai basics and idioms
- `kelora --help-functions` - All Rhai functions
