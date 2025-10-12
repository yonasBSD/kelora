# Format Reference

Complete reference for all input formats supported by Kelora.

## Overview

Kelora supports multiple input formats for parsing log files:

| Format | Description | Use Case |
|--------|-------------|----------|
| `json` | JSON lines (one object per line) | Application logs, structured data |
| `line` | Plain text (default) | Unstructured logs, raw text |
| `logfmt` | Key-value pairs | Heroku-style logs, simple structured logs |
| `csv` / `tsv` | Comma/tab-separated values | Spreadsheet data, exports |
| `syslog` | Syslog RFC5424 and RFC3164 | System logs, network devices |
| `combined` | Apache/Nginx log formats (Common + Combined) | Web server access logs |
| `cef` | ArcSight Common Event Format | Security logs, SIEM data |
| `cols:<spec>` | Custom column parsing | Column-based logs (whitespace or custom separators) |
| `auto` | Auto-detect format | Any supported format |

## JSON Format

### Syntax

```bash
-f json
# or shorthand:
-j
```

### Description

Parse each line as a JSON object. Nested structures are preserved.

### Input Example

```json
{"timestamp": "2024-01-15T10:30:00Z", "level": "ERROR", "service": "api", "message": "Connection failed"}
{"timestamp": "2024-01-15T10:30:01Z", "level": "INFO", "service": "db", "message": "Query executed"}
```

### Output Fields

All JSON fields become event fields with their original names and types.

### Usage

```bash
> kelora -j app.log --levels error
> kelora -f json app.log --keys timestamp,level,message
```

### Notes

- One JSON object per line (JSON Lines format)
- Supports nested objects and arrays
- Preserves field types (strings, numbers, booleans, null)
- Use with `-M json` for multi-line JSON objects

## Line Format

### Syntax

```bash
-f line
# (default if no format specified)
```

### Description

Parse each line as plain text. Each line becomes an event with a single `line` field.

### Input Example

```
2024-01-15 10:30:00 ERROR Connection failed
2024-01-15 10:30:01 INFO Query executed
```

### Output Fields

- `line` - The complete line as a string

### Usage

```bash
> kelora app.log --filter 'e.line.contains("ERROR")'
> kelora -f line app.log --exec 'e.level = e.line.extract_re(r"(ERROR|INFO|WARN)")'
```

### Notes

- Default format when no `-f` is specified
- Empty lines are skipped
- Useful for unstructured logs or custom parsing

## Logfmt Format

### Syntax

```bash
-f logfmt
```

### Description

Parse Heroku-style key-value pairs (logfmt format).

### Input Example

```
timestamp=2024-01-15T10:30:00Z level=ERROR service=api message="Connection failed"
timestamp=2024-01-15T10:30:01Z level=INFO service=db message="Query executed"
```

### Output Fields

All key-value pairs become top-level fields.

### Usage

```bash
> kelora -f logfmt app.log --levels error
> kelora -f logfmt app.log --keys timestamp,service,message
```

### Notes

- Supports quoted values with spaces: `key="value with spaces"`
- Unquoted values end at whitespace
- Keys must be alphanumeric (with underscores/hyphens)

## CSV / TSV Formats

### Syntax

```bash
-f csv        # Comma-separated with header
-f tsv        # Tab-separated with header
-f csvnh      # CSV without header
-f tsvnh      # TSV without header
```

### Description

Parse comma-separated or tab-separated values. First line is used as header (unless `nh` variant).

### Input Example

**With header:**
```csv
timestamp,level,service,message
2024-01-15T10:30:00Z,ERROR,api,Connection failed
2024-01-15T10:30:01Z,INFO,db,Query executed
```

**Without header:**
```csv
2024-01-15T10:30:00Z,ERROR,api,Connection failed
2024-01-15T10:30:01Z,INFO,db,Query executed
```

### Output Fields

**With header:** Field names from header row
**Without header:** `col_0`, `col_1`, `col_2`, etc.

### Usage

```bash
> kelora -f csv data.csv --levels error
> kelora -f csvnh data.csv --keys col_0,col_1,col_2
```

