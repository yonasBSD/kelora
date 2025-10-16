Kelora Span Spec (tumbling, sequential)

Intent

Provide non-overlapping aggregation over a stream with a minimal, deterministic UX. Spans are batches formed by count or event time. Per-event scripts still run; a hook runs once per span.

⸻

CLI

--span <N|DURATION>
--span-close '<Rhai>'

	•	--span <N> → count-based: close after every N events.
	•	--span <DURATION> → time-based: close on fixed time boundaries derived from the events' timestamp field (see "Time source").
	•	--span-close → Rhai snippet executed once when a span closes. Span helpers are available only here.

Sequential only. If --parallel is supplied, Kelora prints a warning and runs sequentially.

⸻

Time source & alignment (time-based mode)
	•	Time is taken from the event's ts (Kelora's canonical timestamp field).
	•	First event anchors the cadence to absolute boundaries:
	•	Compute the boundary period containing the first event, aligned to the duration.
	•	Example: first ts = 12:03:27 with --span 1m → first span is [12:03:00, 12:04:00), then [12:04:00, 12:05:00), etc.
	•	Intervals are half-open: [start, end).

Missing/invalid ts:
	•	Strict mode: event is an error.
	•	Resilient (default): event is processed normally but excluded from time-span aggregation; it gets meta.span_late = true and meta.span_key = "unassigned".

⸻

Late events (time-based)

An event is late iff its ts falls into a span that has already been closed.

Policy (no flags):
	1.	We do not reopen or mutate closed spans.
	2.	Event is tagged:
	•	meta.span_late = true
	•	meta.span_key = "<start ISO8601>/<duration>" of the closed span it would've belonged to.
	•	meta.span_start, meta.span_end reflect that window's bounds.
	3.	Per-event scripts (--exec, --filter) still run.
	4.	Internal counter late_events increments (visible in --stats).

If an event belongs to the currently open span, it's included and meta.span_late = false.

Count-based mode has no late events (spans follow arrival order).

⸻

Span context (Rhai) — available only during --span-close

Functions:
	•	span_start() → DateTime (start bound)
	•	span_end() → DateTime (end bound)
	•	span_key() → String
	•	Time: "{ISO_START}/{DURATION}" (e.g., 2025-10-15T12:03:00Z/1m)
	•	Count: "#<index>" (0-based)
	•	span_events() → Array of events in the span
	•	span_size() → Int (event count)

Metadata (also injected into each event in the span before close):
	•	e.meta.span_key (String)
	•	e.meta.span_start (DateTime)
	•	e.meta.span_end (DateTime)
	•	e.meta.span_late (Bool; always false for in-span events)

⸻

Processor semantics
	1.	Per-event phase (always):
	•	Run --filter / --exec.
	•	Assign/update e.meta.* for spanning (see above).
	2.	Boundary detection:
	•	Count: when N events accumulated → close.
	•	Time: when an event's ts falls in a newer interval than the current open one → close the current.
	3.	Close phase:
	•	Run --span-close once with span helpers.
	•	Reset span state and start the next span with the current event (time) or with an empty buffer (count).
	4.	End of input: if a span is open, close it and run --span-close.

No synthetic empty spans are emitted (gaps with no events produce no span).

⸻

Interactions
	•	--window N (sliding) is unchanged and orthogonal. If both are provided, --span still governs close hooks; window helpers reflect the sliding window within each per-event execution.
	•	--take limits overall output; it does not truncate internal span formation.
	•	--since/--until filter which events enter the stream; spans form over what remains.
	•	--stats shows late_events (time-based only), span counts, and average span size.
	•	--metrics works as usual; use track_* in per-event or --span-close. A track_snapshot(key?) helper is recommended (existing pattern) to read-and-reset between spans.

⸻

Error handling
	•	Invalid --span argument → usage error (Exit 2).
	•	Accepted forms:
	•	Count: /^[1-9]\d*$/
	•	Duration: <int>(ms|s|m|h) (e.g., 500ms, 10s, 1m, 2h)
	•	Time-based with missing ts:
	•	Strict: error per event (Exit 1 if encountered).
	•	Resilient: mark late/unassigned as described; proceed.
	•	If --parallel is set: print a warning "--span forces sequential execution" and continue sequentially.

⸻

Performance & memory
	•	Bounded memory: only the current span is buffered (plus normal pipeline state).
	•	Time-based: a late event never reopens prior spans, so memory does not grow with disorder.
	•	Count-based: buffer size ≤ N events.

⸻

Examples

Count spans, rolling averages

kelora -j --span 500 \
  --exec 'track_sum("lat", e.latency.to_int())' \
  --span-close '
    let n = span_size();
    let sum = track_snapshot("lat");
    emit_each([#{span: span_key(), n: n, avg_latency: if n>0 { sum / n } else { 0 }}]);
  '

Time spans, log late arrivals

kelora -j --span 1m \
  --exec '
    if e.meta.span_late { eprint("late -> " + e.meta.span_key); }
    track_count("hits");
  ' \
  --span-close '
    emit_each([#{start: span_start(), end: span_end(), hits: track_snapshot("hits")}]);
  '

Time spans, emit histogram per minute

kelora -j --span 1m \
  --exec 'track_bucket("status", e.status.to_int())' \
  --span-close '
    let s = span_start();
    let e = span_end();
    let m = track_snapshot();   // whole metrics map
    emit_each([#{start: s, end: e, status_hist: m.status}]);
  '


⸻

Help text additions (concise)

--span <N|DUR>         Form non-overlapping spans by count (N) or time (DUR: 500ms, 2s, 1m, 1h).
                        Sequential only; forces sequential mode if --parallel is set.
                        Time mode uses event ts; spans are [start, end). Late events never mutate closed spans.

--span-close <RHAI>    Run once when a span closes. Available: span_start(), span_end(),
                        span_key(), span_events(), span_size().
                        Each event carries meta.span_key, meta.span_start, meta.span_end, meta.span_late.


⸻

Design notes (fit with Kelora ethos)
	•	Two knobs only keeps the surface area small and legible.
	•	Determinism over cleverness: no reopening, no watermark heuristics.
	•	meta.* ledger makes the processor's bookkeeping explicit and non-invasive.
	•	Sequential by design avoids "stream processor" creep while covering the common CLI/pipe cases.

If you want one optional hard edge later, a single --strict-span could treat any meta.span_late as a hard error—same model, stricter hygiene.

