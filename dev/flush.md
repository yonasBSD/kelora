# Kelora Time-Based Flush / Shutdown Spec (KISS Design)

## Purpose

Ensure Kelora flushes partial state (batches, multiline blocks, stats) when inputs are slow or silent, and terminates cleanly on Ctrl-C — without adding heartbeat threads, async runtimes, or overengineering.

Kelora remains a **streaming CLI tool**: stdin/files in, stdout out.

---

## Architecture

### Threads

* **Reader thread**: the *only* code that blocks on I/O. Reads stdin or files (with `--follow`, keeps fd open). Pushes lines/events into a bounded channel.
* **Main processing thread(s)**:

  * Sequential mode: single thread for chunking, parsing, filter/exec, formatting.
  * Parallel mode: 1 batcher + W worker threads \[+1 reorderer if ordered].
* **No heartbeat thread**: timeouts handled locally with `crossbeam::select! { after(dur) }`.

### Channels

* `line_tx/line_rx`: reader → chunker.
* `event_tx/event_rx`: chunker → batcher.
* `ctrl_tx/ctrl_rx`: signal handler → all components (for SIGINT/SIGTERM).

---

## Control Flow

### Reader

* Blocks on stdin or file read.
* Sends each line into `line_tx`.
* On EOF: closes channel.
* If `--follow`: loops, sleeping (200ms) and retrying at EOF. No rotation handling.

### Processing Loops (chunker, batcher)

Use one unified loop:

```rust
loop {
    let wait = next_deadline.saturating_duration_since(Instant::now());
    crossbeam::select! {
        recv(input_rx) -> msg => { /* handle new data; reset timer */ }
        recv(ctrl_rx) -> msg => { /* flush + graceful exit */ }
        after(wait) => { /* flush if idle too long */ }
    }
}
```

* **Batcher**: flushes if `buf.len() >= --batch-size` or idle ≥ `--batch-timeout` (default 200 ms).
* **Chunker**: flushes open block if idle ≥ `multiline_timeout`.
* **EOF**: flush final buffers then exit.
* **Ctrl-C**: flush if `immediate == false`; drop everything if second Ctrl-C (immediate).

---

## Shutdown Behavior

* On SIGINT: send `Ctrl::Shutdown { immediate: false }`. Components flush then exit. Exit code 130.
* On second SIGINT: `Ctrl::Shutdown { immediate: true }`, components drop and exit.

---

## Thread Counts

* **Sequential**: 2 threads total (reader + main).
* **Parallel ordered**: W + 3 (reader + batcher + reorderer + W workers).
* **Parallel unordered**: W + 2 (drop reorderer).

---

## Edge Cases

* **Trickle input**: still flushes via timeout.
* **Burst input**: size flush dominates, no lag.
* **Quiet stream**: partial state flushed periodically.
* **EOF**: guarantees last batch/block is emitted.
* **Pipes**: stdin blocking is isolated in reader.

---

## Defaults

* `--batch-size = 1000`
* `--batch-timeout = 200ms` (already documented)
* `multiline-timeout = 250–500ms` (TBD, not exposed yet)

---

## Why This Design

* **Zero extra threads** beyond reader + workers.
* **No spurious ticks** or tick channels.
* Each component owns its own timer, making it obvious what flushes when.
* Cleanly maps to Kelora’s existing CLI (`--batch-timeout`, `--parallel`, `--unordered`, `--multiline`).

---

## Testing Checklist

* Quiet stream → periodic flush fires.
* EOF → final flush once.
* Ctrl-C → flush then exit 130.
* Double Ctrl-C → immediate exit.
* Parallel ordered → flushes maintain output order.
* Parallel unordered → no reorder delay.