### Type Annotations

Specify field types for automatic conversion:

```bash
> kelora -f 'csv status:int bytes:int response_time:float' access.csv
```

**Supported types:**

- `int` - Parse as integer
- `float` - Parse as floating-point number
- `bool` - Parse as boolean

### Notes

- Quoted fields supported: `"value, with, commas"`
- Escaped quotes: `"value with ""quotes"""`
- Empty fields become empty strings

## Syslog Format

### Syntax

```bash
-f syslog
```

### Description

Parse syslog messages (RFC5424 and RFC3164).

### Input Example

**RFC5424:**
```
<165>1 2024-01-15T10:30:00.000Z myhost myapp 1234 ID47 - Connection failed
```

**RFC3164:**
```
<34>Jan 15 10:30:00 myhost myapp[1234]: Connection failed
```

### Output Fields

**RFC5424:**

- `facility` - Syslog facility code
- `severity` - Syslog severity level
- `timestamp` - Parsed timestamp
- `hostname` - Source hostname
- `appname` - Application name
- `procid` - Process ID
- `msgid` - Message ID
- `message` - Log message

**RFC3164:**

- `facility` - Syslog facility code
- `severity` - Syslog severity level
- `timestamp` - Parsed timestamp
- `hostname` - Source hostname
- `tag` - Syslog tag (usually appname[pid])
- `message` - Log message

### Usage

```bash
> kelora -f syslog /var/log/syslog --filter 'e.severity <= 3'
> kelora -f syslog messages.log --keys timestamp,hostname,appname,message
```

### Notes

- Auto-detects RFC5424 vs RFC3164
- Severity levels: 0=emerg, 1=alert, 2=crit, 3=err, 4=warn, 5=notice, 6=info, 7=debug

## Combined Log Format

### Syntax

```bash
-f combined
```

### Description

Parse Apache and Nginx web server access logs. Automatically handles three format variants:
- **Apache Common Log Format (CLF)** - Basic format without referer/user-agent
- **Apache Combined Log Format** - Extended format with referer and user-agent
- **Nginx Combined with request_time** - Combined format plus request processing time

### Input Examples

**Apache Common Log Format:**
```
192.168.1.1 - user [15/Jan/2024:10:30:00 +0000] "GET /index.html HTTP/1.0" 200 1234
```

**Apache Combined Log Format:**
```
192.168.1.1 - user [15/Jan/2024:10:30:00 +0000] "GET /api/data HTTP/1.1" 200 1234 "http://example.com/" "Mozilla/5.0"
```

**Nginx Combined with request_time:**
```
192.168.1.1 - - [15/Jan/2024:10:30:00 +0000] "GET /api/data HTTP/1.1" 200 1234 "-" "curl/7.68.0" "0.123"
```

### Output Fields

**Common to all variants:**
- `ip` - Client IP address (required)
- `identity` - RFC 1413 identity (omitted when `-`)
- `user` - HTTP authenticated username (omitted when `-`)
- `timestamp` - Request timestamp (required)
- `request` - Full HTTP request line (required)
- `method` - HTTP method (auto-extracted from request)
- `path` - Request path (auto-extracted from request)
- `protocol` - HTTP protocol version (auto-extracted from request)
- `status` - HTTP status code (required)
- `bytes` - Response size in bytes (omitted when `-`, included when `0`)

**Additional fields in Combined format:**
- `referer` - HTTP referer header (omitted when `-`)
- `user_agent` - HTTP user agent header (omitted when `-`)

**Additional field in Nginx variant:**
- `request_time` - Request processing time in seconds (omitted when `-`)

### Usage

```bash
# Works with all three format variants
> kelora -f combined /var/log/nginx/access.log --filter 'e.status >= 400'
> kelora -f combined /var/log/apache2/access.log --filter 'e.status == 404'
> kelora -f combined access.log --keys ip,status,request,request_time
```

### Notes

- Parser automatically detects which variant is present in each log line
- Auto-extracts method, path, protocol from request line
- `request_time` only available in Nginx logs configured with `$request_time` variable
- Fields with `-` values are omitted from output (except `bytes` which includes `0`)

