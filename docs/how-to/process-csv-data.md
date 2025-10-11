# Process CSV Data

Parse and analyze CSV/TSV files with type annotations, header handling, and field transformations.

## Problem

You have CSV or TSV files and need to parse them with proper type handling, filter rows, aggregate values, or transform the data for further analysis.

## Solutions

### Basic CSV Parsing

Parse CSV files with automatic header detection:

```bash
# Parse CSV with headers
kelora -f csv data.csv -n 5

# Parse TSV (tab-separated)
kelora -f tsv data.tsv -n 5
```

### CSV with Type Annotations

Specify field types for proper numeric handling:

=== "Command"

    ```bash
    # Type annotations can be in the CSV header itself
    kelora -f csv examples/csv_typed.csv -n 3
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    # Type annotations can be in the CSV header itself
    kelora -f csv examples/csv_typed.csv -n 3
    ```

Type annotations enable:

- Numeric comparisons and arithmetic
- Sorting by numeric values
- Proper aggregations (sum, average, etc.)

### CSV without Headers

Process CSV files without header rows:

```bash
# No headers - use csvnh (CSV No Header)
kelora -f csvnh data.csv

# Access by index: _1, _2, _3, etc.
kelora -f csvnh data.csv -e 'e.timestamp = e._1; e.status = e._2.to_int()'

# TSV without headers
kelora -f tsvnh data.tsv
```

### Filter CSV Rows

Filter based on field values:

=== "Command"

    ```bash
    # Filter by status code using CLI type annotations
    kelora -f 'csv status:int' examples/simple_csv.csv \
      --filter 'e.status >= 400'
    ```

=== "Output"

    ```bash exec="on" source="above" result="ansi"
    # Filter by status code using CLI type annotations
    kelora -f 'csv status:int' examples/simple_csv.csv \
      --filter 'e.status >= 400'
    ```

### Aggregate CSV Data

Calculate statistics across rows:

```bash
# Count by status code
kelora -f 'csv status:int' data.csv \
  -e 'track_count(e.status)' \
  --metrics

# Sum bytes transferred
kelora -f 'csv bytes:int' data.csv \
  -e 'track_sum("total_bytes", e.bytes)' \
  --metrics

# Track unique values
kelora -f csv data.csv \
  -e 'track_unique("methods", e.method)' \
  --metrics
```

### Transform CSV Fields

Create new fields or modify existing ones:

```bash
# Calculate derived fields
kelora -f 'csv bytes:int duration:int' data.csv \
  -e 'e.throughput = e.bytes / e.duration'

# Normalize timestamps
kelora -f csv data.csv \
  -e 'e.hour = to_datetime(e.timestamp).hour()'

# Extract path components
kelora -f csv data.csv \
  -e 'e.endpoint = e.path.split("/")[1]' \
  -e 'track_count(e.endpoint)' \
  --metrics
```

### Convert CSV to Other Formats

```bash
# CSV to JSON
kelora -f csv data.csv -J > output.json

# CSV to logfmt
kelora -f csv data.csv -F logfmt > output.log

# CSV to JSON with selected fields
kelora -f csv data.csv -k timestamp,status,bytes -F json
```

### Handle Ragged/Malformed CSV

Process CSV files with missing or extra fields:

```bash
# Resilient mode (default) - skip bad rows
kelora -f csv data.csv --stats

# See parsing errors
kelora -f csv data.csv --verbose

# Strict mode - abort on first error
kelora -f csv data.csv --strict
```

### Select Specific Columns

Output only desired fields:

```bash
# Select specific fields
kelora -f csv data.csv -k timestamp,method,status

# Exclude sensitive fields
kelora -f csv data.csv --exclude-keys email,ip_address

# Reorder fields in output
kelora -f csv data.csv -k status,method,path -F csv
```

## Real-World Examples

### Find Slow API Calls

```bash
kelora -f 'csv path method status:int duration:int' api_log.csv \
  --filter 'e.duration > 1000' \
  -e 'track_count(e.path)' \
  -k path,method,duration --metrics
```

### Calculate Response Time Percentiles

```bash
kelora -f 'csv duration:int' data.csv \
  --window 10000 --end '
    let times = window_numbers("duration");
    print("p50: " + times.percentile(50));
    print("p95: " + times.percentile(95));
    print("p99: " + times.percentile(99))
  '
```

### Export Error Rows

```bash
kelora -f 'csv status:int' data.csv \
  --filter 'e.status >= 500' \
  -J > errors.json
```

### Group by Time Windows

```bash
kelora -f csv data.csv \
  -e 'e.hour = to_datetime(e.timestamp).format("%Y-%m-%d %H:00")' \
  -e 'track_count(e.hour)' \
  --metrics
```

### Clean and Normalize Data

```bash
kelora -f csv raw_data.csv \
  -e 'e.email = e.email.to_lower().strip()' \
  -e 'e.status = to_int_or(e.status, 0)' \
  -e 'e.timestamp = to_datetime(e.timestamp).to_iso()' \
  -F csv > cleaned.csv
```

## Tips

**Type Handling:**

- Use `:int` for status codes, counts, IDs
- Use `:float` for durations, measurements, rates
- Use `:bool` for flags (accepts: true/false, 1/0, yes/no)
- Untyped fields remain as strings

**Performance:**

- Add `--parallel` for large CSV files
- Use `--batch-size` to control memory usage
- Filter early to reduce processing overhead

**Error Handling:**

- Use `--verbose` to see parsing errors
- Use `--stats` to see skip/error counts
- Ragged CSV (missing fields) creates partial events

**Field Access:**

- Headers become field names (spaces → underscores)
- Without headers, use `_1`, `_2`, `_3` etc.
- Use `to_int_or()` / `to_float_or()` for safe type conversion

## See Also

- [Fan Out Nested Structures](fan-out-nested-structures.md) - Process nested CSV data
- [Analyze Web Traffic](analyze-web-traffic.md) - Similar patterns for log files
- [Function Reference](../reference/functions.md) - Type conversion functions
- [CLI Reference](../reference/cli-reference.md) - Format specifications
