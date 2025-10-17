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
	•	Strict mode (`--strict`): event is an error (fail-fast).
	•	Resilient (default): event is processed normally (--filter and --exec run) but excluded from time-span aggregation. It is NOT added to any span buffer and does NOT appear in span_events() or contribute to span_size(). Metadata assigned: meta.span_status = "unassigned", meta.span_id = null, meta.span_start and meta.span_end are null.

⸻

Late events (time-based)

Definitions assume duration_ms is the configured span length (milliseconds) and anchor_start_ms is the start of the first window (derived from the first valid timestamp).

An event is late iff its ts falls into a span that has already been closed, including windows that chronologically precede the anchor event.

Policy (no flags):
	1.	We do not reopen or mutate closed spans.
	2.	Event is tagged:
		•	meta.span_status = "late"
		•	meta.span_id = "<start ISO8601>/<duration>" of the closed span it would've belonged to.
		•	meta.span_start, meta.span_end reflect that window's bounds.
	•	Window bounds use integer math to avoid float precision bugs:
		•	delta_ms = event_ts_ms - anchor_start_ms
		•	k = delta_ms.div_euclid(duration_ms) // floor division handles negatives
		•	window_start_ms = anchor_start_ms + k * duration_ms
		•	window_end_ms = window_start_ms + duration_ms
	3.	Per-event scripts (--exec, --filter) still run.
	4.	Late events are emitted to output (unless suppressed by --filter or --exec).
	5.	Internal counter late_events increments (visible in --stats).

If an event belongs to the currently open span, it's included and meta.span_status = "included".

Count-based mode has no late events (spans follow arrival order).

Note: For accurate time-based aggregation, users should pre-sort logs by timestamp (e.g., `sort -k <timestamp-field>`).

⸻

Span context (Rhai) — available only during --span-close

Note: Window helpers (window_events(), window_size(), etc.) are NOT available in --span-close context. Use span helpers exclusively.
The hook fires after the event that closed the span finishes all per-event stages, so span_events() only contains events that survived the pipeline. The hook still runs even if span_size() == 0 (e.g., every event was filtered or marked unassigned); scripts decide whether to emit anything.

Functions:
	•	span_start() → DateTime (start bound)
	•	span_end() → DateTime (end bound)
	•	span_id() → String
	•	Time: "{ISO_START}/{DURATION}" (e.g., 2025-10-15T12:03:00Z/1m)
	•	Count: "#<index>" (0-based)
	•	span_events() → Array of events in the span, in arrival order
	•	span_size() → Int (event count)
	•	span_metrics → Map snapshot of metrics recorded during the span

Metadata (assigned during per-event processing, available in --exec, --filter, and --span-close):
	•	meta.span_status (String: "included" | "late" | "unassigned" | "filtered")
	•	meta.span_id (String or null)
	•	meta.span_start (DateTime or null)
	•	meta.span_end (DateTime or null)

Status values:
	•	"included" → Event is buffered in current span (normal case)
	•	"late" → Valid ts but arrived after its span closed (time mode only)
	•	"unassigned" → Missing/invalid ts (time mode only)
	•	"filtered" → Failed --filter, not buffered or emitted

⸻

Processor semantics
	1.	Per-event phase (always):
	•	Before the first user-provided stage runs, compute span alignment for the event and assign provisional metadata: meta.span_id/span_start/span_end plus meta.span_status = "included", "late", or "unassigned". This metadata is visible to every --exec/--filter stage.
	•	Run CLI-ordered --exec/--filter stages. Each stage can inspect meta.span_*.
	•	If a --filter returns false, immediately set meta.span_status = "filtered", skip any later stages, and do not emit or buffer the event.
	•	When the event exits the per-event pipeline without being filtered, emit it normally. If meta.span_status is still "included", append it to the current span buffer for span_events(); "late" and "unassigned" events continue to flow but never enter the buffer.
	2.	Boundary detection:
	•	Count: when N events with status "included" have accumulated → close.
	•	Time: when an event's ts crosses into a newer interval → close the current span.
	•	Filtered events: in count mode, don't count toward N; in time mode, advance boundaries but aren't buffered.
	3.	Close phase:
	•	After the event that triggered the boundary finishes the per-event pipeline, run --span-close once with span helpers and access to buffered events via span_events(). span_size() may be 0 if every event was filtered or marked unassigned.
	•	span_metrics snapshots tracked values for the span; the snapshot is cleared after --span-close finishes.
	•	--span-close can emit span-level summaries via emit_each().
	•	Reset span state and start the next span.
	4.	End of input or interrupt signal (SIGINT/SIGTERM):
	•	If a span is open with buffered events, close it and run --span-close before termination.
	•	First SIGINT/SIGTERM during --span-close: defer signal until script completes.
	•	Second SIGINT/SIGTERM within 2 seconds: force immediate exit (code 130/143).

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
	•	late_events (time-based only; count of events with meta.span_status = "late")
	•	unassigned_events (time-based only; count of events with meta.span_status = "unassigned")
	•	--metrics works as usual; use track_* in per-event or --span-close. See "Span metrics access" below for reading metrics.

