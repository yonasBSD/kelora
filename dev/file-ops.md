# File Output Functions — Implementation Guide

## Purpose & Scope

Give Rhai scripts first-class, intentional file-system side effects so users can build reports, shard logs, or persist summaries while Kelora streams events. The scope is limited to three explicit primitives (`mkdir`, `truncate_file`, `append_file`) that cover directory preparation, zeroing outputs, and appending content. Anything beyond these (copying, deleting, random access) stays out of scope until we see real demand.

## Safety Gate: `--allow-fs-writes`

* Default: side-effect functions are inert. They return `false`, emit a one-time warning (`kelora: enable --allow-fs-writes to use mkdir/truncate_file/append_file`), and do nothing.
* `--allow-fs-writes` (no short flag) flips the switch. All three functions become available in every Rhai stage (`--begin`, `--filter`, `--exec`, `--end`, `--on-error`).
* This flag must also be recorded in the config struct we pass into the engine so workers can check it quickly without extra Arc lookups.
* Help text: “Allow Rhai scripts to create directories and write files on disk. Disabled by default for safety.” Update `help-screen.txt` when the flag lands.
* `-qqq` still suppresses the warning spam, even when the functions stay inert.

## Rhai Surface Area

Summary table:

Function | Parameters | Returns | Behavior (with flag)
---------|------------|---------|----------------------
`mkdir(path, create_parents = false)` | `path: string`, `create_parents?: bool` | `bool` | Creates directory. `true` = created; `false` = already there or failed.
`truncate_file(path)` | `path: string` | `bool` | Opens `path` with truncate/create semantics. `true` on success.
`append_file(path, content)` | `path: string`, `content: string or Array<string>` | `bool` | Appends newline-terminated text; `true` on success.

Return values are deliberately simple for scripting: `true` when the requested effect happened, `false` for “no-op” (already existed) or any failure. Errors can still bubble out in `--strict` mode (see Error Handling).

## Detailed Behavior

### `mkdir(path, create_parents = false)`

* Uses `std::fs::create_dir` when `create_parents == false`, `std::fs::create_dir_all` otherwise.
* Success cases: directory created → `true`; directory already exists → `false` (no error).
* Failure cases (permission, invalid path, file instead of dir) → log once per unique `(path, op)` combo when not in `--quiet`, return `false` in resilient mode, raise `EvalAltResult` in strict mode.
* Accepts absolute or relative paths (relative to Kelora’s working directory). No tilde expansion.

### `truncate_file(path)`

* Uses `OpenOptions::new().write(true).create(true).truncate(true)` to zero the file (or create a fresh empty file).
* Returns `true` on success, `false` on failure.
* Parallel-safe: the truncate call must run on the coordinator thread (see Ordering) so multiple workers do not race.

### `append_file(path, content)`

* Accepts either `ImmutableString` or `Array` of `ImmutableString`.
* Normalize to `Vec<Bytes>`:
  * String → append exactly once, ensure the data ends with a single `\n`. Do **not** double up if caller already supplied newline; run `ensure_newline()`.
  * Array → treat each element as a logical line, append every string with a trailing newline. Empty array → `true` and no IO.
* File access: `OpenOptions::new().create(true).append(true).open(path)` and `write_all` per chunk.
* Use buffered writing (small stack buffer) to avoid multiple syscalls per element.
* Guarantee appends are atomic at least per call: in unordered parallel mode we still rely on kernel append semantics, so we need a mutex keyed by `PathBuf` in the executor to serialize concurrent appends to the same file.

## Execution Stages & Hook Points

* Begin/End scripts run on the main thread: execute file ops immediately.
* Filter/Exec/On-error scripts run in the pipeline (potentially parallel). They must enqueue file operations instead of touching the FS directly, then rely on the executor to replay them in the desired order.
* The Rhai functions simply record an operation (`FileOp::MkDir`, `FileOp::Truncate`, `FileOp::Append { data }`) onto a thread-local buffer that is drained after the script stage finishes.
* Interact nicely with `emit_each`: emitted child events get their own buffers; when they surface in the main pipeline they can enqueue file ops too.

## Ordering & Concurrency Semantics

