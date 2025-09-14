# Kelora Vision & Design

Kelora is a **programmable, scriptable log processor** built for real-world logs, fast pipelines, and complete control. It is a **CLI-first tool** for turning messy, diverse, real-world logs into structured, analyzable data — with scripting, formatting, and logic *you* own.

This document defines what Kelora is, what it is not, its design principles, settled decisions, and how to keep it lean, focused, and powerful.

---

## ✅ WHAT KELORA IS

* **CLI tool** for transforming logs into structured events
* Designed to filter, mutate, and format logs using [Rhai](https://rhai.rs/)
* Supports line-oriented input formats (JSON, logfmt, syslog, raw lines, etc.)
* Works as part of UNIX pipelines — stdin in, stdout out
* Supports **stateful processing** with built-in tracking capabilities
* Enables **real-time or batch streaming** with selectable execution modes
* **Scriptable, composable, predictable** — always

---

## ❌ WHAT KELORA IS NOT

| Not a…           | Why not                                                            | Use instead     |
| ---------------- | ------------------------------------------------------------------ | --------------- |
| Log viewer       | Use for interactive exploration                                    | `lnav`, `less`  |
| Log shipper      | Use for ingestion and transport                                    | `fluentbit`, `vector`, `filebeat` |
| Dashboard        | Use for storage/visualization                                      | Grafana, Kibana, Loki |
| Metrics DB       | Use for time-series storage                                        | Prometheus, ClickHouse, InfluxDB |
| Regex toolkit    | Use for simple pattern matching                                    | `grep`, `ripgrep`, `angle-grinder` |
| Universal parser | Kelora is explicit — you configure formats, not guess them blindly | |
| DSL playground   | Rhai is a tool, not the core of your architecture                 | |
| Full-text search | Use for content search                                             | `ripgrep` |
| Interactive log browsing | Use for exploration | `lnav` |
| Cross-host log aggregation | Use for centralized logging | `Loki` |
| Persistent log indexing | Use for searchable storage | `Elasticsearch` |
| Visualization | Use for charts and graphs | `Grafana` |

---

## 🔒 RUTHLESS SCOPE DISCIPLINE

### ❌ ABSOLUTE NOs

* **No UI**: This is a CLI tool. No TUI, no terminal browser.
* **No plugin system**: No dynamic plugins, no `.so`, no WASM. Use `--exec`.
* **No context-aware input formats**: No YAML, XML, GELF, etc.
* **No magic behavior**: No format guessing unless explicitly requested.
* **No automatic retries or fuzzy logic**: Parsing either works or fails, cleanly.
* **No persistence or state files**: No temp DBs, no checkpoints. Stream in, stream out.
* **No silent error suppression**: Unless user opts in, failures are visible.
* **No auto-coercion of types**: Leads to silent failures; better to be explicit
* **No implicit fan-out from arrays**: Too magical and error-prone
* **No implicit flattening**: Controlled flattening via helper functions only
* **No plugin system or extensions**: Overkill; instead expose composable functions via Rhai

---

## ⚠️ ALLOWED WITH GUARDRAILS

### ✅ Multiline Logs via a Chunking Preprocessor

* Only via explicit multiline configuration
* Implemented as a preprocessing stage
* Emits one block of lines per event
* No internal understanding of stacktraces — *you* parse if needed

### ✅ Timestamp Parsing

* Auto-parses from a whitelist of known formats
* Never guesses or infers types
* Users can override with custom timestamp field and format specifications
* Timestamps are parsed once per event and memoized

### ✅ Format Guessing

* Allowed only with explicit auto-detection mode
* Uses fast, deterministic heuristics (fallback to line)
* One format per stream. Mixed formats are explicitly unsupported

---

## 🧠 SETTLED DESIGN PRINCIPLES

| Area | Decision / Philosophy |
|------|----------------------|
| **Core Identity** | CLI tool for processing structured logs (not a viewer, shipper, or platform) |
| **Event Model** | Each log line becomes an Event with structured, typed fields |
| **Special Fields** | `ts`, `level`, `msg` are promoted/normalized during parsing, not afterwards |
| **Field Typing** | Default to String; explicit type conversion with safe coercion helpers |
| **Input Formats** | JSON, logfmt, syslog, and others. Flexible user-defined formats with Rhai scripting |
| **Emit/Fan-out** | Array elements can be emitted as separate events (suppresses original by default) |
| **Flattening** | Path-based access with optional flattening using dot+bracket syntax |
| **Error Handling** | Resilient by default with context-specific recovery; strict mode for fail-fast |
| **Script Scope** | Direct field access with fallback to bracket notation for complex keys |
| **Parallelism** | Configurable parallel processing with ordering and batching controls |
| **Output Formats** | Multiple formats supported: default, JSON, logfmt, CSV, etc. |
| **Type Coercion** | Explicit only; no auto-coercion of fields |
| **Fan-out** | Array fan-out functionality for processing individual elements |
| **Field Access Style** | Direct field access and path-based access for nested values with safety helpers |
| **Script Safety** | Robust variable handling and error recovery in Rhai execution |

---

## 🧱 EVENT MODEL

Each event consists of:

* **`fields`**: `IndexMap<String, FieldValue>` — preserves log field order
* **`ts`, `level`, `msg`**: Special fields used internally
* **`meta`**: Internal tracking or derived values (not exposed unless needed)

**Field values may be:**
* String
* Number
* Bool
* Array
* Null

**Logs are data, not text**. Treat each line as a structured record.

---

## 🧪 RESILIENCY MODEL

* **Resilient Mode (default)**: Skip errors, continue processing, show summary at end
* **Strict Mode**: Fail-fast on any error with immediate error display
* Errors go to stderr, not mixed into stdout
* Context-specific error handling optimized for each stage

### 🔀 Context-Specific Behavior:

**Input Parsing:**
- **Resilient**: Skip unparseable lines automatically, continue processing
- **Strict**: Abort on first parsing error

**Filtering (`--filter` expressions):**
- **Resilient**: Filter errors evaluate to false (event is skipped)
- **Strict**: Filter errors abort processing

**Transformations (`--exec` expressions):**
- **Resilient**: Atomic execution with rollback - failed transformations return original event unchanged
- **Strict**: Transformation errors abort processing

**Default behavior must be resilient**. Robust error recovery with visibility.

---

## 🔧 CLI PRINCIPLES

* **Each flag should be orthogonal and predictable**
* **Prefer composability over configurability**
* **Don't auto-correct or auto-infer user mistakes**
* **Fail fast, or fail cleanly**
* **Errors go to stderr; valid logs to stdout**
* **Flags should support scripting use cases**
* **One job per flag**. Don't overload behavior.
* **One format per stream**. Don't guess midstream.
* **Errors must be seen**. But not mixed with valid output.
* **Fast enough to use in CI**. But correct enough to trust in prod.

---

## 🧱 DISTINCTIVE TRAITS VS. OTHER TOOLS

| Tool | Kelora Is… |
|------|------------|
| **jq** | More structured, stateful, supports multiline, real scripting |
| **awk** | Safer, saner, and field-aware — built for logs, not CSVs |
| **lnav** | Not interactive — scriptable, batch-oriented, composable in pipelines |
| **angle-grinder** | More flexible due to Rhai, chunking, and tracked state |
| **Loki / Vector** | Not a log shipper — Kelora is a processing tool, not a system |

---

## 📍 DEVELOPER PREFERENCES & PHILOSOPHY

* ✅ **Value clarity, minimalism, and control**
* ✅ **Tolerate complexity internally to provide clean, predictable behavior externally**
* ✅ **Prioritize CLI ergonomics and scriptable UX**
* ✅ **Prefer building blocks over opinionated automation**
* ✅ **Design for untrusted, inconsistent input** (e.g. malformed fields, bad types)
* ✅ **Have learned from previous project (klp) and want to avoid its feature creep**

---

## ✨ KELORA IS GREAT FOR…

* **CI pipelines** with structured logs
* **Converting logfmt → JSON** (or vice versa)
* **Quick local forensic work**
* **One-liner data pipelines**
* **In-place field enrichment**
* **Filtering complex logs with business logic**

---

## 📌 FINAL GUIDING PRINCIPLES

* **Logs are data, not text**. Treat each line as a structured record.
* **One job per flag**. Don't overload behavior.
* **One format per stream**. Don't guess midstream.
* **Errors must be seen**. But not mixed with valid output.
* **Default behavior must be resilient**. Robust error recovery with visibility.
* **Fast enough to use in CI**. But correct enough to trust in prod.
* **Scriptable, composable, predictable**. Always.

---

## ✨ SUMMARY TAGLINE

**Kelora is a scriptable log processor for real-world logs.**

Designed for pipelines, CI, and fast triage. One-liners in Rhai. Structured in, structured out. Nothing more — and nothing less.