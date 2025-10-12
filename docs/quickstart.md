# Quickstart

Get started with Kelora in minutes. This guide shows real examples from parsing to advanced transformations.

## Installation

Download the latest release from [GitHub Releases](https://github.com/dloss/kelora/releases) or install via Cargo:

```bash
cargo install kelora
```

## Get the Examples

```bash
# With git
git clone https://github.com/dloss/kelora && cd kelora
```

```bash
# Without git
curl -L https://github.com/dloss/kelora/archive/refs/heads/main.zip -o kelora.zip && \
  unzip kelora.zip && \
  cd kelora-main
```

## Parse Unstructured Logs

Turn raw web server logs into structured, queryable data:

=== "Command"

    ```bash
    kelora -f combined examples/simple_combined.log -k ip,status,method,path -n 5
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f combined examples/simple_combined.log -k ip,status,method,path -n 5
    ```

The `-f combined` parses Apache/NGINX access logs into fields. The `-k` flag selects which fields to display, and `-n` limits output. Kelora automatically extracts `ip`, `timestamp`, `method`, `path`, `status`, `user_agent`, and more from each line.

## Filter and Transform

Filter by HTTP status codes and add computed fields:

=== "Command"

    ```bash
    kelora -f combined examples/simple_combined.log \
      --filter 'e.status >= 400' \
      -e 'e.error_type = if e.status >= 500 { "server" } else { "client" }' \
      -k ip,status,error_type,path -n 5
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f combined examples/simple_combined.log \
      --filter 'e.status >= 400' \
      -e 'e.error_type = if e.status >= 500 { "server" } else { "client" }' \
      -k ip,status,error_type,path -n 5
    ```

The `--filter` expression keeps only error responses (4xx and 5xx). The `-e` flag adds a computed `error_type` field based on the status code.

## Track Metrics

Count requests by status code and track response sizes:

=== "Command"

    ```bash
    kelora -f combined examples/simple_combined.log \
      -e 'track_count(e.status.to_string()); track_sum("total_bytes", e.bytes)' \
      -F none -m
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f combined examples/simple_combined.log \
      -e 'track_count(e.status.to_string()); track_sum("total_bytes", e.bytes)' \
      -F none -m
    ```

Use `track_count()`, `track_sum()`, `track_min()`, and `track_max()` to collect metrics. The `-F none` suppresses event output, `-m` displays metrics at the end.

## Convert Between Formats

Kelora converts between all supported formats. Some examples:

Convert syslog to JSON:

=== "Command"

    ```bash
    kelora -f syslog examples/simple_syslog.log -F json -n 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f syslog examples/simple_syslog.log -F json -n 3
    ```

Convert web logs to CSV:

=== "Command"

    ```bash
    kelora -f combined examples/web_access_large.log.gz -F csv -k ip,status,request -n 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    kelora -f combined examples/web_access_large.log.gz -F csv -k ip,status,request -n 3
    ```

The `-f` flag specifies input format, `-F` specifies output format (we could have used `-J` as a shortcut for JSON). Gzipped files are automatically decompressed.

## Common Patterns

```bash
# Stream processing (tail -f, kubectl logs, etc.)
kubectl logs -f deployment/api | kelora -f json -l error

# Multiple files - track which files have errors
kelora -f json logs/*.log --metrics \
  --exec 'if e.level == "ERROR" { track_count(meta.filename) }'

# Time-based filtering
kelora -f combined access.log --since "1 hour ago" --until "10 minutes ago"

# Extract prefixes (Docker Compose, systemd, etc.)
docker compose logs | kelora --extract-prefix container -f json

# Auto-detect format and output brief values only
kelora -f auto mixed.log -k timestamp,level,message -b

# Custom timestamp formats
kelora -f line app.log --ts-format "%d/%b/%Y:%H:%M:%S" --ts-field timestamp
```

## Get Help

```bash
kelora --help              # Complete CLI reference
kelora --help-examples     # More usage patterns
kelora --help-rhai         # Rhai scripting guide
kelora --help-functions    # All built-in Rhai functions
kelora --help-time         # Timestamp format reference
```

## Next Steps

- **Start with events** — Practice accessing and mutating `e.field` values on JSON or logfmt samples. Then branch into [How-To: Find Errors in Logs](how-to/find-errors-in-logs.md).
- **Explore parsers** — Try `-f json`, `-f combined`, and `-f 'cols:...'`, then dig into the [Input Formats reference](reference/formats.md).
- **Layer scripts** — Combine `--filter`, `--exec`, and `--keys` for enrichment. Deepen your skills with the [Scripting Transforms tutorial](tutorials/scripting-transforms.md).
- **Add metrics** — Introduce `track_count`, `track_sum`, and `--metrics`, then read the [Metrics & Tracking tutorial](tutorials/metrics-and-tracking.md).
- **Tune pipelines** — Experiment with multi-stage workflows, `--begin`/`--end`, and configs; the [Pipeline Model concept](concepts/pipeline-model.md) and [Configuration System guide](concepts/configuration-system.md) explain the moving pieces.
- **Control output** — Swap `-F` formats, use `-k`/`-K`, and convert timestamps. Reference the [CLI options](reference/cli-reference.md) when you need exact flag behaviour.
