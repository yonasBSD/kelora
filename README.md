# Kelora

Kelora is a programmable, scriptable CLI tool for turning messy, real-world logs into structured, analyzable data. It’s designed for fast pipelines, complete control, and logic you own — not a log viewer, not a dashboard, not a black box.

---

## 🚀 Try It in One Line

```bash
# Filter any log file (default line format)
cat /var/log/syslog | kelora --filter 'line.matches("ERROR|WARN")'

# Parse syslog format (RFC3164/RFC5424)
cat /var/log/syslog | kelora -f syslog --filter 'severity <= 3'

# Filter structured logs with Rhai
cat logs.jsonl | kelora -f jsonl --filter 'status >= 400'

# Enrich and transform fields
kelora -f jsonl --exec 'let sev = if status >= 500 { "crit" } else { "warn" };' logs.jsonl

# Track max value across the stream
kelora -f jsonl \
  --exec 'track_max("max", duration_ms)' \
  --end 'print(`Max: ${tracked["max"]}`)' logs.jsonl

# Real-time Kubernetes logs
kubectl logs app | kelora -f jsonl --filter 'level == "error"' -F text
```

---

## ⚙️ What It Is

* A CLI tool for structured log transformation
* Designed for UNIX-style pipelines — stdin in, stdout out
* Supports JSON, logfmt, syslog, and raw lines
* Uses [Rhai](https://rhai.rs/), a simple JavaScript-like language, to filter, mutate, and analyze logs
* Includes built-in global state tracking (`track_*`)
* Supports parallel and streaming modes

---

## 📃 Rhai Primer

Rhai is a tiny, embeddable scripting language built for Rust. Kelora uses it to let you embed logic directly into log pipelines — with no external runtime.

```rhai
// Conditional tagging
let sev = if status >= 500 { "crit" } else { "warn" };

// Global counters or stats
track_count("errors");
track_max("max_duration", duration_ms);
```

Available variables:

* `event` — parsed field map
* `line` — original line text
* `tracked` — global metrics state
* `meta.linenum` — current line number

---

## 📊 What It’s Great For

* Filtering and enriching logs in CI pipelines
* Transforming logfmt ⇄ JSON
* Real-time `kubectl logs` processing
* Streaming one-liner data pipelines
* Field selection, tagging, and global stats

---

## 🕵️ What It’s Not

| Task                     | Use Instead                                                               |
| ------------------------ | ------------------------------------------------------------------------- |
| Browsing logs            | [lnav](https://lnav.org/)                                                 |
| Multi-host log ingestion | [Loki](https://grafana.com/oss/loki/), [fluentbit](https://fluentbit.io/) |
| Full-text search         | [ripgrep](https://github.com/BurntSushi/ripgrep)                          |
| JSON-only transformation | [jq](https://jqlang.org/)                                                 |
| Regex-heavy pipelines    | [angle-grinder](https://github.com/rcoh/angle-grinder)                    |
| Dashboards and alerting  | [Grafana](https://grafana.com/), [Kibana](https://www.elastic.co/kibana/) |

---

## ✏️ Installation

```bash
git clone https://github.com/dloss/kelora.git
cd kelora
cargo build --release
```

---

## ✈️ CLI Overview

| Flag            | Purpose                                      |
| --------------- | ---------------------------------------- |
| `-f`            | Input format: `jsonl`, `logfmt`, `syslog`, `line` (default) |
| `-F`            | Output format: `json`, `text`, `csv`   |
| `--filter`      | Rhai filter expression (repeatable)    |
| `--exec`        | Rhai exec scripts (repeatable)         |
| `--begin/--end` | Logic before/after stream              |
| `--on-error`    | Strategy: skip, print, abort, stub |
| `--parallel`    | Enable parallel batch mode             |
| `--unordered`   | Drop output order for performance      |

---

## 🔖 Philosophy

* Logs are **data**, not text
* Be **explicit** — no guessing
* Fail **visibly** — don’t drop data silently
* CLI-first. Scriptable. Composable.
* One format per stream. One job per flag.

---

## ✍️ License

MIT License — see [LICENSE](LICENSE)
