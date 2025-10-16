Kelora Span Spec (tumbling, sequential)

Intent

Provide non-overlapping aggregation over a stream with a minimal, deterministic UX. Spans are batches formed by count or event time. Per-event scripts still run; a hook runs once per span.

⸻

CLI

--span <N|DURATION>
--span-close '<Rhai>'

	•	--span <N> → count-based: close after every N events that pass --filter (if specified). The count applies to the filtered event stream.
	•	--span <DURATION> → time-based: close on fixed time boundaries derived from the events' timestamp field (see "Time source").
	•	--span-close → Rhai snippet executed once when a span closes. Span helpers are available only here.

Sequential only. If --parallel is supplied, Kelora prints a warning and runs sequentially.

⸻

Time source & alignment (time-based mode)
	•	Time is taken from the event's ts (Kelora's canonical timestamp field).
	•	First event with valid ts anchors the cadence to absolute boundaries:
	•	Compute the boundary period containing the first event, aligned to the duration.
	•	Example: first ts = 12:03:27 with --span 1m → first span is [12:03:00, 12:04:00), then [12:04:00, 12:05:00), etc.
	•	Events with missing/invalid ts do not affect anchor selection.
	•	Intervals are half-open: [start, end).
	•	Implementation note: Use integer milliseconds for timestamp comparisons to avoid floating-point precision issues.

Missing/invalid ts:
	•	Strict mode: event is an error.
	•	Resilient (default): event is processed normally (--filter and --exec run) but excluded from time-span aggregation. It is NOT added to any span buffer and does NOT appear in span_events() or contribute to span_size(). Metadata assigned: meta.span_late = true, meta.span_key = "unassigned", meta.span_start and meta.span_end are omitted or null.

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
	4.	Late events are emitted to output (unless suppressed by --filter or --exec).
	5.	Internal counter late_events increments (visible in --stats).

If an event belongs to the currently open span, it's included and meta.span_late = false.

Count-based mode has no late events (spans follow arrival order).

Note: For accurate time-based aggregation, users should pre-sort logs by timestamp (e.g., `sort -k <timestamp-field>`).

⸻

Span context (Rhai) — available only during --span-close

Note: Window helpers (window_events(), window_size(), etc.) are NOT available in --span-close context. Use span helpers exclusively.

Functions:
	•	span_start() → DateTime (start bound)
	•	span_end() → DateTime (end bound)
	•	span_key() → String
	•	Time: "{ISO_START}/{DURATION}" (e.g., 2025-10-15T12:03:00Z/1m)
	•	Count: "#<index>" (0-based)
	•	span_events() → Array of events in the span, in arrival order
	•	span_size() → Int (event count)

Metadata (injected into each buffered event immediately before --span-close runs):
	•	e.meta.span_key (String)
	•	e.meta.span_start (DateTime)
	•	e.meta.span_end (DateTime)
	•	e.meta.span_late (Bool; always false for in-span events)

Note: For events passing through without entering a span (late/unassigned), metadata is set during per-event processing phase.

⸻

Processor semantics
	1.	Per-event phase (always):
	•	Run --filter / --exec.
	•	Assign/update e.meta.* for spanning (see above).
	•	Events are emitted as they arrive (unless suppressed by filter/exec).
	2.	Boundary detection:
	•	Count: when N events that passed --filter (if specified) have accumulated → close.
	•	Time: when an event's ts (valid or invalid) falls in a newer interval than the current open one → close the current.
	•	Time mode: All events participate in boundary detection, but only events with valid ts and passing --filter are added to the span buffer.
	3.	Close phase:
	•	Inject span metadata into buffered events.
	•	Run --span-close once with span helpers. This is in addition to per-event output; --span-close can emit span-level summaries via emit_each().
	•	Reset span state and start the next span with the current event (time) or with an empty buffer (count).
	4.	End of input or interrupt signal (SIGINT/SIGTERM):
	•	If a span is open, close it gracefully and run --span-close before termination.
	•	If SIGINT/SIGTERM is received during --span-close execution, defer signal until script completes.

No synthetic empty spans are emitted (gaps with no events produce no span).

Example: With --span 1m and events at 12:00, 12:05, only the two spans containing events are emitted (not 12:01, 12:02, 12:03, 12:04).

⸻

