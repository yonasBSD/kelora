✅ Kelora in a Nutshell

Kelora is a fast, scriptable, CLI-native log processor. It turns messy, real-world logs into structured events, lets users filter, enrich, and analyze them using Rhai, and works great in pipelines, CI, and batch modes — all without depending on servers or dashboards.

⸻

🧠 Settled Design Principles

Area	            Decision / Philosophy
Core Identity	    CLI tool for processing structured logs (not a viewer, shipper, or platform)
Event Model	        Each log line becomes an Event with typed IndexMap<String, FieldValue>
Special Fields	    ts, level, msg are promoted/normalized during parsing, not afterwards
Field Typing	    Default to String; Rhai’s .to_*() + optional try_*() wrappers for safe coercion
Input Formats	    JSON, logfmt, and flexible user-defined formats via -f line + Rhai
Multiline Support	✅ Needed; planned via pluggable chunkers (inspired by Stelp strategies)
Emit/Fan-out	    ✅ Supported via emit_each() (suppresses original event by default)
Flattening	        get_path() + optional flattening with dot+bracket syntax (user.roles[0])
Error Handling	    Default: emit-errors; other modes: fail-fast, skip, default-value
Script Scope	    Inject valid keys as variables, fallback to event["non_ident"], allow --script
Parallelism	        Controlled via --serial, --unordered, --realtime, --batch-size, etc.
Output Formats	    Minimal and clean: default, JSON, logfmt (only default formatter does styling)
Type Coercion       Explicit only; no auto-coercion of fields
Fan-outg	        emit_each() accepted as best name (alternatives evaluated)
Field Access Style	event["key"], get_path("a.b[0].c") for deep/nested values
Strict Vars Mode	Plan to use Engine::set_strict_variables(true) for robustness

⸻

🔧 Implemented or Planned Features
	•	✅ try_*() Rhai helpers for ergonomic field coercion
	•	✅ Multiline chunkers based on indentation, regex, or date prefixes
	•	✅ emit_each() with clear suppress + side-effect semantics
	•	✅ Formatter cleanup (formatter_utils.rs)
	•	✅ run_parallel() / run_sequential() refactor in main.rs
	•	✅ Input format sniffing (--format auto)
	•	✅ Summary tables (--summary)
	•	✅ Native track_*() functions (count, avg, unique, etc.)
	•	✅ Benchmarking with Criterion.rs
	•	✅ Fuzzing with cargo fuzz
	•	✅ Error strategy flag (--on-error) with clear defaults
	•	✅ Clean stream modes UX table (default, serial, realtime, unordered)

⸻

🛠️ In Development / For Immediate Focus

- Nom-based logfmt parser with robust edge case support
- Finalize field extraction and flattening strategy
- Define and inject standard kelora_std Rhai helpers
- Build standard tests for fan-out, coercion, emit logic
- Add --config / .kelorarc for persistent options

⸻

🧱 Distinctive Traits vs. Other Tools

Tool	        Kelora Is…
jq  	        More structured, stateful, supports multiline, real scripting
awk	            Safer, saner, and field-aware — built for logs, not CSVs
lnav	        Not interactive — scriptable, batch-oriented, composable in pipelines
angle-grinder	More flexible due to Rhai, chunking, tracked state, and event fan-out
Loki / Vector	Not a log shipper — Kelora is a processing tool, not a system

⸻

❌ Things You’ve Decided Not to Do

- No auto-coercion of types: Leads to silent failures; better to be explicit
- No implicit fan-out from arrays: Too magical and error-prone
- No implicit flattening: Controlled flattening via helper functions only
- No plugin system or extensions: Overkill; instead expose composable functions via Rhai

⸻

📍 Your Preferences as a Developer
	•	✅ Value clarity, minimalism, and control
	•	✅ Tolerate complexity internally to provide clean, predictable behavior externally
	•	✅ Prioritize CLI ergonomics and scriptable UX
	•	✅ Prefer building blocks over opinionated automation
	•	✅ Design for untrusted, inconsistent input (e.g. malformed fields, bad types)
	•	✅ Have learned from previous project (klp) and want to avoid its feature creep

⸻

✨ Summary Tagline

Kelora is a scriptable log processor for real-world logs.
Designed for pipelines, CI, and fast triage. One-liners in Rhai. Structured in, structured out. Nothing more — and nothing less.
