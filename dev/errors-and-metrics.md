# 📄 Kelora Error Handling, Reporting, Metrics, and Quarantine Specification

---

## Overview

Kelora separates:

* **What to do when errors occur** → `--on-error`
* **How errors are reported** → `--error-report`
* **What values are tracked from Rhai scripts** → `--metrics`
* **What performance and structural stats are reported** → `--stats`

The design prioritizes clarity, composability, scriptability, and safe handling of bad or broken input.

---

## 🟦 `--on-error <action>`

Controls how Kelora behaves when an error is encountered.

| Mode         | Behavior                                                                           |
| ------------ | ---------------------------------------------------------------------------------- |
| `abort`      | Stop immediately on first error; exit non-zero                                     |
| `skip`       | Silently discard bad input                                                         |
| `quarantine` | Process all lines, isolate broken events, expose only via `meta` to Rhai (default) |

**Short option:** `-x`
**Default:** `--on-error=quarantine`

---

## 🟨 `--error-report <style>[=file]`

Controls how errors are reported.

| Style     | Behavior                                     | Output Format | Output Location |
| --------- | -------------------------------------------- | ------------- | --------------- |
| `off`     | Suppress all error messages                  | —             | —               |
| `summary` | Group errors by type, show counts + examples | JSON          | stderr or file  |
| `print`   | Print each error immediately to stderr       | Plain text    | stderr or file  |

If `=file` is provided, output is written there; otherwise, defaults to stderr.

---

## 🔚 Exit Codes

| Error Type Encountered   | `--on-error=abort` | `--on-error=skip/quarantine` |
| ------------------------ | ------------------ | ---------------------------- |
| Fatal error (panic, I/O) | `exit(2)`          | `exit(2)`                    |
| Any other error          | `exit(1)`          | `exit(0)`                    |
| No errors                | `exit(0)`          | `exit(0)`                    |

---

## 📊 Internal Error Severities

| Severity | Examples                                  | Printed? | Triggers Exit?  |
| -------- | ----------------------------------------- | -------- | --------------- |
| Fatal    | I/O failure, panic                        | Always   | Yes (`exit(2)`) |
| Hard     | Rhai script error, CLI misuse, regex fail | Always   | Yes (`exit(1)`) |
| Medium   | Parse failure, CSV mismatch               | Optional | Yes (`exit(1)`) |
| Soft     | Missing field, null, coercion fail        | Optional | No              |

---

## 🧪 Quarantine Mode Details (`--on-error=quarantine`)

For input lines that fail to parse or decode:

* The main event object is **empty**.
* The following are injected into the Rhai `meta` namespace:

| `meta` Attribute    | Description                                      |
| ------------------- | ------------------------------------------------ |
| `meta.line`         | Raw input line                                   |
| `meta.line_number`  | Line number in input stream (if available)       |
| `meta.parse_error`  | Parse error message (for format-level failures)  |
| `meta.decode_error` | Input decode error message (e.g., invalid UTF-8) |

**Note:**

* No partially parsed fields are included (`partial` is deliberately omitted for simplicity and consistency).
* These broken events are **not** emitted to the CLI output stream unless the Rhai script explicitly re-emits them.

Example in Rhai:

```rhai
if meta.contains("parse_error") || meta.contains("decode_error") {
    track_count("bad_lines");
    false  // filter out by default
}
```

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

## 📦 `tracked` Variable (Rhai)

Exposes **full raw internal state** from `track_*()`:

* For `track_unique()`, this includes the **complete set** of unique values.
* For counts and maxes, it holds the raw numeric value.

Example:

```rhai
for user in tracked["users"] {
    print(user);
}
```

---

## 📈 `--stats`

Reports runtime and parsing statistics.

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

| Field        | Description                                                                                         |
| ------------ | --------------------------------------------------------------------------------------------------- |
| `lines_in`   | Input lines read                                                                                    |
| `lines_out`  | Events emitted after parsing/filtering                                                              |
| `duration`   | Total runtime, shown as `s`, `ms`                                                                   |
| `throughput` | Processing rate, e.g., `9.3k/s`                                                                     |
| `levels`     | Comma-separated log levels found (no spaces)                                                        |
| `keys`       | Comma-separated parsed field names (no spaces); designed for direct use with `--keys` or `--levels` |