Interactions
	•	--window N (sliding) is unchanged and orthogonal. If both are provided, --span still governs close hooks; window helpers reflect the sliding window within each per-event execution.
	•	--take limits overall output. If the take limit is reached mid-span, the current span is closed and --span-close runs before termination.
	•	--since/--until filter which events enter the stream; spans form over what remains.
	•	--stats shows:
	•	total_spans_closed (number of spans that closed)
	•	avg_events_per_span (mean span size)
	•	late_events (time-based only; count of events with meta.span_late = true)
	•	--metrics works as usual; use track_* in per-event or --span-close. Use pop_metric(key) or pop_metrics() to read-and-reset metrics between spans.

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
	•	If count N > 100,000: print a warning about potential memory usage. Consider breaking into smaller spans or using time-based mode.

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
    let sum = pop_metric("lat");
    emit_each([#{span: span_key(), n: n, avg_latency: if n>0 { sum / n } else { 0 }}]);
  '

Time spans, log late arrivals

kelora -j --span 1m \
  --exec '
    if e.meta.span_late { eprint("late -> " + e.meta.span_key); }
    track_count("hits");
  ' \
  --span-close '
    emit_each([#{start: span_start(), end: span_end(), hits: pop_metric("hits")}]);
  '

Time spans, emit histogram per minute

kelora -j --span 1m \
  --exec 'track_bucket("status", e.status.to_int())' \
  --span-close '
    let s = span_start();
    let e = span_end();
    let m = pop_metrics();   // whole metrics map
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

⸻

Implementation notes

Edge cases & clarifications:

1. Timestamp comparison precision
	•	Use integer milliseconds (or nanoseconds) for all timestamp comparisons internally.
	•	Avoid floating-point comparisons to prevent boundary detection bugs.
	•	Events with ts at exact boundary (e.g., 12:04:00.000 when span ends at 12:04:00) belong to the next span per half-open interval semantics.

2. Filter interaction with boundaries (time-based)
	•	All events (including those failing --filter) participate in span boundary detection.
	•	Filtered-out events advance the span clock but are not buffered.
	•	This ensures deterministic span boundaries regardless of filter logic.

3. First event anchoring (time-based)
	•	Only the first event with a valid ts anchors the time alignment.
	•	Events with missing/invalid ts are skipped during anchor selection.
	•	Once anchored, all subsequent boundary calculations are deterministic.

4. Large count spans
	•	Count mode buffers all N events in memory until close.
	•	For N > 100,000, warn about potential OOM risk.
	•	Recommendation: use time-based mode for high-volume streams.

5. Signal handling during close
	•	SIGINT/SIGTERM during --span-close execution: defer signal until script completes.
	•	This ensures span summaries are not corrupted by interruption.
	•	After --span-close completes, terminate gracefully.

6. Empty time spans
	•	Time gaps between events do NOT produce synthetic empty spans.
	•	Example: --span 1m with events at 12:00 and 12:05 → only 2 spans emitted.
	•	Users expecting regular time series should generate synthetic events upstream.

7. Metrics and pop_metric/pop_metrics
	•	track_* functions operate globally across all spans (not automatically scoped).
	•	Use pop_metric(key) or pop_metrics() in --span-close to read-and-reset per-span aggregations.
	•	Pattern: --exec increments, --span-close pops metrics and emits.

⸻

Testing recommendations

Critical edge cases to cover in integration tests:

1. End-of-stream handling
	•	Partial span at EOF is closed and --span-close runs.
	•	SIGINT/SIGTERM mid-stream: current span closes gracefully.

2. Events without timestamps (time-based)
	•	Missing ts: marked as unassigned, not buffered, but emitted.
	•	Invalid ts format: same handling as missing.
	•	First event has missing ts: next valid ts anchors alignment.

3. Boundary precision
	•	Events at exact boundary time (e.g., 12:04:00.000) belong to next span.
	•	Multiple events with identical timestamps stay in same span.
	•	Millisecond-level precision maintained throughout.

4. Filter interactions
	•	Count mode: --filter affects span size (only filtered events count toward N).
	•	Time mode: --filter doesn't affect boundary detection, but filtered events not buffered.
	•	Late events still pass through --filter.

5. Empty spans
	•	Time gap produces no synthetic spans (e.g., 12:00 → 12:05 with --span 1m = 2 spans, not 6).
	•	First span after anchor may be partial if first event is mid-interval.

6. Late events
	•	Event arrives after span closed: meta.span_late = true, not buffered.
	•	Late event still runs through --exec and --filter.
	•	late_events counter increments.

7. Large counts
	•	--span 100000 triggers warning but works.
	•	Memory usage scales linearly with N in count mode.

8. Interaction with other flags
	•	--take N: closes current span before stopping.
	•	--since/--until: spans form over filtered stream.
	•	--window N: orthogonal; window helpers work in per-event context.

9. Signal handling
	•	SIGINT during --span-close: script completes before exit.
	•	SIGINT during per-event --exec: current span closes gracefully.

10. Metrics pop operations
	•	pop_metric(key) reads and resets a single metric between spans.
	•	pop_metrics() reads and resets all metrics, returning a map.
	•	Multiple metrics tracked independently.

