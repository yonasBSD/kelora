# Kelora 

Kelora is a programmable, scriptable log processor built for real-world logs, fast pipelines, and complete control.

It is **not** a log viewer. It is **not** a dashboard. It is **not** a metrics aggregator.
It is a **CLI-first tool** for turning messy, diverse, real-world logs into structured, analyzable data — with scripting, formatting, and logic *you* own.

This document defines what Kelora is, what it is not, and how to keep it lean, focused, and powerful.

---

## ✅ WHAT KELORA IS

* A CLI tool for transforming logs into structured events
* Designed to filter, mutate, and format logs using [Rhai](https://rhai.rs/)
* Supports line-oriented input formats (e.g. JSON, logfmt, raw lines)
* Works as part of UNIX pipelines — stdin in, stdout out
* Supports stateful processing via built-in `track_*` functions
* Enables real-time or batch streaming with selectable execution modes

---

## ❌ WHAT KELORA IS NOT

| Not a…           | Why not                                                            |
| ---------------- | ------------------------------------------------------------------ |
| Log viewer       | Use `lnav` or `less` for interactive exploration                   |
| Log shipper      | Use `fluentbit`, `vector`, or `filebeat` for ingestion             |
| Dashboard        | Use Grafana, Kibana, or Loki for storage/visualization             |
| Metrics DB       | Use Prometheus, ClickHouse, or InfluxDB                            |
| Regex toolkit    | Use `grep`, `ripgrep`, or `angle-grinder` for that                 |
| Universal parser | Kelora is explicit — you configure formats, not guess them blindly |
| DSL playground   | Rhai is a tool, not the core of your architecture                  |

---

## 🔒 RUTHLESS SCOPE DISCIPLINE

### ❌ ABSOLUTE NOs

* No UI: This is a CLI tool. No TUI, no terminal browser.
* No plugin system: No dynamic plugins, no `.so`, no WASM. Use `--eval`.
* No multiline parsers: Use a chunking preprocessor. Kelora won’t understand stacktraces — it will pass them whole.
* No context-aware input formats: No YAML, XML, GELF, etc.
* No magic behavior: No format guessing unless explicitly requested.
* No automatic retries or fuzzy logic: Parsing either works or fails, cleanly.
* No persistence or state files: No temp DBs, no checkpoints. Stream in, stream out.
* No silent error suppression: Unless user opts in, failures are visible.

---

## ⚠️ ALLOWED WITH GUARDRAILS

### ✅ Multiline Logs via Chunking

* Only via explicit `--chunker` strategy
* Implemented as a preprocessing stage
* Emits one block of lines per event
* No internal understanding of stacktraces — *you* parse if needed

### ✅ Timestamp Parsing

* Auto-parses from a whitelist of known formats
* Never guesses or infers types
* Users can override with `--timestamp-field` and `--timestamp-format`
* Timestamps are parsed once per event and memoized

### ✅ Format Guessing

* Allowed only with `--format auto`
* Uses fast, deterministic heuristics (JSON, logfmt, fallback to line)
* One format per stream. Mixed formats are explicitly unsupported

---

## 🧱 EVENT MODEL

Each event consists of:

* `fields`: `IndexMap<String, FieldValue>` — preserves log field order
* `ts`, `level`, `msg`: Special fields used internally
* `meta`: Internal tracking or derived values (not exposed unless needed)

Field values may be:

* String
* Number
* Bool
* Array
* Null

---

## 🧪 ERROR HANDLING STRATEGY

* Errors go to stderr, not mixed into stdout
* Summary of errors is printed at the end, unless `--errors abort` is used
* Exit code is 0, unless `--errors abort` is used

### 🔀 Available Modes:

- `collect` (default): Store error message (up to 5), continue processing, and show the collected errors at the end
- `print`: Log immediately to stderr, continue processing. 
- `debug`: Emit additional backtraces/context info for dev/debug use
- `skip`: Silently drop failed lines. Show summary/error stats 
- `abort`: Log immediately and abort on first error. No summary.

Maybe:

- `replace`: Replace missing/invalid fields with null/`""`/`0`

---

## 🔧 CLI PRINCIPLES

* Each flag must be orthogonal and predictable
* Prefer composability over configurability
* Don’t auto-correct or auto-infer user mistakes
* Fail fast, or fail cleanly
* Errors go to stderr; valid logs to stdout
* Flags should support scripting use cases

---

## ✨ KELORA IS GREAT FOR…

* CI pipelines with structured logs
* Converting logfmt → JSON (or vice versa)
* Quick local forensic work
* One-liner data pipelines
* In-place field enrichment
* Filtering complex logs with business logic

---

## 🚫 KELORA IS NOT FOR…

| Task                       | Use instead     |
| -------------------------- | --------------- |
| Full-text search           | `ripgrep`       |
| Interactive log browsing   | `lnav`          |
| Cross-host log aggregation | `Loki`          |
| Persistent log indexing    | `Elasticsearch` |
| Visualization              | `Grafana`       |

---

## 📌 FINAL GUIDING PRINCIPLES

* Logs are data, not text. Treat each line as a structured record.
* One job per flag. Don’t overload behavior.
* One format per stream. Don’t guess midstream.
* Errors must be seen. But not mixed with valid output.
* Default behavior must be safe. Never silently discard data.
* Fast enough to use in CI. But correct enough to trust in prod.
* Scriptable, composable, predictable. Always.
