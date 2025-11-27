# Performance ideas (behaviour-neutral)

Collected spots where we can trim overhead without changing outputs or tests.

- Cache AST field lookups in filter/exec stages: `FilterStage::apply` and `ExecStage::apply` call `read_fields()` on every event and rebuild `BTreeSet`s for warnings. Cache the accessed field list when the stage is built; skip the warning path entirely when the cache is empty.
- Reuse line buffers in the file-aware reader thread (`parallel.rs`): we currently `trim_end().to_string()` each line, causing an allocation/copy per line. Trim in place and `mem::take` the buffer into the channel message so the same `String` is reused.
- Cut allocations in multiline timestamp detection (`pipeline/multiline.rs`): `timestamp_prefix_candidates` eagerly builds multiple `String`s and re-counts chars. Returning slices/ranges (e.g., via `SmallVec` of indices) and only materializing on parse attempts would avoid several allocations and UTF-8 scans per line.
- Reduce stdin copy churn (`readers.rs`): `ChannelStdinReader` copies each 8KB chunk into a fresh `Vec<u8>` before sending. Reusing/pooling buffers (send owned buffers back for refill or use `Box<[u8]>` with reclamation) removes one allocation+copy per chunk.
- Lower lock contention when merging worker state (`parallel.rs`): `GlobalTracker` uses `std::sync::Mutex` and locks multiple times per batch. Swap to `parking_lot::Mutex` and batch merges to a single lock per batch for cheaper sync under many workers.

Suggested flow: pick one hotspot, refactor with local benchmarks (`just bench-quick`), then proceed to the next.