| Mode | Behavior |
|------|----------|
| Sequential | Apply operations immediately after script evaluation. No buffering beyond the thread-local staging vector. |
| Parallel Ordered | Each worker attaches file ops to the event payload. When the reorderer restores event order, it also replays the associated operations. This guarantees that append/truncate/mkdir happen in the same sequence as the logical event order. |
| Parallel Unordered | Workers hand their file ops to a central executor via a lock-free queue. The executor drains ops as soon as they arrive. Ordering between *different files* is undefined. Within a single file we maintain serialization via the per-path mutex, so lines from one event stay together. |

Implementation support pieces:

* `FileOp` enum stored in `crate::rhai_functions::file_ops` module.
* `FileOpRecorder` (thread local `RefCell<Vec<FileOp>>`) exposed via helper functions so each Rhai call just pushes onto it.
* `FileOpSink` trait with two implementations: `Immediate` (sequential) and `Buffered { reorder_key }` for pipeline modes.
* Engine integration: after each Rhai stage evaluation, call `file_ops::flush_pending(stage, sink)` to hand the recorded ops to the executor.
* The executor (`FileOpExecutor`) owns a `DashMap<PathBuf, Arc<Mutex<()>>>` to coordinate append serialization and runs in the main orchestrator thread.

## Error Handling Model

* Resilient (default): Rhai function returns `false`; executor logs a single warning per unique `(path, operation, errno)` while keeping the pipeline alive.
* Strict (`--strict`): the first failure bubbles as `anyhow::Error` from the executor, canceling the run. Include context (`truncate_file("reports/2025.csv") failed: Permission denied`).
* When `--allow-fs-writes` is *not* set, the functions:
  * Return `false` immediately.
  * Set a thread-local flag so we warn at most once per process.
  * Skip the executor entirely so no operations queue up.
* Respect `--quiet` levels: `-q` prints minimal warnings, `-qq` silent, `-qqq` completely silent.

## Implementation Steps

1. **CLI / Config**
   * Add `allow_fs_writes: bool` to the top-level settings struct (propagated into `EngineBuilder`).
   * Wire `--allow-fs-writes` through Clap and config file parsing.

2. **Rhai Module**
   * Create `src/rhai_functions/file_ops.rs` with `register_functions(engine: &mut Engine, cfg: FileOpConfig)`.
   * Maintain a `FileOpState` (flag + recorder accessors). Expose `with_recording(|recorder| ...)` helpers.

3. **Engine Plumbing**
   * Extend `rhai_functions::register_all_functions` to accept context (or add secondary function) so `file_ops::register_functions` sees the config.
   * After every stage (`run_begin`, `run_exec`, etc.) call `flush_file_ops(stage_name, sink)`.

4. **Executor**
   * Implement `FileOpExecutor::apply(FileOp, mode)` using synchronous std::fs APIs.
   * Provide `apply_in_sequence(Vec<FileOp>)` so ordered pipeline can pass batches.
   * Ensure truncates happen before append ops when both appear in the same batch (sort by explicit ordering inside the vector).

5. **Logging & Telemetry**
   * Count successes/failures for `--stats` surface.
   * Emit `kelora: appended 120 lines to reports/errors.log` at `-vv` for insight.

6. **Documentation**
   * Update `docs/` Rhai reference once implemented.
   * Refresh `help-screen.txt` when CLI flag is available.

## Testing Matrix

* Unit tests in `file_ops.rs` for each function: inert mode, success paths, error propagation, array handling, newline normalization.
* Executor tests verifying:
  * Append serializes per file across threads (use tempdir + threads).
  * Ordered vs unordered sequences yield expected file contents.
  * Truncate followed by append returns expected final payload.
* Integration tests under `tests/`:
  * `file_ops_inert_without_flag` — run `kelora` with scripts that call the functions; assert files untouched and warning produced.
  * `file_ops_basic_workflow` — end-to-end begin/exec/end using tempfile directory, verify output.
  * `file_ops_strict_mode_failure` — simulate permission error and assert process exits non-zero.
* Update golden docs (`help-screen.txt`) after CLI additions.

## Usage Recipes (For Future Docs)

* Warm start: `kelora --allow-fs-writes --begin 'truncate_file("report.txt"); append_file("report.txt", "HEADER")'` …
* Per-service shards: `mkdir("services/" + e.service, true); append_file("services/" + e.service + "/events.log", e.line)`.
* Error capture with tail newline normalization.

These recipes do not need full coverage tests but make great examples for the user docs once the feature lands.

***

This guide should give enough detail to implement the three file operations safely, predictably, and with full parity across sequential and parallel execution modes.
