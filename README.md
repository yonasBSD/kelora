# Kelora

Kelora is a fast, scriptable CLI log processor built for real-world logs and clean data pipelines. It reads raw or structured logs (JSON, syslog, logfmt, CSV, etc.), and uses Rhai scripts to filter, enrich, and analyze them — all from the terminal.

---

## 🔧 Quick Start

```bash
# Filter warnings and errors from a structured JSON log
kelora -f jsonl app.log --levels warn,error

# System logs (RHEL/Fedora: /var/log/messages; Ubuntu: /var/log/syslog)
kelora -f syslog /var/log/messages --levels warn,error
# OR
kelora -f syslog /var/log/syslog --levels warn,error

# Extract slow HTTP requests
kelora -f jsonl access.log \
  --filter 'response_time.to_int() > 1000' \
  --keys timestamp,method,path,response_time

# Add a derived status class and export as CSV
kelora -f jsonl app.log \
  --exec 'let class = status_class(status)' \
  --keys timestamp,status,class -F csv

# Process compressed logs and filter server errors
kelora -f jsonl logs/app.log.1.gz \
  --filter 'status.to_int() >= 500'

# Detect repeated login failures using a sliding window
kelora -f jsonl auth.log --window 5 \
  --filter 'event_type == "login_failed"' \
  --exec 'let failures = window_values(window, "username").len(); if failures >= 3 { print("Suspicious activity") }'

# Real-time log triage from Kubernetes
kubectl logs -f app | kelora -f jsonl --levels warn,error
```

---

## 💡 Key Features

- **Rhai scripting**: powerful and readable one-liners
- **Window analysis**: look at past N events with `--window`
- **Flexible formats**: JSON, logfmt, syslog, CEF, CSV, TSV, raw lines
- **Real-time capable**: stream from `tail`, `kubectl logs`, stdin
- **Compressed input**: automatic `.gz` decompression
- **Multi-file support**: process logs in CLI, alphabetical, or mtime order
- **Parallel mode**: scale up with `--parallel`, `--threads`, `--unordered`

---

## ✍️ Rhai Snippets

```rhai
// Filtering
level == "error" && user != "system"

// Enrichment
let sev = if status >= 500 { "crit" } else { "info" };

// Global metrics
track_count("errors");
track_max("max_duration", duration_ms);

// Window-based logic (with --window N)
let recent = window_values(window, "status");
let changed = window.len() > 1 && window[1].status != status;

// Regex, strings, JSON
let user = line.extract_re("user=(\w+)");
let ip = line.extract_ip();
let data = parse_json(line);
```

Built-in objects:
- `line` – raw line
- `event` – parsed fields
- `meta.linenum` – line number
- `tracked` – global state
- `window` – recent events (if `--window` is used)

---

## 🏗️ Common Patterns

```bash
# Filter and extract fields
kelora -f jsonl logs.jsonl --filter 'status >= 500' --keys ts,level,msg

# Format output
kelora -f jsonl app.log -F csv --keys user,status,duration_ms
kelora -f jsonl app.log -F logfmt --core --brief

# Use aliases (from config)
kelora -a errors logs.jsonl
```

---

## 📦 Install

```bash
git clone https://github.com/dloss/kelora
cd kelora
cargo install --path .
```

---

## 📚 Help & Options

```bash
kelora --help         # Show CLI help
kelora --show-config  # Show config file and aliases
```

### Example flags:

| Flag         | Purpose                                      |
| ------------ | -------------------------------------------- |
| `-f`         | Input format (`jsonl`, `line`, `syslog`, …) |
| `-F`         | Output format (`jsonl`, `csv`, `logfmt`, …) |
| `--filter`   | Rhai expression to include events            |
| `--exec`     | Rhai script to transform events              |
| `--window N` | Enable N+1 sliding event window              |
| `--summary`  | Show tracked key/value table                 |
| `--stats`    | Show line counts and performance stats       |
| `--on-error` | How to handle bad lines (`print`, `skip`, …) |

---

## 🕵️ Not a Replacement For

| Task                | Use Instead                          |
| ------------------- | ------------------------------------- |
| Log browsing        | `lnav`                                |
| Full-text search    | `ripgrep`                             |
| Dashboards          | `Grafana`, `Kibana`                   |
| Log ingestion       | `fluentbit`, `vector`, `loki`         |
| JSON-only pipelines | `jq`                                  |
| Regex pipelines     | `angle-grinder`                       |

---

## 📄 License

MIT — see [LICENSE](LICENSE)
