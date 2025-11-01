# Pitch
Kelora is a programmable, scriptable log processor built for real-world logs, fast pipelines, and complete control.

It treats logs as structured data — not just text — and gives you full control over filtering, transforming, and analyzing them using the Rhai scripting language. Designed for CLI-first workflows and composability, Kelora works with JSON, logfmt, syslog, and other formats, combining parallel processing, stateful spans/windows, streaming metrics, and a resilient error model that handles messy data. It excels where traditional tools fall short: CI pipelines, unpredictable production logs, and ad hoc forensic work. It's not a log viewer or shipper — it's a processing tool for turning diverse logs into structured, analyzable data.

Use it when you want to:
	•	Transform messy, inconsistent logs into structured events
	•	Enrich events with tracking functions and sliding-window context
	•	Partition streams with spans and ship stats alongside the data
	•	Finish with log-native outputs — JSON, CSV, logfmt, level maps, and more

# README Header

Kelora is a programmable, scriptable log processor built for real-world logs, fast pipelines, and complete control.

It turns messy, diverse logs into structured events and lets you filter, transform, and analyze them using powerful Rhai scripts. Designed for CLI-first workflows and composability, Kelora works with JSON, logfmt, syslog, and other formats, pairing a resilient runtime with stateful spans/windows and tracker-powered metrics — making it perfect for pipelines, automation, and forensic work.

# Man page

Kelora is a stream-oriented log processor that reads structured or semi-structured logs from stdin or files, transforms them using Rhai scripts, and writes filtered or enriched output.
It is designed to be composable with other CLI tools, offering high-performance filtering, tracking functions, span/window hooks, metrics export, and formatting for JSON, logfmt, syslog, level maps, and custom formats with robust error handling for real-world data.

# Taglines

Kelora is a scriptable log processor for real-world logs.
Designed for pipelines, CI, and fast triage with spans and metrics. One-liners in Rhai.

Structured, stateful log processing — from stdin to insight.
Kelora filters, transforms, and analyzes logs using clean CLI pipelines, embedded scripting, and tracker-powered metrics.

# Elevator Pitch / Why?

Most tools treat logs as text. Kelora treats them as structured data.
With support for multiple formats, tracking functions, span/window orchestration, parallel execution, embedded scripting, and a resilient error handling model that gracefully handles real-world messiness, Kelora bridges the gap between simple pattern matching tools and complex observability stacks — without the bloat.
It's perfect for local debugging, CI pipelines, forensic analysis, and anywhere you need complete control over log processing.

# Tweet
Logs are data, not text.
Kelora is a programmable log processor for your terminal.
Transform messy, real-world logs into structured events using CLI pipelines, embedded Rhai scripts, stateful spans, and streaming metrics.
🛠️ https://github.com/dloss/kelora #logs #cli #observability
