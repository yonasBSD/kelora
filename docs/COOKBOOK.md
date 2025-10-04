# Kelora Cookbook

Quick, runnable patterns for common log-processing workflows. Every example stays close to the
built-in sample logs in `examples/` so you can try them without touching production data.

## 1. Fast Filters & Projections

### Narrow to interesting events
```bash
kelora -j examples/simple_json.jsonl --filter 'e.service == "database"' \
  --exec 'e.duration_s = e.get_path("duration_ms", 0) / 1000' \
  --keys timestamp,message,duration_s
```

### Slice by text before parsing
```bash
cat examples/prefix_docker.log | \
  kelora --extract-prefix container --prefix-sep ' | ' \
    --filter 'e.container == "web_1"'
```

## 2. Streaming & Alerts

### Tail logs and count incidents
```bash
tail -f examples/simple_logfmt.log | \
  kelora -f logfmt \
    --filter '"duration" in e && e.duration.to_int_or(0) >= 1000' \
    --exec 'track_count("slow_requests")' \
    --metrics
```

### Sliding-window anomaly detection
```bash
kelora -f syslog examples/simple_syslog.log \
  --filter '"msg" in e && e.msg.contains("Failed login")' \
  --window 5 \
  --exec 'let hits = window_values("msg").filter(|m| m.contains("Failed login"));\
           if hits.len() >= 3 { e.alert = true; }' \
  --filter 'e.alert == true'
```

## 3. Enrichment & Reshaping

### Add derived attributes
```bash
kelora -f combined examples/web_access_large.log.gz \
  --exec 'let status = e.status.to_int();\
          e.family = if status >= 500 { "server_error" } else if status >= 400 { "client_error" } else { "ok" };'
```

### Anonymise while keeping linkability
```bash
kelora -j examples/security_audit.jsonl \
  --exec 'e.user_alias = pseudonym(e.user, "users"); e.ip_masked = e.ip.mask_ip(1)' \
  --keys timestamp,event,user_alias,ip_masked
```

## 4. Fan-Out & Nested Data

### Flatten arrays safely
```bash
kelora -j examples/json_arrays.jsonl \
  --exec 'emit_each(e.get_path("users", []))' \
  --keys id,name,score
```

### Multi-level fan-out with context
```bash
kelora -j examples/fan_out_batches.jsonl \
  --exec 'let ctx = #{batch_id: e.batch_id}; emit_each(e.orders, ctx)' \
  --exec 'let ctx2 = #{batch_id: e.batch_id, order_id: e.order_id}; emit_each(e.items, ctx2)' \
  --keys batch_id,order_id,sku,qty,price
```

## 5. Metrics & Windows

### Rolling average on numeric streams
```bash
kelora -j examples/window_metrics.jsonl \
  --window 5 \
  --exec 'let values = window_numbers("value");\
           if values.len() == 5 { let total = 0.0; for v in values { total += v; }\
                                  e.moving_avg = total / values.len(); }'
```

### Histogram buckets per request family
```bash
kelora -f combined examples/web_access_large.log.gz \
  --metrics \
  --exec 'track_bucket("status_family", (e.status / 100 * 100).to_int_or(0))' \
  --end 'for (bucket, counts) in metrics.status_family {\
           print(bucket.to_string() + ": " + counts.to_string()); }' \
  -F none
```

## 6. Format Tricks

### Type annotations on CSV/TSV
```bash
kelora -f "csv status:int bytes:int duration_ms:int" examples/simple_csv.csv
kelora -f "tsv: user_id:int success:bool" examples/simple_tsv.tsv
```

### Column specs with joins and strict mode
```bash
kelora -f "cols:date(2) level *msg:string" examples/cols_fixed.log
kelora -f "csv status:int" --strict examples/errors_csv_ragged.csv
```

## 7. Performance Checklist

1. **Streaming?** stay sequential (default) and emit to stdout.
2. **Archives?** add `--parallel --stats`, monitor stats for skew, then tune `--batch-size`/`--batch-timeout`.
3. **Windowed workloads?** prefer smaller windows (`--window 50`) or sample events upstream to curb memory.
4. **Verbose scripts?** switch to `-q` once pipelines stabilise to cut stderr noise.
5. **Need ordering guarantees?** skip `--unordered`; otherwise enable it for faster parallel flushes.

## 8. Troubleshooting Cheats

- Diagnose parse hiccups with `-F inspect` or `--verbose`.
- Timestamp woes? add `--ts-field`, `--ts-format`, or `--input-tz` (see `kelora --help-time`).
- Rhai panics? wrap lookups with `e.get_path("field", ())` and numeric conversions with `to_int_or`.
- Gzip everywhere? nothing to add: Kelora auto-detects compressed input.

## See Also

- `kelora --help-quick` – one-screen cheat sheet of the busiest flags.
- `kelora --help` – grouped CLI reference.
- `kelora --help-rhai` – language recap and stage semantics.
- [examples index](https://github.com/dloss/kelora/blob/main/examples/README.md) – sample catalogue.