## CEF Format

### Syntax

```bash
-f cef
```

### Description

Parse ArcSight Common Event Format (security logs).

### Input Example

```
CEF:0|Security|threatmanager|1.0|100|worm successfully stopped|10|src=10.0.0.1 dst=2.1.2.2 spt=1232
```

### Output Fields

- `cef_version` - CEF format version
- `device_vendor` - Device vendor name
- `device_product` - Device product name
- `device_version` - Device version
- `signature_id` - Event signature ID
- `name` - Event name
- `severity` - Event severity
- Plus all extension fields as top-level fields

### Usage

```bash
> kelora -f cef security.log --filter 'e.severity > 5'
> kelora -f cef security.log --keys timestamp,name,src,dst
```

## Column Format

### Syntax

```bash
-f 'cols:<spec>'
```

### Description

Parse custom column-based formats with field specifications. The parser splits each line by whitespace by default (or a custom separator) and applies the tokens you provide.

### Specification Syntax

```
cols:field1 field2 field3
```

**Token types:**

- `field` - Consume one column and assign it to `field`
- `field(N)` - Consume `N` columns and join them into `field`
- `-` / `-(N)` - Skip one or `N` columns
- `*field` - Capture every remaining column into `field` (must be last)
- `field:type` - Apply a type annotation (`int`, `float`, `bool`, `string`)

### Input Examples

**Whitespace-delimited:**
```
ERROR api "Connection failed"
```

**Spec:**
```bash
-f 'cols:level service *message'
```

**Timestamp spread across columns:**
```
2024-01-15 10:30:00 INFO Connection failed
```

**Spec:**
```bash
-f 'cols:timestamp(2) level *message'
```

### Output Fields

Field names come from the specification. Type annotations convert values after extraction.

### Usage

```bash
# Whitespace-delimited
> kelora -f 'cols:level service *message' app.log

# Multi-token timestamp with type annotations
> kelora -f 'cols:timestamp(2) level status:int *message' app.log --ts-field timestamp

# Custom separator
> kelora -f 'cols:name age:int city' --cols-sep ',' data.txt
```

### Notes

- Default separator: whitespace
- Use `--cols-sep` to specify a custom separator
- Give timestamps recognizable names (`timestamp`, `ts`, `time`) or set `--ts-field`
- `*field` must be the final token because it consumes the remainder of the line

## Auto-Detection

### Syntax

```bash
-f auto
```

### Description

Automatically detect input format from first line.

### Detection Order

1. JSON (if line starts with `{`)
2. Syslog (if line starts with `<NNN>`)
3. CEF (if line starts with `CEF:`)
4. Combined (if matches Apache/Nginx Common or Combined log pattern)
5. Logfmt (if contains `key=value` pairs)
6. CSV (if contains commas with consistent pattern)
7. Line (fallback)

### Usage

```bash
> kelora -f auto mixed.log --levels error
```

### Notes

- Detects format from first non-empty line
- Uses same format for all subsequent lines
- Not suitable for files with mixed formats

## Format-Specific Options

### Timestamp Configuration

#### `--ts-field <field>`

Specify custom timestamp field name:

```bash
> kelora -j --ts-field created_at app.log
```

#### `--ts-format <format>`

Specify custom timestamp format (chrono format strings):

```bash
> kelora --ts-format '%Y-%m-%d %H:%M:%S' app.log
> kelora --ts-format '%d/%b/%Y:%H:%M:%S %z' access.log
```

See `--help-time` for format reference.

#### `--input-tz <timezone>`

Timezone for naive timestamps:

```bash
> kelora --input-tz local app.log
> kelora --input-tz Europe/Berlin app.log
```

### Column Format Options

#### `--cols-sep <separator>`

Column separator for `cols:<spec>` format:

```bash
> kelora -f 'cols:name age city' --cols-sep '|' data.txt
```

### Prefix Extraction

#### `--extract-prefix <field>`

Extract text before separator into field (before parsing):

