# **Kelora Integration Guide**

*Composing with the right tools makes logs sing.*

Kelora is the log scalpel — it shapes raw text into structured events.
Once logs are clean, other tools can filter, analyze, or visualize them with elegance.
This guide shows how Kelora pairs with a handful of timeless allies that share its spirit:
small, sharp, and scriptable.

---

## 🧩 Core Idea

**Kelora normalizes. Others analyze.**
Use it to turn noise into structure — JSONL, TSV, or logfmt — then pipe the result onward.
Every tool below does one thing brilliantly.

---

## ⚙️ The Essential Five

### **[jq](https://jqlang.github.io/jq/) / [jaq](https://github.com/01mf02/jaq) — Deep JSON slicing**

Kelora emits JSONL; jq dissects it.

```bash
kelora -f combined -F json --filter 'e.status >= 500' \
| jq -r '. | [.ts, .path, .status] | @tsv'
```

* `jq`’s power meets Kelora’s discipline — no malformed JSON, ever.
* `jaq` is a Rust rewrite that’s orders faster; perfect for big logs.

💡 *Bonus*: jq can stream-parse gigabytes; Kelora’s `--parallel` keeps up.

---

### **[qsv](https://github.com/jqnatividad/qsv) — Lightning CSV analytics**

For top-Ns, counts, or quick histograms:

```bash
kelora -f json -F csv --keys ts,service,status \
| qsv frequency --column service --limit 10
```

* qsv runs Rust-fast and memory-light.
* Combine with `--stats` or `--metrics` for instant dashboards without dashboards.

💡 *Tip*: use `kelora --exec 'e.day = e.ts.format("%F")'` to enrich timestamps for time-based grouping.

---

### **[VisiData](https://www.visidata.org/) — Interactive spelunking**

Turn structured output into an instant spreadsheet:

```bash
kelora -f logfmt -F tsv --keys ts,level,msg | vd -
```

* Sort, pivot, graph — all inside your terminal.
* Streams stay live: press `g#` for group counts, `/` for filtering, or `Shift+G` for plots.

💡 *Pair with*: `--window` in Kelora to feed contextual streams into VisiData for exploration.

---

### **[spyql](https://github.com/dcmoura/spyql) — SQL on streams**

SQL aggregation, no database needed:

```bash
kelora -j -F json --keys ts,user,action \
| spyql -O table "SELECT user, COUNT(*) c FROM input GROUP BY user ORDER BY c DESC"
```

* Feels like SQLite, runs like awk.
* Works perfectly with `kelora --exec` transformations.

💡 *Mix-in*: `emit_each()` fan-outs from arrays, turning nested JSON into rows for SpyQL to count.

---

### **[rare](https://github.com/zix99/rare) — Regex histograms**

Quick regex-driven insights:

```bash
kelora -f syslog -k msg | rare -r 'error|warn|timeout'
```

* Shows dominant patterns — perfect for first-look diagnostics.
* Add `--since`/`--until` to narrow the temporal window.

💡 *Pro tip*: Pipe structured Kelora fields into `rare -r` for statistical debugging.

---

## 🏗️ Unix Classics — The Original Power Tools

Kelora speaks fluent POSIX.

### **awk — Stream arithmetic**

```bash
kelora -F tsv --keys ts,service,status \
| awk -F'\t' '$3 >= 500 {print $1, $2, $3}'
```

* For quick ratios, deltas, or counters — awk still rules.
* Kelora’s clean TSV guarantees no quoting nightmares.

💡 *Enrich first*: `--exec 'e.delta = e.end.to_int() - e.start.to_int()'` then post-process in awk.

---

### **cut / sort / uniq — The timeless trio**

```bash
kelora -F tsv --keys service \
| cut -f1 | sort | uniq -c | sort -nr | head
```

* Minimal, composable, transparent.
* Kelora’s field normalization makes these safe and predictable.

💡 *Trick*: `kelora -F tsv --keys ip | sort | uniq -c` gives quick top IPs from any format.

---

### **sqlite3 — Instant structured DB**

```bash
kelora -F tsv --keys ts,service,status > events.tsv
sqlite3 :memory: <<'SQL'
.mode tabs
.import events.tsv events
SELECT service, COUNT(*) FROM events GROUP BY service ORDER BY 2 DESC;
SQL
```

* Treat logs as tables; run ad-hoc joins or time filters.
* SQLite loves Kelora’s TSV output — no schema headaches.

💡 *Hack*: combine with `kelora --exec 'e.day = e.ts.format("%F")'` for date-based rollups.

---

## 🧭 Interactive Explorers — Watch Logs Breathe

### **[lnav](https://lnav.org/) — The Living Log Viewer**

```bash
kelora -f json -F json app.log | lnav -i json
```

* lnav auto-detects timestamps, builds timelines, and runs SQL.
* Kelora feeds it pristine structure — lnav handles the live browsing.

💡 Run inside lnav:

```
;SELECT level, COUNT(*) FROM log GROUP BY level;
```

---

### **[klogg](https://klogg.filimonov.dev/) — GUI grep for giants**

```bash
kelora -f syslog --filter 'e.level == "error"' > errors.log
klogg errors.log
```

* Opens terabyte logs instantly.
* Use it when `less` starts to sweat.

💡 Combine with `--pretty-ts` for readable time context in visual searches.

---

### **[tailspin](https://github.com/bensadeh/tailspin) — Streaming color**

```bash
kelora -f combined | tailspin
```

* Colorized, leveled tails for real-time debugging.
* Matches Kelora’s `--realtime` mode perfectly.

---

### **[Benthos](https://www.benthos.dev/) — Stream orchestration**

```bash
benthos -c benthos.yaml | kelora -f json --exec 'e.env = "prod"' -F json
```

* Benthos handles I/O, Kelora handles semantics.
* Together they form a declarative, testable log refinery.

---

## 🔄 Model Workflows

### From chaos to clarity

```bash
tail -f /var/log/nginx/access.log \
| kelora -f combined --filter 'e.status >= 500' \
  --keys ts,path,status -F csv \
| qsv frequency -c path --limit 20
```

Or interactively:

```bash
kelora -f json -F tsv --keys ts,level,msg app.log | vd -
```

Or classically:

```bash
kelora -f logfmt -F tsv --keys level | cut -f1 | sort | uniq -c
```

**Kelora parses and filters — everything else explores and aggregates.**

---

## 🌐 Inbound & Outbound

**Inbound:** raw logs, JSONL, syslog, logfmt, csv/tsv, gzipped streams.
→ Preprocessors like [`jc`](https://github.com/kellyjonbrazil/jc) (for shell command output) or [`evtx2json`](https://github.com/omerbenamram/evtx) (for Windows event logs) slot right in.

**Outbound:** `-F json`, `-F csv`, or `-F tsv` depending on what follows.
Use `--keys`, `--filter`, and `--exec` to shape before export.

---

## 🧘 Philosophy Recap

Kelora doesn’t compete — it completes.
It’s the prelude to every great analysis: clean in, clean out.
Logs, refined to truth.
Then — pass them on.

---

Would you like me to add a short “**Integration Recipes Appendix**” next — e.g. *“Top IPs in 10 ways”* showing the same goal solved with `awk`, `qsv`, `jq`, `sqlite3`, etc.? That would turn this into a practical companion for advanced users.