⸻

Span metrics access

In --span-close context, two read-only maps are available:

	•	span_metrics → Metrics collected since the current span opened. The map is refreshed before --span-close runs and cleared afterward.
	•	metrics → Global metrics accumulated for the whole run (same as --end stage with --metrics enabled).

Patterns:
	•	Per-span aggregation: Use track_* in --exec to accumulate, then read values directly from span_metrics during --span-close (e.g., `let hits = if span_metrics.contains("hits") { span_metrics["hits"] } else { 0 };`). The map is empty if the span produced no tracked values.
	•	Cross-span state: Read metrics[...] for cumulative totals and continue updating them with track_* if you need rolling aggregates across spans.

Both maps are immutable from Rhai scripts; functions such as metrics.pop() are unavailable. Span consumption never mutates the global metrics map, so the final --metrics report remains intact.

⸻

Filter interaction decision table

Event disposition by mode and condition:

┌─────────────────┬──────────────┬────────────────┬──────────────────┬─────────────────┐
│ Condition       │ Counted for  │ Buffered for   │ Emitted to       │ span_status     │
│                 │ Boundary?    │ span_events()? │ Output?          │                 │
├─────────────────┼──────────────┼────────────────┼──────────────────┼─────────────────┤
│ COUNT MODE                                                                            │
├─────────────────┼──────────────┼────────────────┼──────────────────┼─────────────────┤
│ Passes --filter │ Yes (N++)    │ Yes            │ Yes              │ "included"      │
├─────────────────┼──────────────┼────────────────┼──────────────────┼─────────────────┤
│ Fails --filter  │ No           │ No             │ No               │ "filtered"      │
├─────────────────┼──────────────┼────────────────┼──────────────────┼─────────────────┤
│ TIME MODE                                                                             │
├─────────────────┼──────────────┼────────────────┼──────────────────┼─────────────────┤
│ Valid ts,       │ Yes (closes  │ Yes            │ Yes              │ "included"      │
│ passes filter   │ if new       │                │                  │                 │
│                 │ interval)    │                │                  │                 │
├─────────────────┼──────────────┼────────────────┼──────────────────┼─────────────────┤
│ Valid ts,       │ Yes (closes  │ No             │ No               │ "filtered"      │
│ fails filter    │ if new       │                │                  │                 │
│                 │ interval)    │                │                  │                 │
├─────────────────┼──────────────┼────────────────┼──────────────────┼─────────────────┤
│ Missing/invalid │ No           │ No             │ Yes              │ "unassigned"    │
│ ts, passes      │              │                │                  │                 │
│ filter          │              │                │                  │                 │
├─────────────────┼──────────────┼────────────────┼──────────────────┼─────────────────┤
│ Late ts (after  │ No           │ No             │ Yes              │ "late"          │
│ span closed),   │              │                │                  │                 │
│ passes filter   │              │                │                  │                 │
└─────────────────┴──────────────┴────────────────┴──────────────────┴─────────────────┘

Key principle: In time mode, all events with valid ts participate in boundary detection (deterministic span boundaries regardless of filter logic). Only "included" events are buffered.

⸻

