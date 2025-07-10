Certainly. Here is the **complete, finalized specification** for Kelora’s error handling, reporting, and metrics output, ready for implementation, documentation, and user education.

---

# 📄 Kelora Error Handling, Reporting, and Metrics Specification

## Overview

Kelora separates:

* **What to do when errors occur** → `--on-error`
* **How errors are reported** → `--error-report`
* **What values are tracked from Rhai scripts** → `--metrics`
* **How performance and log field data is reported** → `--stats`

The design is focused on clarity, safety, composability, and usability — both for interactive use and pipelines.

---

## 🟦 `--on-error <action>`

Controls how Kelora behaves when an error is encountered.

| Mode       | Behavior                                       |
| ---------- | ---------------------------------------------- |
| `fail`     | Stop immediately on first error; exit non-zero |
| `skip`     | Silently discard bad input and continue        |
| `continue` | Process all lines despite errors (default)     |

**Short option:** `-x`
**Default:** `--on-error=continue`

---

## 🟨 `--error-report <style>[=file]`

Controls how errors are reported.

| Style     | Behavior                                     | Output Format | Output Location |
| --------- | -------------------------------------------- | ------------- | --------------- |
| `off`     | Suppress all error messages                  | —             | —               |
| `summary` | Group errors by type, show counts + examples | JSON          | stderr or file  |
| `print`   | Print each error immediately to stderr       | Plain text    | stderr or file  |

If `=file` is given, output is written there. If omitted, it defaults to stderr.

---

## 🔚 Exit Codes

| Error Type Encountered | `--on-error=fail` | `--on-error=skip/continue` |
| ---------------------- | ----------------- | -------------------------- |
| Fatal error            | `exit(2)`         | `exit(2)`                  |
| Any other error        | `exit(1)`         | `exit(0)`                  |
| No errors              | `exit(0)`         | `exit(0)`                  |

---

## 📊 Internal Error Severities

Kelora classifies errors internally:

| Severity | Examples                           | Printed? | Triggers Exit?  |
| -------- | ---------------------------------- | -------- | --------------- |
| Fatal    | I/O failure, panic                 | Always   | Yes (`exit(2)`) |
| Hard     | Rhai error, CLI misuse, bad regex  | Always   | Yes (`exit(1)`) |
| Medium   | Parse failure, CSV mismatch        | Optional | Yes (`exit(1)`) |
| Soft     | Missing field, null, coercion fail | Optional | No              |

---

## 📈 `--metrics`

Prints values tracked via Rhai `track_*()` functions.

| Function                 | Output Key | Format                        |
| ------------------------ | ---------- | ----------------------------- |
| `track_count("x")`       | `x`        | Integer                       |
| `track_max("x", val)`    | `x`        | Float or int                  |
| `track_unique("x", val)` | `x`        | `{ count: N, sample: [...] }` |

**Rhai variable name:** `metrics`

**CLI output:**

```text
📊 === Kelora Metrics ===
errors       = 83
latency_ms   = 948
users        = { count: 189, sample: ["alice", "bob", "carol"] }
```

**JSON output (via `--metrics-file=...`):**

```json
{
  "errors": 83,
  "latency_ms": 948,
  "users": {
    "count": 189,
    "sample": ["alice", "bob", "carol"]
  }
}
```

---

## 📈 `--stats`

Shows processing statistics and parsed data characteristics.

**CLI output:**

```text
📈 === Kelora Stats ===
lines_in   = 12000
lines_out  = 11890
duration   = 1.28s
throughput = 9.3k/s
levels     = info,error,debug
keys       = ts,level,msg,user
```

| Field        | Description                            |
| ------------ | -------------------------------------- |
| `lines_in`   | Input lines read                       |
| `lines_out`  | Events emitted after parsing/filtering |
| `duration`   | Total run time, human-readable         |
| `throughput` | Processing rate                        |
| `levels`     | Comma-separated levels discovered      |
| `keys`       | Comma-separated field names parsed     |

Lists are comma-separated with **no spaces**, ready to reuse with `--levels` or `--keys`.

---

## ❌ `--no-section-headers`

Suppresses all section headers (emoji + `=== Kelora ... ===`) from stderr output. This is useful for:

* Scripting
* Embedding in pipelines
* Log post-processing

This does **not** affect file outputs from `--metrics-file` or `--error-report=...`.

---

## 📘 Rhai Scripting Integration

All metrics tracked in Rhai are exposed via the `metrics` variable:

```rhai
track_count("errors");
track_max("duration", latency_ms);
track_unique("users", user_id);

if metrics["errors"] > 100 {
  print("Too many errors!");
}
```

You can inspect `.count`, `.sample`, etc., for unique metrics:

```rhai
let users = metrics["users"];
if users.count > 500 {
  print("High cardinality");
}
```

---

## 🧪 Example CLI Usage

```bash
kelora logs.jsonl --metrics --stats

kelora logs.jsonl --error-report summary=errors.json

kelora -x fail logs.jsonl --metrics-file=metrics.json

kelora logs.jsonl --metrics --no-section-headers | grep latency
```

---

## 🧰 Configuration Example

```ini
[defaults]
on-error = continue
error-report = summary=errors.json
metrics = true
metrics-file = metrics.json
stats = true
```

---

## 📘 CLI Help Summary

```text
ERROR HANDLING

  -x, --on-error <action>         What to do when errors occur (default: continue)
                                    fail      Stop on first error (exit 1/2)
                                    skip      Skip invalid input, continue
                                    continue  Process all lines regardless

      --error-report <style>[=file]
                                  How to report errors
                                    off       Suppress all error messages
                                    summary   Grouped summary (default for continue)
                                    print     Print every error (default for fail)

METRICS AND STATS

      --metrics                   Show values tracked in Rhai via track_*()
      --metrics-file <path>      Write metrics to file (JSON format)

      --stats                    Show performance statistics and log field info

OUTPUT FORMAT CONTROL

      --no-section-headers       Suppress emoji + section headers from stderr output
```
