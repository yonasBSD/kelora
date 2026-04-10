# Merge Sorted Files by Timestamp

Combine multiple log files into one chronological stream without loading
everything into memory first.

## When This Guide Helps

- You have one file per host, pod, service, or rotation window.
- Each file is already internally sorted by timestamp.
- You want to reconstruct a single incident timeline across those files.
- The files are large enough that pre-sorting them would be expensive or awkward.

## What `--merge-sorted` Actually Guarantees

`--merge-sorted` merges **already sorted** inputs. It does not perform a full
global sort over all events in all files.

Kelora keeps one pending event from each input file, emits the earliest
timestamp, then advances only that file. The result is efficient and
streaming-friendly:

- Memory stays bounded by the number of open input files, not total file size.
- Output becomes available immediately instead of after a full read/sort pass.
- Large archive sets remain practical to process in one command.

The tradeoff is straightforward: Kelora is strict about mergeability. If one
file is out of order, missing timestamps, or cannot be parsed well enough to
extract timestamps, the merge aborts instead of guessing.

## Practical Example

Imagine three collectors writing one JSONL file each:

- `api-a.jsonl`
- `api-b.jsonl`
- `worker.jsonl`

Each file is chronological on its own, but the timestamps overlap across files.
You want one merged view for an outage investigation.

```bash
kelora -j api-a.jsonl api-b.jsonl worker.jsonl \
  --merge-sorted \
  -k timestamp,service,level,message
```

This gives you a single interleaved timeline such as:

```text
2026-04-09T09:41:02Z api-a  WARN  upstream latency rising
2026-04-09T09:41:03Z worker INFO  retry queue depth=120
2026-04-09T09:41:04Z api-b  ERROR database timeout
2026-04-09T09:41:05Z api-a  ERROR request failed
```

That is often exactly what you need during incident work: one timeline across
multiple already-ordered sources, without paying for a heavyweight sort step.

## Why This Is Useful Despite the Sorting Requirement

In practice, many log sources are naturally append-only and already ordered:

- One file per service instance
- One shard per hour or day
- One collector output per host
- One rotated file per process

For those cases, `--merge-sorted` solves the real problem: **merge several
ordered streams into one ordered stream**.

A true global sort would require one of these heavier approaches:

- Buffer all events in memory
- Spill to disk and run an external sort
- Delay output behind a bounded reordering window

Those approaches make sense for a different class of feature. For normal
operations work, a streaming merge is usually the better tradeoff.

## Recommended Usage Pattern

Be explicit about the parser so Kelora can extract timestamps predictably.
Kelora auto-detects common timestamp field names such as `timestamp` and `ts`;
add `--ts-field <field>` only when your data uses a different key:

```bash
kelora -j shard-*.jsonl --merge-sorted -J
kelora -f logfmt app-*.log --merge-sorted --ts-field ts -F json
```

Good fits:

- JSON Lines with `timestamp` or `ts` fields
- Logfmt files with a stable timestamp key
- Rotated files where each file was written sequentially

Poor fits:

- Files known to contain clock rewrites or backfilled old events
- Inputs that mix unrelated formats line by line
- CSV/TSV workflows, which are not supported by `--merge-sorted` today

## Common Gotchas

- `--merge-sorted` is incompatible with `--parallel`
- Auto-detection must resolve to a concrete format before merging
- Before emitting the first event, Kelora must find one timestamped event in every input file
- If one file is internally out of order, `--merge-sorted` aborts at the first offending event
- Missing timestamps and merge-time parse failures are fatal in all modes; use `--ts-field <field>` when the timestamp is stored under a non-default key
- Output is still streamed. If a late merge error occurs after some events were emitted, that prefix remains valid; Kelora stops instead of trying to recover ordering

When you suspect disorder inside a file, inspect a sample first:

```bash
kelora -j shard-01.jsonl --take 20 -k timestamp,message
```

## Incident-Oriented Variant

Merge first, then apply your normal filters:

```bash
kelora -j api-*.jsonl worker-*.jsonl \
  --merge-sorted \
  --since '2026-04-09 09:40' \
  --until '2026-04-09 10:00' \
  -l error,warn \
  -k timestamp,service,request_id,message
```

This works well when an incident spans multiple components and each component
logged to its own ordered file.

## See Also

- [CLI Reference](../reference/cli-reference.md#merge-sorted) for the flag contract and constraints
- [Process Archives at Scale](batch-process-archives.md) for throughput and batch-processing tradeoffs
- [Integrate Kelora with External Tools](integrate-external-tools.md) when you need a heavier external sort or pre-filter step