```bash
> docker compose logs | kelora --extract-prefix service -j
```

#### `--prefix-sep <separator>`

Prefix separator (default: `|`):

```bash
> kelora --extract-prefix node --prefix-sep ' :: ' cluster.log
```

### Multi-line Events

#### `-M, --multiline <strategy>`

Multi-line event detection:

```bash
> kelora -M json app.log              # Multi-line JSON objects
> kelora -M '^\\d{4}-' app.log        # Events start with year
> kelora -M '^\\S' app.log            # Events start with non-whitespace
```

See `--help-multiline` for strategy reference.

## Output Formats

Control how Kelora outputs events:

### `-F, --output-format <format>`

| Format | Description |
|--------|-------------|
| `default` | Key-value format with colors (default) |
| `json` | JSON lines (one object per line) |
| `logfmt` | Key-value pairs (logfmt format) |
| `inspect` | Debug format with type information |
| `levelmap` | Grouped by log level |
| `csv` | CSV with header |
| `tsv` | Tab-separated values with header |
| `csvnh` | CSV without header |
| `tsvnh` | TSV without header |
| `none` | No output (useful with `--stats` or `--metrics`) |

### Examples

```bash
# Output as JSON
> kelora -j app.log -F json

# Output as CSV
> kelora -j app.log -F csv --keys timestamp,level,message

# No event output, only stats
> kelora -j app.log -F none --stats
```

## Format Conversion

Convert between formats by combining input and output formats:

```bash
# JSON to CSV
> kelora -j app.log -F csv --keys timestamp,level,message > output.csv

# Logfmt to JSON
> kelora -f logfmt app.log -F json > output.jsonl

# CSV to Logfmt
> kelora -f csv data.csv -F logfmt > output.log

# Combined log to JSON
> kelora -f combined access.log -F json > access.jsonl
```

## Common Patterns

### Parse JSON with Auto-Detection

```bash
> kelora -f auto app.log --levels error
```

### Parse Web Logs and Filter

```bash
> kelora -f combined /var/log/nginx/access.log \
    --filter 'e.status >= 400' \
    --keys ip,status,request
```

### Parse CSV with Type Conversion

```bash
> kelora -f 'csv status:int bytes:int' data.csv \
    --filter 'e.status >= 400'
```

### Parse Custom Format

```bash
> kelora -f 'cols:timestamp level service *message' app.log \
    --ts-field timestamp \
    --levels error
```

### Parse Syslog and Extract

```bash
> kelora -f syslog /var/log/syslog \
    --filter 'e.severity <= 3' \
    --keys timestamp,hostname,message
```

### Parse with Prefix Extraction

```bash
> docker compose logs | kelora --extract-prefix container -j \
    --filter 'e.container == "web_1"'
```

## Troubleshooting

### Format Not Detected

**Problem:** Auto-detection picks wrong format.

**Solution:** Specify format explicitly:
```bash
> kelora -f json app.log
```

### Timestamp Not Parsed

**Problem:** Timestamps not recognized.

**Solution:** Specify timestamp format:
```bash
> kelora --ts-format '%Y-%m-%d %H:%M:%S' app.log
```

### CSV Parsing Issues

**Problem:** Quoted fields not handled correctly.

**Solution:** Ensure CSV is properly formatted with standard quoting:
```csv
"field with, comma","another field"
```

### Multi-line Events

**Problem:** Single event spans multiple lines.

**Solution:** Use multi-line mode:
```bash
> kelora -M json app.log
```

### Mixed Formats

**Problem:** Log file contains multiple formats.

**Solution:** Use preprocessing or separate files:
```bash
> grep '^{' mixed.log | kelora -j --levels error
> grep -v '^{' mixed.log | kelora -f line
```

## See Also

- [CLI Reference](cli-reference.md) - Complete flag documentation
- [Quickstart](../quickstart.md) - Format examples
- [Parsing Custom Formats Tutorial](../tutorials/parsing-custom-formats.md) - Step-by-step guide
- [Process CSV Data How-To](../how-to/process-csv-data.md) - CSV-specific tips