Error handling
	•	Invalid --span argument → usage error (Exit 2).
	•	Accepted forms:
	•	Count: /^[1-9]\d*$/
	•	Duration: <int>(ms|s|m|h) (e.g., 500ms, 10s, 1m, 2h)
	•	Time-based with missing ts:
	•	Strict: error per event (Exit 1 if encountered).
	•	Resilient: mark as "unassigned" status; proceed.
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
    let sum = if span_metrics.contains("lat") { span_metrics["lat"] } else { 0 };
    emit_each([#{span: span_id(), n: n, avg_latency: if n > 0 { sum / n } else { 0 }}]);
  '

Time spans, log late arrivals

kelora -j --span 1m \
  --exec '
    if meta.span_status == "late" {
      eprint("⚠️  Late: " + e.ts + " → " + meta.span_id);
    }
    if meta.span_status == "included" {
      track_count("hits");
    }
  ' \
  --span-close '
    let hits = if span_metrics.contains("hits") { span_metrics["hits"] } else { 0 };
    emit_each([#{start: span_start(), end: span_end(), hits: hits}]);
  '

Time spans, emit histogram per minute

kelora -j --span 1m \
  --exec 'track_bucket("status", e.status.to_int())' \
  --span-close '
    let hist = if span_metrics.contains("status") { span_metrics["status"] } else { [] };
    emit_each([#{start: span_start(), end: span_end(), status_hist: hist}]);
  '

Running total across spans

kelora -j --span 500 \
  --exec 'track_count("hits")' \
  --span-close '
    let span_hits = if span_metrics.contains("hits") { span_metrics["hits"] } else { 0 };
    track_sum("total", span_hits); // add just this span's delta
    let cumulative = if metrics.contains("total") { metrics["total"] } else { 0 };
    emit_each([#{
      span: span_id(),
      span_hits: span_hits,
      total: cumulative
    }]);
  '


⸻

Help text additions (concise)

--span <N|DUR>         Form non-overlapping spans by count (N) or time (DUR: 500ms, 2s, 1m, 1h).
                        Sequential only; forces sequential mode if --parallel is set.
                        Time mode uses event ts; spans are [start, end). Late events never mutate closed spans.

--span-close <RHAI>    Run once whenever a span closes.
                        Available: span_start(), span_end(), span_id(), span_events(), span_size(), span_metrics.
                        Metrics: span_metrics (per span, read-only), metrics dict (global, read-only).
                        Each event carries meta.span_status, meta.span_id, meta.span_start, meta.span_end.


⸻

Design philosophy

	1.	Arrival order is truth: Spans form based on when events arrive, not when they claim to have occurred. Late events never mutate history.

	2.	Streaming by default: Events flow through immediately with metadata assigned. Buffering is internal and minimal (current span only).

	3.	Explicit over implicit: Event disposition is visible in meta.span_status. No silent drops or state mutations.

	4.	Fail-safe defaults: Missing timestamps mark events as unassigned but don't halt processing. Run with --strict if you prefer fail-fast timestamp handling.

	5.	Single responsibility: Spans aggregate; filters filter. A filtered event in time mode still advances the clock (deterministic boundaries).

	6.	Two knobs only: Minimal surface area keeps the feature legible and composable.

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
	•	First SIGINT/SIGTERM during --span-close: defer signal until script completes.
	•	This ensures span summaries are not corrupted by interruption.
	•	Second SIGINT/SIGTERM within 2 seconds: force immediate exit (code 130/143).
	•	Recommendation: Print "Received signal, waiting for span close... (Ctrl+C again to force quit)" on first signal.

6. Empty time spans
	•	Time gaps between events do NOT produce synthetic empty spans.
	•	Example: --span 1m with events at 12:00 and 12:05 → only 2 spans emitted.
	•	Spans can still close with span_size() == 0 if every event in the interval was filtered or marked unassigned; --span-close still runs so scripts can emit zeros or diagnostics.
	•	Users expecting regular time series should generate synthetic events upstream.

7. Metrics access patterns
	•	track_* functions operate globally; span-level diffs are derived automatically.
	•	Read span_metrics during --span-close for per-span summaries (e.g., `let hits = span_metrics["hits"];`). The map empties after each close.
	•	Global metrics remain available via metrics[...] and feed the final --metrics report.
	•	No manual reset step is required; span metrics never accumulate across spans.

⸻

Troubleshooting

Common issues and solutions:

Q: Why is span_size() always 0?
A: Check if events are failing --filter. Filtered events (span_status="filtered") don't enter the buffer. Only "included" events count.

Q: Late events not appearing in output?
A: They are emitted (check span_status="late" in output) but not in span_events(). Late events never enter closed spans.

Q: Metrics growing unexpectedly across spans?
A: track_* updates the global metrics map. Use span_metrics to read the per-span delta; it clears automatically after each close. Global metrics continue to grow until you intentionally track them to a new value.

Q: First span seems wrong or partial?
A: First event with valid ts anchors time alignment. If first event is mid-interval (e.g., 12:03:27 with --span 1m), first span is [12:03:00, 12:04:00). Pre-sort logs by timestamp for accuracy.

Q: Getting "unassigned" events in time mode?
A: Events have missing or invalid ts field. Check timestamp parsing. Run with --strict to turn them into hard errors, or fix upstream data.

Q: Span boundaries seem inconsistent?
A: In time mode, all events with valid ts advance the clock, even filtered ones. This ensures deterministic boundaries regardless of filter logic.

⸻

Testing recommendations

Critical edge cases to cover in integration tests:

1. End-of-stream handling
	•	Partial span at EOF is closed and --span-close runs.
	•	SIGINT/SIGTERM mid-stream: current span closes gracefully.

2. Events without timestamps (time-based)
	•	Missing ts: span_status="unassigned", not buffered, but emitted.
	•	Invalid ts format: same handling as missing.
	•	First event has missing ts: next valid ts anchors alignment.

3. Boundary precision
	•	Events at exact boundary time (e.g., 12:04:00.000) belong to next span.
	•	Multiple events with identical timestamps stay in same span.
	•	Millisecond-level precision maintained throughout.

4. Filter interactions
	•	Count mode: --filter affects span size (only events that pass every filter count toward N).
	•	Time mode: --filter doesn't affect boundary detection, but filtered events not buffered.
	•	Late events still pass through --filter.

5. Empty spans
	•	Time gap produces no synthetic spans (e.g., 12:00 → 12:05 with --span 1m = 2 spans, not 6).
	•	First span after anchor may be partial if first event is mid-interval.

6. Late events
	•	Event arrives after span closed: span_status="late", not buffered.
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

10. Metrics access operations
	•	span_metrics: contains only the metrics recorded during the current span; cleared after use.
	•	metrics dict or metrics[key]: read cumulative values without side effects.
	•	Span metrics do not require manual reset and never pollute later spans.