---

## 🛡️ Section Headers

All non-event outputs use:

* Emoji-prefixed section headers:

  * 📊 === Kelora Metrics ===
  * ❌ === Kelora Errors ===
  * 📈 === Kelora Stats ===
* Printed to stderr by default.
* Suppressible with:

  ```bash
  --no-section-headers
  ```

---

## 📘 Rhai Scripting Integration

Kelora injects two variables into the Rhai scope:

| Variable  | Description                                   |
| --------- | --------------------------------------------- |
| `metrics` | Compact summary (counts, maxes, samples)      |
| `tracked` | Full internal tracking state (sets, raw data) |

---

## 🧪 Example CLI Usage

```bash
kelora logs.jsonl --metrics --stats

kelora logs.jsonl --error-report summary=errors.json

kelora -x abort logs.jsonl --metrics-file=metrics.json

kelora logs.jsonl --metrics --no-section-headers | grep latency

levels=$(kelora logs.jsonl --stats | grep '^levels' | cut -d= -f2)
kelora logs.jsonl --levels "$levels"
```

---

## 🧰 Configuration Example

```ini
[defaults]
on-error = quarantine
error-report = summary=errors.json
metrics = true
metrics-file = metrics.json
stats = true
```

---

## 📘 CLI Help Summary

```text
ERROR HANDLING

  -x, --on-error <action>         What to do when errors occur (default: quarantine)
                                    abort       Stop on first error (exit 1/2)
                                    skip        Silently discard bad input
                                    quarantine  Process all lines; expose broken lines only via meta for tracking

      --error-report <style>[=file]
                                  How to report errors
                                    off       Suppress all error messages
                                    summary   Grouped summary (default for quarantine)
                                    print     Print every error (default for abort)

METRICS AND STATS

      --metrics                   Show values tracked in Rhai via track_*()
      --metrics-file <path>      Write metrics to file (JSON format)

      --stats                    Show performance statistics and log field info

OUTPUT FORMAT CONTROL

      --no-section-headers       Suppress emoji + section headers from stderr output
```

---


## 📊 Summary of Combinations

| `--on-error` | `--error-report` Options            | Recommended Defaults + Notes                                                                                                                                  |
| ------------ | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `abort`      | `print` (default), `summary`, `off` | ✅ Default `print` — show fatal reason before exit. <br>✅ Allow `summary` for grouped pre-exit info.<br>⚠️ `off` dangerous; user gets no explanation on abort. |
| `skip`       | `off` (default), `summary`, `print` | ✅ Default `off` — skip silently.<br>✅ Allow `summary` for final counts.<br>✅ Allow `print` for live tracking.                                                 |
| `quarantine` | `summary` (default), `print`, `off` | ✅ Default `summary` — see grouped error info.<br>✅ Allow `print` to watch live quarantine events.<br>✅ Allow `off` for silent run + only Rhai handling.       |

---

## 🔍 Combination Analysis

### ✅ Good combinations

* `abort + print` → stops + shows last error
* `abort + summary` → stops + shows grouped summary
* `skip + off` → fastest, silent
* `skip + summary` → skip but summarize what happened
* `quarantine + summary` → analyze all, summarize errors
* `quarantine + print` → analyze all, live-track errors

---

### ⚠️ Risky or odd combinations (but still allowed)

* `abort + off` → stop on error but no output why; only for CI scripts checking exit codes
* `skip + print` → might overwhelm user with skipped line reports, but technically allowed

---

### ❌ Disallowed combinations (recommend blocking or warning)

None strictly need to be blocked, but **clearly document**:

> "`--on-error abort` with `--error-report off` will abort without printing any error reason."

---

## 🧠 Recommended Defaults Table

| `--on-error` | Default `--error-report` |
| ------------ | ------------------------ |
| `abort`      | `print`                  |
| `skip`       | `off`                    |
| `quarantine` | `summary`                |

---

## 💬 CLI Help Summary

```
By default:
  abort        → error-report=print
  skip         → error-report=off
  quarantine   → error-report=summary

All combinations are allowed, but note:
  abort + off → aborts silently (no printed error)
  skip + print → skips lines but prints every skip event
```

