# Pitch 
Kelora is a fast, scriptable, stream-oriented log processor for the command line.

It treats logs as structured events — not just lines of text — and gives you full control over filtering, transforming, and analyzing them using the Rhai scripting language. Designed for composability and clarity, Kelora works with common formats like JSON and logfmt, supports powerful batch and parallel processing, and features a robust resiliency model that gracefully handles malformed data. It excels in environments where traditional tools fall short: CI pipelines, messy logs, and ad hoc forensic work. It's not a log viewer or a log shipper — it's a log shaper.

Use it when you want to:
	•	Normalize inconsistent logs
	•	Enrich events with derived fields
	•	Extract insights from raw streams
	•	Do real processing — not just grepping

# README Header

Kelora is a fast, scriptable log processor for the command line.

It turns raw logs into structured events and lets you filter, transform, and analyze them using simple, powerful Rhai scripts.
Designed for composability and clarity, Kelora works with JSON, logfmt, and other formats with a robust resiliency model that gracefully handles malformed data — making it a perfect tool for pipelines, automation, and log forensics.

# Man page

Kelora is a stream-oriented log processor that reads structured or semi-structured logs from stdin or files, transforms them using Rhai scripts, and writes filtered or enriched output.
It is designed to be composable with other CLI tools, offering high-performance filtering, aggregation, and formatting for JSON, logfmt, and custom formats.

# Taglines

Kelora is a scriptable log processor for real-world logs.
Designed for pipelines, CI, and fast triage. One-liners in Rhai. 

Structured, scriptable log processing — from stdin to insight.
Kelora filters, transforms, and analyzes logs using clean CLI pipelines and embedded scripting.

# Elevator Pitch / Why?

Most tools treat logs as text. Kelora treats them as data.
With support for structured formats, custom field logic, batch processing, built-in scripting, and a resilient error handling model, Kelora bridges the gap between grep and full-blown observability stacks — without the bloat.
It's perfect for local debugging, CI pipelines, forensic audits, and anything in between.

# Tweet
Logs are data.
Kelora is a scriptable log processor for your terminal.
Filter, transform, and analyze JSON/logfmt logs using clean CLI pipelines, embedded scripts, and resilient error handling.
🛠️ https://github.com/dloss/kelora #logs #cli #observability