#![allow(dead_code)]
use anyhow::Result;
use chrono::{DateTime, Utc};
use crossbeam_channel::{bounded, select, unbounded, Receiver, Sender};
use rhai::Dynamic;
use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::event::Event;
use crate::formatters::GapTracker;
use crate::pipeline::{self, PipelineBuilder, DEFAULT_MULTILINE_FLUSH_TIMEOUT_MS};
use crate::platform::{Ctrl, SHOULD_TERMINATE};
use crate::stats::{get_thread_stats, stats_finish_processing, stats_start_timer, ProcessingStats};

/// Configuration for parallel processing
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    pub num_workers: usize,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub preserve_order: bool,
    pub buffer_size: Option<usize>,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            num_workers: num_cpus::get(),
            batch_size: 1000,
            batch_timeout_ms: 200,
            preserve_order: true,
            buffer_size: Some(10000),
        }
    }
}

/// A batch of lines to be processed together
#[derive(Debug, Clone)]
pub struct Batch {
    pub id: u64,
    pub lines: Vec<String>,
    pub start_line_num: usize,
    pub filenames: Vec<Option<String>>,   // Filename for each line
    pub csv_headers: Option<Vec<String>>, // CSV headers for this batch (if applicable)
}

#[derive(Debug)]
enum LineMessage {
    Line {
        line: String,
        filename: Option<String>,
    },
    Error {
        error: io::Error,
        filename: Option<String>,
    },
    Eof,
}

/// Result of processing a batch
#[derive(Debug)]
pub struct BatchResult {
    pub batch_id: u64,
    pub results: Vec<ProcessedEvent>,
    pub internal_tracked_updates: HashMap<String, Dynamic>,
    pub worker_stats: ProcessingStats,
}

/// An event that has been processed and is ready for output
#[derive(Debug)]
pub struct ProcessedEvent {
    pub event: Event,
    pub captured_prints: Vec<String>,
    pub captured_eprints: Vec<String>,
    pub captured_messages: Vec<crate::rhai_functions::strings::CapturedMessage>,
    pub timestamp: Option<DateTime<Utc>>,
}

/// Thread-safe statistics tracker for merging worker states
#[derive(Debug, Default, Clone)]
pub struct GlobalTracker {
    internal_tracked: Arc<Mutex<HashMap<String, Dynamic>>>,
    processing_stats: Arc<Mutex<ProcessingStats>>,
    start_time: Option<Instant>,
}

impl GlobalTracker {
    pub fn new() -> Self {
        Self {
            internal_tracked: Arc::new(Mutex::new(HashMap::new())),
            processing_stats: Arc::new(Mutex::new(ProcessingStats::new())),
            start_time: Some(Instant::now()),
        }
    }

    pub fn merge_worker_stats(&self, worker_stats: &ProcessingStats) -> Result<()> {
        let mut global_stats = self.processing_stats.lock().unwrap();
        // Don't merge lines_read - that's handled by reader thread
        // Merge error counts (needed for --stats display and termination case)
        global_stats.lines_errors += worker_stats.lines_errors;
        global_stats.errors += worker_stats.errors;
        // Merge other worker stats
        global_stats.files_processed += worker_stats.files_processed;
        global_stats.script_executions += worker_stats.script_executions;
        // Calculate total processing time from global start time
        if let Some(start_time) = self.start_time {
            global_stats.processing_time = start_time.elapsed();
        }
        Ok(())
    }

    pub fn extract_final_stats_from_tracking(
        &self,
        metrics: &HashMap<String, Dynamic>,
    ) -> Result<()> {
        let mut stats = self.processing_stats.lock().unwrap();

        let output = metrics
            .get("__kelora_stats_output")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;
        // Note: Line-level filtering is not used - all filtering is done at event level
        let lines_errors = metrics
            .get("__kelora_stats_lines_errors")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;
        let events_created = metrics
            .get("__kelora_stats_events_created")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;
        let events_output = metrics
            .get("__kelora_stats_events_output")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;
        let events_filtered = metrics
            .get("__kelora_stats_events_filtered")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;

        stats.lines_output = output;
        stats.lines_errors = lines_errors;
        stats.errors = lines_errors; // Keep errors field for backward compatibility
        stats.events_created = events_created;
        stats.events_output = events_output;
        stats.events_filtered = events_filtered;

        // Extract discovered levels from tracking data
        if let Some(levels_dynamic) = metrics.get("__kelora_stats_discovered_levels") {
            if let Ok(levels_array) = levels_dynamic.clone().into_array() {
                for level in levels_array {
                    if let Ok(level_str) = level.into_string() {
                        stats.discovered_levels.insert(level_str);
                    }
                }
            }
        }

        // Extract discovered keys from tracking data
        if let Some(keys_dynamic) = metrics.get("__kelora_stats_discovered_keys") {
            if let Ok(keys_array) = keys_dynamic.clone().into_array() {
                for key in keys_array {
                    if let Ok(key_str) = key.into_string() {
                        stats.discovered_keys.insert(key_str);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_final_stats(&self) -> ProcessingStats {
        let mut stats = self.processing_stats.lock().unwrap().clone();
        // Ensure we have the latest processing time
        if let Some(start_time) = self.start_time {
            stats.processing_time = start_time.elapsed();
        }
        stats
    }

    pub fn set_total_lines_read(&self, total_lines: usize) -> Result<()> {
        let mut global_stats = self.processing_stats.lock().unwrap();
        global_stats.lines_read = total_lines;
        Ok(())
    }

    pub fn add_lines_filtered(&self, count: usize) -> Result<()> {
        let mut global_stats = self.processing_stats.lock().unwrap();
        global_stats.lines_filtered += count;
        Ok(())
    }

    pub fn merge_worker_state(&self, worker_state: HashMap<String, Dynamic>) -> Result<()> {
        let mut global = self.internal_tracked.lock().unwrap();

        for (key, value) in &worker_state {
            if key.starts_with("__op_") {
                global.insert(key.clone(), value.clone());
                continue;
            }

            if let Some(existing) = global.get(key) {
                let op_key = format!("__op_{}", key);
                let operation = worker_state
                    .get(&op_key)
                    .and_then(|v| v.clone().into_string().ok())
                    .unwrap_or_else(|| "replace".to_string());

                match operation.as_str() {
                    "count" => {
                        if let (Ok(a), Ok(b)) = (existing.as_int(), value.as_int()) {
                            global.insert(key.clone(), Dynamic::from(a + b));
                            continue;
                        }
                    }
                    "min" => {
                        // Take minimum
                        if let (Ok(a), Ok(b)) = (existing.as_int(), value.as_int()) {
                            global.insert(key.clone(), Dynamic::from(a.min(b)));
                            continue;
                        }
                    }
                    "max" => {
                        // Take maximum
                        if let (Ok(a), Ok(b)) = (existing.as_int(), value.as_int()) {
                            global.insert(key.clone(), Dynamic::from(a.max(b)));
                            continue;
                        }
                    }
                    "unique" => {
                        // Merge unique arrays
                        if let (Ok(existing_arr), Ok(new_arr)) =
                            (existing.clone().into_array(), value.clone().into_array())
                        {
                            let mut merged = existing_arr;
                            for item in new_arr {
                                if !merged.iter().any(|v| {
                                    // Compare string representations for simplicity
                                    v.to_string() == item.to_string()
                                }) {
                                    merged.push(item);
                                }
                            }
                            global.insert(key.clone(), Dynamic::from(merged));
                            continue;
                        }
                    }
                    "bucket" => {
                        // Merge bucket maps by summing counts
                        if let (Some(existing_map), Some(new_map)) = (
                            existing.clone().try_cast::<rhai::Map>(),
                            value.clone().try_cast::<rhai::Map>(),
                        ) {
                            let mut merged = existing_map;
                            for (bucket_key, bucket_value) in new_map {
                                if let Ok(bucket_count) = bucket_value.as_int() {
                                    let existing_count = merged
                                        .get(&bucket_key)
                                        .and_then(|v| v.as_int().ok())
                                        .unwrap_or(0);
                                    merged.insert(
                                        bucket_key,
                                        Dynamic::from(existing_count + bucket_count),
                                    );
                                }
                            }
                            global.insert(key.clone(), Dynamic::from(merged));
                            continue;
                        }
                    }
                    "error_examples" => {
                        // Merge error examples arrays (max 3 per type)
                        if let (Ok(existing_arr), Ok(new_arr)) =
                            (existing.clone().into_array(), value.clone().into_array())
                        {
                            let mut merged = existing_arr;
                            for item in new_arr {
                                if merged.len() < 3
                                    && !merged.iter().any(|v| v.to_string() == item.to_string())
                                {
                                    merged.push(item);
                                }
                            }
                            global.insert(key.clone(), Dynamic::from(merged));
                            continue;
                        }
                    }
                    _ => {
                        // Default: replace with newer value
                    }
                }
                global.insert(key.clone(), value.clone());
            } else {
                global.insert(key.clone(), value.clone());
            }
        }

        Ok(())
    }

    pub fn get_final_state(&self) -> HashMap<String, Dynamic> {
        self.internal_tracked.lock().unwrap().clone()
    }
}

/// Main parallel processor
pub struct ParallelProcessor {
    config: ParallelConfig,
    global_tracker: GlobalTracker,
    take_limit: Option<usize>,
}

impl ParallelProcessor {
    pub fn new(config: ParallelConfig) -> Self {
        Self {
            config,
            global_tracker: GlobalTracker::new(),
            take_limit: None,
        }
    }

    pub fn with_take_limit(mut self, take_limit: Option<usize>) -> Self {
        self.take_limit = take_limit;
        self
    }

    /// Process input using the parallel pipeline
    pub fn process_with_pipeline<
        R: std::io::BufRead + Send + 'static,
        W: std::io::Write + Send + 'static,
    >(
        &self,
        reader: R,
        pipeline_builder: PipelineBuilder,
        stages: Vec<crate::config::ScriptStageType>,
        config: &crate::config::KeloraConfig,
        output: W,
        ctrl_rx: crossbeam_channel::Receiver<Ctrl>,
    ) -> Result<()> {
        // For file processing, try to use file-aware reader if available
        if !config.input.files.is_empty() {
            return self.process_with_file_aware_pipeline(
                pipeline_builder,
                stages,
                config,
                output,
                ctrl_rx,
            );
        }

        // Fallback to original implementation for stdin
        self.process_with_generic_pipeline(
            reader,
            pipeline_builder,
            stages,
            config,
            output,
            ctrl_rx,
        )
    }

    fn process_with_generic_pipeline<
        R: std::io::BufRead + Send + 'static,
        W: std::io::Write + Send + 'static,
    >(
        &self,
        reader: R,
        pipeline_builder: PipelineBuilder,
        stages: Vec<crate::config::ScriptStageType>,
        config: &crate::config::KeloraConfig,
        output: W,
        ctrl_rx: crossbeam_channel::Receiver<Ctrl>,
    ) -> Result<()> {
        // Create channels
        let (batch_sender, batch_receiver) = if let Some(size) = self.config.buffer_size {
            bounded(size)
        } else {
            unbounded()
        };

        let (result_sender, result_receiver) = if self.config.preserve_order {
            bounded(self.config.num_workers * 4) // Increased from 2x to 4x workers
        } else {
            unbounded()
        };

        // For CSV formats, we need to peek at the first line to initialize headers
        // We'll wrap the reader to handle this preprocessing
        let (reader, pipeline_builder, preprocessing_line_count) = if matches!(
            config.input.format,
            crate::config::InputFormat::Csv
                | crate::config::InputFormat::Tsv
                | crate::config::InputFormat::Csvnh
                | crate::config::InputFormat::Tsvnh
        ) {
            Self::preprocess_csv_with_reader(reader, pipeline_builder, config)?
        } else {
            (
                Box::new(reader) as Box<dyn std::io::BufRead + Send>,
                pipeline_builder,
                0,
            )
        };

        let batch_timeout = Duration::from_millis(self.config.batch_timeout_ms);
        let line_buffer_bound = self.config.buffer_size.unwrap_or(10000);
        let (line_sender, line_receiver) = bounded(line_buffer_bound);

        let io_handle = {
            let ctrl_for_io = ctrl_rx.clone();
            thread::spawn(move || Self::plain_io_reader_thread(reader, line_sender, ctrl_for_io))
        };

        let batch_handle = {
            let batch_sender = batch_sender.clone();
            let batch_size = self.config.batch_size;
            let ignore_lines = config.input.ignore_lines.clone();
            let skip_lines = config.input.skip_lines;
            let global_tracker_clone = self.global_tracker.clone();
            let input_format = config.input.format.clone();
            let ctrl_for_batcher = ctrl_rx.clone();

            thread::spawn(move || {
                Self::batcher_thread(
                    line_receiver,
                    batch_sender,
                    batch_size,
                    batch_timeout,
                    global_tracker_clone,
                    ignore_lines,
                    skip_lines,
                    input_format,
                    preprocessing_line_count,
                    ctrl_for_batcher,
                )
            })
        };

        // Start worker threads
        let mut worker_handles = Vec::with_capacity(self.config.num_workers);
        let worker_multiline_timeout = if config.input.multiline.is_some() {
            Some(Duration::from_millis(DEFAULT_MULTILINE_FLUSH_TIMEOUT_MS))
        } else {
            None
        };

        for worker_id in 0..self.config.num_workers {
            let batch_receiver = batch_receiver.clone();
            let result_sender = result_sender.clone();
            let worker_pipeline_builder = pipeline_builder.clone();
            let worker_stages = stages.clone();
            let worker_ctrl = ctrl_rx.clone();
            let worker_timeout = worker_multiline_timeout;

            let handle = thread::spawn(move || {
                Self::worker_thread(
                    worker_id,
                    batch_receiver,
                    result_sender,
                    worker_pipeline_builder,
                    worker_stages,
                    worker_timeout,
                    worker_ctrl,
                )
            });
            worker_handles.push(handle);
        }

        // Drop senders to signal completion
        drop(batch_sender);
        drop(result_sender);

        // Start result sink thread
        let sink_handle = {
            let result_receiver = result_receiver;
            let preserve_order = self.config.preserve_order;
            let global_tracker = self.global_tracker.clone();
            let mut output = output;
            let config_clone = config.clone();
            let take_limit = self.take_limit;

            thread::spawn(move || {
                Self::pipeline_result_sink_thread(
                    result_receiver,
                    preserve_order,
                    global_tracker,
                    &mut output,
                    &config_clone,
                    take_limit,
                )
            })
        };

        // Wait for all threads to complete
        io_handle.join().unwrap()?;
        batch_handle.join().unwrap()?;

        for handle in worker_handles {
            handle.join().unwrap()?;
        }

        sink_handle.join().unwrap()?;

        Ok(())
    }

    fn process_with_file_aware_pipeline<W: std::io::Write + Send + 'static>(
        &self,
        pipeline_builder: PipelineBuilder,
        stages: Vec<crate::config::ScriptStageType>,
        config: &crate::config::KeloraConfig,
        output: W,
        ctrl_rx: crossbeam_channel::Receiver<Ctrl>,
    ) -> Result<()> {
        // Create file-aware reader
        let file_aware_reader = crate::pipeline::builders::create_file_aware_input_reader(config)?;

        // Create channels
        let (batch_sender, batch_receiver) = if let Some(size) = self.config.buffer_size {
            bounded(size)
        } else {
            unbounded()
        };

        let (result_sender, result_receiver) = if self.config.preserve_order {
            bounded(self.config.num_workers * 4)
        } else {
            unbounded()
        };

        // For CSV formats, we need to handle per-file preprocessing
        let file_aware_pipeline_builder = if matches!(
            config.input.format,
            crate::config::InputFormat::Csv
                | crate::config::InputFormat::Tsv
                | crate::config::InputFormat::Csvnh
                | crate::config::InputFormat::Tsvnh
        ) {
            // For now, we'll let the file-aware reader handle CSV initialization
            // This will be improved when we implement proper per-file schema detection
            pipeline_builder
        } else {
            pipeline_builder
        };

        let batch_timeout = Duration::from_millis(self.config.batch_timeout_ms);
        let line_buffer_bound = self.config.buffer_size.unwrap_or(10000);
        let (line_sender, line_receiver) = bounded(line_buffer_bound);

        let io_handle = {
            let ctrl_for_io = ctrl_rx.clone();
            thread::spawn(move || {
                Self::file_aware_io_reader_thread(file_aware_reader, line_sender, ctrl_for_io)
            })
        };

        let batch_handle = {
            let batch_sender = batch_sender.clone();
            let batch_size = self.config.batch_size;
            let ignore_lines = config.input.ignore_lines.clone();
            let skip_lines = config.input.skip_lines;
            let global_tracker_clone = self.global_tracker.clone();
            let input_format = config.input.format.clone();
            let ctrl_for_batcher = ctrl_rx.clone();

            thread::spawn(move || {
                Self::file_aware_batcher_thread(
                    line_receiver,
                    batch_sender,
                    batch_size,
                    batch_timeout,
                    global_tracker_clone,
                    ignore_lines,
                    skip_lines,
                    input_format,
                    ctrl_for_batcher,
                )
            })
        };

        // Start worker threads
        let mut worker_handles = Vec::with_capacity(self.config.num_workers);
        let worker_multiline_timeout = if config.input.multiline.is_some() {
            Some(Duration::from_millis(DEFAULT_MULTILINE_FLUSH_TIMEOUT_MS))
        } else {
            None
        };

        for worker_id in 0..self.config.num_workers {
            let batch_receiver = batch_receiver.clone();
            let result_sender = result_sender.clone();
            let worker_pipeline_builder = file_aware_pipeline_builder.clone();
            let worker_stages = stages.clone();
            let worker_ctrl = ctrl_rx.clone();
            let worker_timeout = worker_multiline_timeout;

            let handle = thread::spawn(move || {
                Self::worker_thread(
                    worker_id,
                    batch_receiver,
                    result_sender,
                    worker_pipeline_builder,
                    worker_stages,
                    worker_timeout,
                    worker_ctrl,
                )
            });
            worker_handles.push(handle);
        }

        // Drop senders to signal completion
        drop(batch_sender);
        drop(result_sender);

        // Start result sink thread
        let sink_handle = {
            let result_receiver = result_receiver;
            let preserve_order = self.config.preserve_order;
            let global_tracker = self.global_tracker.clone();
            let mut output = output;
            let config_clone = config.clone();
            let take_limit = self.take_limit;

            thread::spawn(move || {
                Self::pipeline_result_sink_thread(
                    result_receiver,
                    preserve_order,
                    global_tracker,
                    &mut output,
                    &config_clone,
                    take_limit,
                )
            })
        };

        // Wait for all threads to complete
        io_handle.join().unwrap()?;
        batch_handle.join().unwrap()?;

        for handle in worker_handles {
            handle.join().unwrap()?;
        }

        sink_handle.join().unwrap()?;

        Ok(())
    }

    /// Get the final merged global state for use in --end stage
    /// This converts __internal_tracked to the user-visible 'tracked' variable
    pub fn get_final_tracked_state(&self) -> HashMap<String, Dynamic> {
        self.global_tracker.get_final_state()
    }

    /// Get the final merged statistics from all workers
    pub fn get_final_stats(&self) -> ProcessingStats {
        self.global_tracker.get_final_stats()
    }

    /// Extract stats from tracking system into global stats
    pub fn extract_final_stats_from_tracking(
        &self,
        final_tracked: &HashMap<String, Dynamic>,
    ) -> Result<()> {
        self.global_tracker
            .extract_final_stats_from_tracking(final_tracked)
    }

    fn plain_io_reader_thread<R: std::io::BufRead>(
        mut reader: R,
        line_sender: Sender<LineMessage>,
        ctrl_rx: Receiver<Ctrl>,
    ) -> Result<()> {
        let mut buffer = String::new();
        loop {
            if let Ok(Ctrl::Shutdown { immediate }) = ctrl_rx.try_recv() {
                let _ = line_sender.send(LineMessage::Eof);
                if immediate {
                    return Ok(());
                }
                break;
            }

            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => {
                    let _ = line_sender.send(LineMessage::Eof);
                    break;
                }
                Ok(_) => {
                    let line = buffer.trim_end().to_string();
                    if line_sender
                        .send(LineMessage::Line {
                            line,
                            filename: None,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    let _ = line_sender.send(LineMessage::Error {
                        error: e,
                        filename: None,
                    });
                    break;
                }
            }
        }
        Ok(())
    }

    fn file_aware_io_reader_thread(
        mut reader: Box<dyn crate::readers::FileAwareRead>,
        line_sender: Sender<LineMessage>,
        ctrl_rx: Receiver<Ctrl>,
    ) -> Result<()> {
        let mut buffer = String::new();
        loop {
            if let Ok(Ctrl::Shutdown { immediate }) = ctrl_rx.try_recv() {
                let _ = line_sender.send(LineMessage::Eof);
                if immediate {
                    return Ok(());
                }
                break;
            }

            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => {
                    let _ = line_sender.send(LineMessage::Eof);
                    break;
                }
                Ok(_) => {
                    let line = buffer.trim_end().to_string();
                    let filename = reader.current_filename().map(|s| s.to_string());
                    if line_sender
                        .send(LineMessage::Line { line, filename })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    let filename = reader.current_filename().map(|s| s.to_string());
                    let _ = line_sender.send(LineMessage::Error { error: e, filename });
                    break;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn batcher_thread(
        line_receiver: Receiver<LineMessage>,
        batch_sender: Sender<Batch>,
        batch_size: usize,
        batch_timeout: Duration,
        global_tracker: GlobalTracker,
        ignore_lines: Option<regex::Regex>,
        skip_lines: usize,
        input_format: crate::config::InputFormat,
        preprocessing_line_count: usize,
        ctrl_rx: Receiver<Ctrl>,
    ) -> Result<()> {
        let mut batch_id = 0u64;
        let mut current_batch = Vec::with_capacity(batch_size);
        let mut line_num = preprocessing_line_count;
        let mut batch_start_line = 1usize;
        let mut pending_deadline: Option<Instant> = None;
        let mut skipped_lines_count = 0usize;
        let mut filtered_lines = 0usize;

        let ctrl_rx = ctrl_rx;

        'outer: loop {
            if let Some(deadline) = pending_deadline {
                let now = Instant::now();
                if deadline <= now {
                    if !current_batch.is_empty() {
                        Self::send_batch(
                            &batch_sender,
                            &mut current_batch,
                            batch_id,
                            batch_start_line,
                        )?;
                        batch_id += 1;
                        batch_start_line = line_num + 1;
                    }
                    pending_deadline = None;
                    continue;
                }

                let wait = deadline - now;
                let timeout = crossbeam_channel::after(wait);
                select! {
                    recv(ctrl_rx) -> msg => {
                        match msg {
                            Ok(Ctrl::Shutdown { immediate }) => {
                                if !current_batch.is_empty() && !immediate {
                                    Self::send_batch(
                                        &batch_sender,
                                        &mut current_batch,
                                        batch_id,
                                        batch_start_line,
                                    )?;
                                }
                                break 'outer;
                            }
                            Err(_) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch(
                                        &batch_sender,
                                        &mut current_batch,
                                        batch_id,
                                        batch_start_line,
                                    )?;
                                }
                                break 'outer;
                            }
                        }
                    }
                    recv(line_receiver) -> msg => {
                        match msg {
                            Ok(LineMessage::Line { line, .. }) => {
                                Self::handle_plain_line(
                                    line,
                                    &batch_sender,
                                    &mut current_batch,
                                    batch_size,
                                    batch_timeout,
                                    &mut batch_id,
                                    &mut batch_start_line,
                                    &mut line_num,
                                    &mut skipped_lines_count,
                                    &mut filtered_lines,
                                    skip_lines,
                                    &input_format,
                                    &ignore_lines,
                                    &mut pending_deadline,
                                )?;
                            }
                            Ok(LineMessage::Error { error, .. }) => return Err(error.into()),
                            Ok(LineMessage::Eof) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch(
                                        &batch_sender,
                                        &mut current_batch,
                                        batch_id,
                                        batch_start_line,
                                    )?;
                                }
                                break 'outer;
                            }
                            Err(_) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch(
                                        &batch_sender,
                                        &mut current_batch,
                                        batch_id,
                                        batch_start_line,
                                    )?;
                                }
                                break 'outer;
                            }
                        }
                    }
                    recv(timeout) -> _ => {
                        if !current_batch.is_empty() {
                            Self::send_batch(
                                &batch_sender,
                                &mut current_batch,
                                batch_id,
                                batch_start_line,
                            )?;
                            batch_id += 1;
                            batch_start_line = line_num + 1;
                        }
                        pending_deadline = None;
                    }
                }
            } else {
                select! {
                    recv(ctrl_rx) -> msg => {
                        match msg {
                            Ok(Ctrl::Shutdown { immediate }) => {
                                if !current_batch.is_empty() && !immediate {
                                    Self::send_batch(
                                        &batch_sender,
                                        &mut current_batch,
                                        batch_id,
                                        batch_start_line,
                                    )?;
                                }
                                break 'outer;
                            }
                            Err(_) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch(
                                        &batch_sender,
                                        &mut current_batch,
                                        batch_id,
                                        batch_start_line,
                                    )?;
                                }
                                break 'outer;
                            }
                        }
                    }
                    recv(line_receiver) -> msg => {
                        match msg {
                            Ok(LineMessage::Line { line, .. }) => {
                                Self::handle_plain_line(
                                    line,
                                    &batch_sender,
                                    &mut current_batch,
                                    batch_size,
                                    batch_timeout,
                                    &mut batch_id,
                                    &mut batch_start_line,
                                    &mut line_num,
                                    &mut skipped_lines_count,
                                    &mut filtered_lines,
                                    skip_lines,
                                    &input_format,
                                    &ignore_lines,
                                    &mut pending_deadline,
                                )?;
                            }
                            Ok(LineMessage::Error { error, .. }) => return Err(error.into()),
                            Ok(LineMessage::Eof) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch(
                                        &batch_sender,
                                        &mut current_batch,
                                        batch_id,
                                        batch_start_line,
                                    )?;
                                }
                                break 'outer;
                            }
                            Err(_) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch(
                                        &batch_sender,
                                        &mut current_batch,
                                        batch_id,
                                        batch_start_line,
                                    )?;
                                }
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }

        global_tracker.set_total_lines_read(line_num)?;
        global_tracker.add_lines_filtered(filtered_lines)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn file_aware_batcher_thread(
        line_receiver: Receiver<LineMessage>,
        batch_sender: Sender<Batch>,
        batch_size: usize,
        batch_timeout: Duration,
        global_tracker: GlobalTracker,
        ignore_lines: Option<regex::Regex>,
        skip_lines: usize,
        input_format: crate::config::InputFormat,
        ctrl_rx: Receiver<Ctrl>,
    ) -> Result<()> {
        let mut batch_id = 0u64;
        let mut current_batch = Vec::with_capacity(batch_size);
        let mut current_filenames = Vec::with_capacity(batch_size);
        let mut line_num = 0usize;
        let mut batch_start_line = 1usize;
        let mut pending_deadline: Option<Instant> = None;
        let mut skipped_lines_count = 0usize;
        let mut filtered_lines = 0usize;
        let mut last_filename: Option<String> = None;
        let mut current_headers: Option<Vec<String>> = None;

        let ctrl_rx = ctrl_rx;

        'outer: loop {
            if let Some(deadline) = pending_deadline {
                let now = Instant::now();
                if deadline <= now {
                    if !current_batch.is_empty() {
                        Self::send_batch_with_filenames_and_headers(
                            &batch_sender,
                            &mut current_batch,
                            &mut current_filenames,
                            batch_id,
                            batch_start_line,
                            current_headers.clone(),
                        )?;
                        batch_id += 1;
                        batch_start_line = line_num + 1;
                    }
                    pending_deadline = None;
                    continue;
                }

                let wait = deadline - now;
                let timeout = crossbeam_channel::after(wait);
                select! {
                    recv(ctrl_rx) -> msg => {
                        match msg {
                            Ok(Ctrl::Shutdown { immediate }) => {
                                if !current_batch.is_empty() && !immediate {
                                    Self::send_batch_with_filenames_and_headers(
                                        &batch_sender,
                                        &mut current_batch,
                                        &mut current_filenames,
                                        batch_id,
                                        batch_start_line,
                                        current_headers.clone(),
                                    )?;
                                }
                                break 'outer;
                            }
                            Err(_) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch_with_filenames_and_headers(
                                        &batch_sender,
                                        &mut current_batch,
                                        &mut current_filenames,
                                        batch_id,
                                        batch_start_line,
                                        current_headers.clone(),
                                    )?;
                                }
                                break 'outer;
                            }
                        }
                    }
                    recv(line_receiver) -> msg => {
                        match msg {
                            Ok(LineMessage::Line { line, filename }) => {
                                Self::handle_file_aware_line(
                                    line,
                                    filename,
                                    &batch_sender,
                                    &mut current_batch,
                                    &mut current_filenames,
                                    batch_size,
                                    batch_timeout,
                                    &mut batch_id,
                                    &mut batch_start_line,
                                    &mut line_num,
                                    &mut skipped_lines_count,
                                    &mut filtered_lines,
                                    skip_lines,
                                    &input_format,
                                    &ignore_lines,
                                    &mut pending_deadline,
                                    &mut current_headers,
                                    &mut last_filename,
                                )?;
                            }
                            Ok(LineMessage::Error { error, .. }) => return Err(error.into()),
                            Ok(LineMessage::Eof) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch_with_filenames_and_headers(
                                        &batch_sender,
                                        &mut current_batch,
                                        &mut current_filenames,
                                        batch_id,
                                        batch_start_line,
                                        current_headers.clone(),
                                    )?;
                                }
                                break 'outer;
                            }
                            Err(_) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch_with_filenames_and_headers(
                                        &batch_sender,
                                        &mut current_batch,
                                        &mut current_filenames,
                                        batch_id,
                                        batch_start_line,
                                        current_headers.clone(),
                                    )?;
                                }
                                break 'outer;
                            }
                        }
                    }
                    recv(timeout) -> _ => {
                        if !current_batch.is_empty() {
                            Self::send_batch_with_filenames_and_headers(
                                &batch_sender,
                                &mut current_batch,
                                &mut current_filenames,
                                batch_id,
                                batch_start_line,
                                current_headers.clone(),
                            )?;
                            batch_id += 1;
                            batch_start_line = line_num + 1;
                        }
                        pending_deadline = None;
                    }
                }
            } else {
                select! {
                    recv(ctrl_rx) -> msg => {
                        match msg {
                            Ok(Ctrl::Shutdown { immediate }) => {
                                if !current_batch.is_empty() && !immediate {
                                    Self::send_batch_with_filenames_and_headers(
                                        &batch_sender,
                                        &mut current_batch,
                                        &mut current_filenames,
                                        batch_id,
                                        batch_start_line,
                                        current_headers.clone(),
                                    )?;
                                }
                                break 'outer;
                            }
                            Err(_) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch_with_filenames_and_headers(
                                        &batch_sender,
                                        &mut current_batch,
                                        &mut current_filenames,
                                        batch_id,
                                        batch_start_line,
                                        current_headers.clone(),
                                    )?;
                                }
                                break 'outer;
                            }
                        }
                    }
                    recv(line_receiver) -> msg => {
                        match msg {
                            Ok(LineMessage::Line { line, filename }) => {
                                Self::handle_file_aware_line(
                                    line,
                                    filename,
                                    &batch_sender,
                                    &mut current_batch,
                                    &mut current_filenames,
                                    batch_size,
                                    batch_timeout,
                                    &mut batch_id,
                                    &mut batch_start_line,
                                    &mut line_num,
                                    &mut skipped_lines_count,
                                    &mut filtered_lines,
                                    skip_lines,
                                    &input_format,
                                    &ignore_lines,
                                    &mut pending_deadline,
                                    &mut current_headers,
                                    &mut last_filename,
                                )?;
                            }
                            Ok(LineMessage::Error { error, .. }) => return Err(error.into()),
                            Ok(LineMessage::Eof) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch_with_filenames_and_headers(
                                        &batch_sender,
                                        &mut current_batch,
                                        &mut current_filenames,
                                        batch_id,
                                        batch_start_line,
                                        current_headers.clone(),
                                    )?;
                                }
                                break 'outer;
                            }
                            Err(_) => {
                                if !current_batch.is_empty() {
                                    Self::send_batch_with_filenames_and_headers(
                                        &batch_sender,
                                        &mut current_batch,
                                        &mut current_filenames,
                                        batch_id,
                                        batch_start_line,
                                        current_headers.clone(),
                                    )?;
                                }
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }

        global_tracker.set_total_lines_read(line_num)?;
        global_tracker.add_lines_filtered(filtered_lines)?;

        Ok(())
    }

    fn handle_plain_line(
        line: String,
        batch_sender: &Sender<Batch>,
        current_batch: &mut Vec<String>,
        batch_size: usize,
        batch_timeout: Duration,
        batch_id: &mut u64,
        batch_start_line: &mut usize,
        line_num: &mut usize,
        skipped_lines_count: &mut usize,
        filtered_lines: &mut usize,
        skip_lines: usize,
        input_format: &crate::config::InputFormat,
        ignore_lines: &Option<regex::Regex>,
        pending_deadline: &mut Option<Instant>,
    ) -> Result<()> {
        *line_num += 1;

        if *skipped_lines_count < skip_lines {
            *skipped_lines_count += 1;
            *filtered_lines += 1;
            return Ok(());
        }

        if line.is_empty() && !matches!(input_format, crate::config::InputFormat::Line) {
            return Ok(());
        }

        if let Some(ref ignore_regex) = ignore_lines {
            if ignore_regex.is_match(&line) {
                *filtered_lines += 1;
                return Ok(());
            }
        }

        current_batch.push(line);

        if current_batch.len() >= batch_size || batch_timeout.is_zero() {
            Self::send_batch(batch_sender, current_batch, *batch_id, *batch_start_line)?;
            *batch_id += 1;
            *batch_start_line = *line_num + 1;
            *pending_deadline = None;
        } else {
            *pending_deadline = Some(Instant::now() + batch_timeout);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_file_aware_line(
        line: String,
        filename: Option<String>,
        batch_sender: &Sender<Batch>,
        current_batch: &mut Vec<String>,
        current_filenames: &mut Vec<Option<String>>,
        batch_size: usize,
        batch_timeout: Duration,
        batch_id: &mut u64,
        batch_start_line: &mut usize,
        line_num: &mut usize,
        skipped_lines_count: &mut usize,
        filtered_lines: &mut usize,
        skip_lines: usize,
        input_format: &crate::config::InputFormat,
        ignore_lines: &Option<regex::Regex>,
        pending_deadline: &mut Option<Instant>,
        current_headers: &mut Option<Vec<String>>,
        last_filename: &mut Option<String>,
    ) -> Result<()> {
        *line_num += 1;

        if *skipped_lines_count < skip_lines {
            *skipped_lines_count += 1;
            *filtered_lines += 1;
            return Ok(());
        }

        if line.is_empty() && !matches!(input_format, crate::config::InputFormat::Line) {
            return Ok(());
        }

        if let Some(ref ignore_regex) = ignore_lines {
            if ignore_regex.is_match(&line) {
                *filtered_lines += 1;
                return Ok(());
            }
        }

        let filename_changed = match (&filename, &*last_filename) {
            (Some(new), Some(prev)) => new != prev,
            (None, None) => false,
            _ => true,
        };

        if matches!(
            input_format,
            crate::config::InputFormat::Csv
                | crate::config::InputFormat::Tsv
                | crate::config::InputFormat::Csvnh
                | crate::config::InputFormat::Tsvnh
        ) && filename_changed
        {
            if !current_batch.is_empty() {
                Self::send_batch_with_filenames_and_headers(
                    batch_sender,
                    current_batch,
                    current_filenames,
                    *batch_id,
                    *batch_start_line,
                    current_headers.clone(),
                )?;
                *batch_id += 1;
                *batch_start_line = *line_num + 1;
                *pending_deadline = None;
            }

            *current_headers = Self::create_csv_parser_for_file(input_format, &line)
                .map(|parser| parser.get_headers());
            *last_filename = filename.clone();

            if matches!(
                input_format,
                crate::config::InputFormat::Csv | crate::config::InputFormat::Tsv
            ) {
                return Ok(());
            }
        } else if filename_changed {
            *last_filename = filename.clone();
        }

        current_batch.push(line);
        current_filenames.push(filename);

        if current_batch.len() >= batch_size || batch_timeout.is_zero() {
            Self::send_batch_with_filenames_and_headers(
                batch_sender,
                current_batch,
                current_filenames,
                *batch_id,
                *batch_start_line,
                current_headers.clone(),
            )?;
            *batch_id += 1;
            *batch_start_line = *line_num + 1;
            *pending_deadline = None;
        } else {
            *pending_deadline = Some(Instant::now() + batch_timeout);
        }

        Ok(())
    }

    fn create_csv_parser_for_file(
        input_format: &crate::config::InputFormat,
        first_line: &str,
    ) -> Option<crate::parsers::CsvParser> {
        let mut parser = match input_format {
            crate::config::InputFormat::Csv => crate::parsers::CsvParser::new_csv(),
            crate::config::InputFormat::Tsv => crate::parsers::CsvParser::new_tsv(),
            crate::config::InputFormat::Csvnh => crate::parsers::CsvParser::new_csv_no_headers(),
            crate::config::InputFormat::Tsvnh => crate::parsers::CsvParser::new_tsv_no_headers(),
            _ => return None,
        };

        if parser.initialize_headers_from_line(first_line).is_ok() {
            Some(parser)
        } else {
            None
        }
    }

    fn send_batch(
        batch_sender: &Sender<Batch>,
        current_batch: &mut Vec<String>,
        batch_id: u64,
        batch_start_line: usize,
    ) -> Result<()> {
        if current_batch.is_empty() {
            return Ok(());
        }

        let batch_len = current_batch.len();
        let batch = Batch {
            id: batch_id,
            lines: std::mem::take(current_batch),
            start_line_num: batch_start_line,
            filenames: vec![None; batch_len], // No filename tracking for regular batches
            csv_headers: None,                // No CSV headers for regular batches
        };

        if batch_sender.send(batch).is_err() {
            return Err(anyhow::anyhow!("Channel closed"));
        }

        Ok(())
    }

    fn send_batch_with_filenames_and_headers(
        batch_sender: &Sender<Batch>,
        current_batch: &mut Vec<String>,
        current_filenames: &mut Vec<Option<String>>,
        batch_id: u64,
        batch_start_line: usize,
        csv_headers: Option<Vec<String>>,
    ) -> Result<()> {
        if current_batch.is_empty() {
            return Ok(());
        }

        let batch = Batch {
            id: batch_id,
            lines: std::mem::take(current_batch),
            start_line_num: batch_start_line,
            filenames: std::mem::take(current_filenames),
            csv_headers,
        };

        if batch_sender.send(batch).is_err() {
            return Err(anyhow::anyhow!("Channel closed"));
        }

        Ok(())
    }

    /// Worker thread: processes batches in parallel
    fn worker_thread(
        _worker_id: usize,
        batch_receiver: Receiver<Batch>,
        result_sender: Sender<BatchResult>,
        pipeline_builder: PipelineBuilder,
        stages: Vec<crate::config::ScriptStageType>,
        multiline_timeout: Option<Duration>,
        ctrl_rx: Receiver<Ctrl>,
    ) -> Result<()> {
        crate::rhai_functions::strings::set_parallel_mode(true);

        stats_start_timer();

        let (mut pipeline, mut ctx) = pipeline_builder.clone().build_worker(stages.clone())?;

        let mut current_csv_headers: Option<Vec<String>> = None;
        let mut immediate_shutdown = false;

        let ctrl_rx = ctrl_rx;
        let batch_receiver = batch_receiver;

        'worker_loop: loop {
            if immediate_shutdown {
                break;
            }

            let flush_deadline = multiline_timeout
                .filter(|_| pipeline.has_pending_chunk())
                .map(|timeout| Instant::now() + timeout);

            if let Some(deadline) = flush_deadline {
                let wait = deadline.saturating_duration_since(Instant::now());
                let timeout = crossbeam_channel::after(wait);
                select! {
                    recv(ctrl_rx) -> msg => {
                        if Self::handle_worker_ctrl(
                            msg,
                            &mut immediate_shutdown,
                            &mut pipeline,
                            &mut ctx,
                            &result_sender,
                        )? {
                            break 'worker_loop;
                        }
                    }
                    recv(batch_receiver) -> msg => {
                        match msg {
                            Ok(batch) => {
                                if !Self::worker_process_batch(
                                    batch,
                                    &mut pipeline,
                                    &mut ctx,
                                    &pipeline_builder,
                                    &stages,
                                    &result_sender,
                                    &mut current_csv_headers,
                                )? {
                                    break 'worker_loop;
                                }
                            }
                            Err(_) => break 'worker_loop,
                        }
                    }
                    recv(timeout) -> _ => {
                        Self::worker_flush_pipeline(
                            &mut pipeline,
                            &mut ctx,
                            &result_sender,
                            false,
                        )?;
                    }
                }
            } else {
                select! {
                    recv(ctrl_rx) -> msg => {
                        if Self::handle_worker_ctrl(
                            msg,
                            &mut immediate_shutdown,
                            &mut pipeline,
                            &mut ctx,
                            &result_sender,
                        )? {
                            break 'worker_loop;
                        }
                    }
                    recv(batch_receiver) -> msg => {
                        match msg {
                            Ok(batch) => {
                                if !Self::worker_process_batch(
                                    batch,
                                    &mut pipeline,
                                    &mut ctx,
                                    &pipeline_builder,
                                    &stages,
                                    &result_sender,
                                    &mut current_csv_headers,
                                )? {
                                    break 'worker_loop;
                                }
                            }
                            Err(_) => break 'worker_loop,
                        }
                    }
                }
            }
        }

        if !immediate_shutdown {
            Self::worker_flush_pipeline(&mut pipeline, &mut ctx, &result_sender, true)?;
        }

        stats_finish_processing();

        Ok(())
    }

    fn handle_worker_ctrl(
        msg: Result<Ctrl, crossbeam_channel::RecvError>,
        immediate_shutdown: &mut bool,
        pipeline: &mut pipeline::Pipeline,
        ctx: &mut pipeline::PipelineContext,
        result_sender: &Sender<BatchResult>,
    ) -> Result<bool> {
        match msg {
            Ok(Ctrl::Shutdown { immediate }) => {
                if immediate {
                    *immediate_shutdown = true;
                    return Ok(true);
                }

                Self::worker_flush_pipeline(pipeline, ctx, result_sender, false)?;
                Ok(false)
            }
            Err(_) => {
                // Treat channel closure as graceful shutdown request
                Self::worker_flush_pipeline(pipeline, ctx, result_sender, false)?;
                Ok(true)
            }
        }
    }

    fn worker_flush_pipeline(
        pipeline: &mut pipeline::Pipeline,
        ctx: &mut pipeline::PipelineContext,
        result_sender: &Sender<BatchResult>,
        final_flush: bool,
    ) -> Result<()> {
        match pipeline.flush(ctx) {
            Ok(mut flush_results) => {
                if final_flush {
                    if let Some(trailing) = pipeline.finish_formatter() {
                        if !trailing.line.is_empty() {
                            flush_results.push(trailing);
                        }
                    }
                }

                if flush_results.is_empty() {
                    return Ok(());
                }

                let mut flush_batch_results = Vec::with_capacity(flush_results.len());
                for formatted_result in flush_results {
                    let mut dummy_event = Event::default_with_line(formatted_result.line);
                    dummy_event.set_metadata(0, None);

                    flush_batch_results.push(ProcessedEvent {
                        event: dummy_event,
                        captured_prints: Vec::new(),
                        captured_eprints: Vec::new(),
                        captured_messages: Vec::new(),
                        timestamp: formatted_result.timestamp,
                    });
                }

                let mut flush_tracking_updates = HashMap::new();
                for (key, value) in &ctx.tracker {
                    if key.starts_with("__kelora_stats_") || key.starts_with("__op___kelora_stats_")
                    {
                        flush_tracking_updates.insert(key.clone(), value.clone());
                    }
                }

                let thread_tracking = crate::rhai_functions::tracking::get_thread_tracking_state();
                for (key, value) in thread_tracking {
                    flush_tracking_updates.insert(key, value);
                }

                let flush_batch_result = BatchResult {
                    batch_id: u64::MAX - 1,
                    results: flush_batch_results,
                    internal_tracked_updates: flush_tracking_updates,
                    worker_stats: ProcessingStats::new(),
                };

                let _ = result_sender.send(flush_batch_result);
                Ok(())
            }
            Err(e) => {
                if ctx.config.strict {
                    return Err(e);
                }
                eprintln!("Warning: Failed to flush worker pipeline: {}", e);
                Ok(())
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn worker_process_batch(
        batch: Batch,
        pipeline: &mut pipeline::Pipeline,
        ctx: &mut pipeline::PipelineContext,
        pipeline_builder: &PipelineBuilder,
        stages: &[crate::config::ScriptStageType],
        result_sender: &Sender<BatchResult>,
        current_csv_headers: &mut Option<Vec<String>>,
    ) -> Result<bool> {
        if batch.csv_headers.is_some() && batch.csv_headers != *current_csv_headers {
            *current_csv_headers = batch.csv_headers.clone();

            let new_pipeline_builder = pipeline_builder
                .clone()
                .with_csv_headers(current_csv_headers.clone().unwrap());
            let (new_pipeline, new_ctx) = new_pipeline_builder.build_worker(stages.to_vec())?;
            *pipeline = new_pipeline;
            ctx.rhai = new_ctx.rhai;
        }

        let before = (
            ctx.tracker
                .get("__kelora_stats_output")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
            ctx.tracker
                .get("__kelora_stats_lines_errors")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
            ctx.tracker
                .get("__kelora_stats_events_created")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
            ctx.tracker
                .get("__kelora_stats_events_output")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
            ctx.tracker
                .get("__kelora_stats_events_filtered")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
        );

        let mut batch_results = Vec::with_capacity(batch.lines.len());

        for (line_idx, line) in batch.lines.iter().enumerate() {
            let current_line_num = batch.start_line_num + line_idx;
            ctx.meta.line_num = Some(current_line_num);
            ctx.meta.filename = batch.filenames.get(line_idx).cloned().flatten();

            crate::rhai_functions::strings::clear_captured_prints();
            crate::rhai_functions::strings::clear_captured_eprints();

            match pipeline.process_line(line.clone(), ctx) {
                Ok(formatted_results) => {
                    if !formatted_results.is_empty() {
                        ctx.tracker
                            .entry("__kelora_stats_output".to_string())
                            .and_modify(|v| *v = Dynamic::from(v.as_int().unwrap_or(0) + 1))
                            .or_insert(Dynamic::from(1i64));
                        ctx.tracker.insert(
                            "__op___kelora_stats_output".to_string(),
                            Dynamic::from("count"),
                        );
                    }

                    let captured_prints = crate::rhai_functions::strings::take_captured_prints();
                    let captured_eprints = crate::rhai_functions::strings::take_captured_eprints();
                    let captured_messages =
                        crate::rhai_functions::strings::take_captured_messages();

                    if formatted_results.is_empty()
                        && (!captured_prints.is_empty()
                            || !captured_eprints.is_empty()
                            || !captured_messages.is_empty())
                    {
                        let dummy_event = Event::default_with_line(String::new());
                        batch_results.push(ProcessedEvent {
                            event: dummy_event,
                            captured_prints,
                            captured_eprints,
                            captured_messages,
                            timestamp: None,
                        });
                    } else {
                        for formatted_result in formatted_results {
                            let mut dummy_event = Event::default_with_line(formatted_result.line);
                            dummy_event.set_metadata(current_line_num, None);

                            batch_results.push(ProcessedEvent {
                                event: dummy_event,
                                captured_prints: captured_prints.clone(),
                                captured_eprints: captured_eprints.clone(),
                                captured_messages: captured_messages.clone(),
                                timestamp: formatted_result.timestamp,
                            });
                        }
                    }
                }
                Err(e) => {
                    let captured_eprints = crate::rhai_functions::strings::take_captured_eprints();
                    let captured_messages =
                        crate::rhai_functions::strings::take_captured_messages();

                    if !captured_eprints.is_empty() || !captured_messages.is_empty() {
                        let dummy_event = Event::default_with_line(String::new());
                        batch_results.push(ProcessedEvent {
                            event: dummy_event,
                            captured_prints: Vec::new(),
                            captured_eprints,
                            captured_messages,
                            timestamp: None,
                        });
                    }

                    if ctx.config.strict {
                        return Err(e);
                    } else {
                        continue;
                    }
                }
            }

            if crate::rhai_functions::process::is_exit_requested() {
                let exit_code = crate::rhai_functions::process::get_exit_code();
                std::process::exit(exit_code);
            }
        }

        let after = (
            ctx.tracker
                .get("__kelora_stats_output")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
            ctx.tracker
                .get("__kelora_stats_lines_errors")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
            ctx.tracker
                .get("__kelora_stats_events_created")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
            ctx.tracker
                .get("__kelora_stats_events_output")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
            ctx.tracker
                .get("__kelora_stats_events_filtered")
                .and_then(|v| v.as_int().ok())
                .unwrap_or(0),
        );

        let mut deltas = std::collections::HashMap::new();
        if after.0 > before.0 {
            deltas.insert(
                "__kelora_stats_output".to_string(),
                Dynamic::from(after.0 - before.0),
            );
            deltas.insert(
                "__op___kelora_stats_output".to_string(),
                Dynamic::from("count"),
            );
        }
        if after.1 > before.1 {
            deltas.insert(
                "__kelora_stats_lines_errors".to_string(),
                Dynamic::from(after.1 - before.1),
            );
            deltas.insert(
                "__op___kelora_stats_lines_errors".to_string(),
                Dynamic::from("count"),
            );
        }
        if after.2 > before.2 {
            deltas.insert(
                "__kelora_stats_events_created".to_string(),
                Dynamic::from(after.2 - before.2),
            );
            deltas.insert(
                "__op___kelora_stats_events_created".to_string(),
                Dynamic::from("count"),
            );
        }
        if after.3 > before.3 {
            deltas.insert(
                "__kelora_stats_events_output".to_string(),
                Dynamic::from(after.3 - before.3),
            );
            deltas.insert(
                "__op___kelora_stats_events_output".to_string(),
                Dynamic::from("count"),
            );
        }
        if after.4 > before.4 {
            deltas.insert(
                "__kelora_stats_events_filtered".to_string(),
                Dynamic::from(after.4 - before.4),
            );
            deltas.insert(
                "__op___kelora_stats_events_filtered".to_string(),
                Dynamic::from("count"),
            );
        }

        for (key, value) in &ctx.tracker {
            if !key.starts_with("__kelora_stats_") && !key.starts_with("__op___kelora_stats_") {
                deltas.insert(key.clone(), value.clone());
            }
        }

        let thread_tracking = crate::rhai_functions::tracking::get_thread_tracking_state();
        for (key, value) in thread_tracking {
            if (!key.starts_with("__op___kelora_stats_")
                || key == "__op___kelora_stats_discovered_levels"
                || key == "__op___kelora_stats_discovered_keys")
                && (!key.starts_with("__kelora_stats_")
                    || key == "__kelora_stats_discovered_levels"
                    || key == "__kelora_stats_discovered_keys")
            {
                deltas.insert(key, value);
            }
        }

        let batch_result = BatchResult {
            batch_id: batch.id,
            results: batch_results,
            internal_tracked_updates: deltas,
            worker_stats: get_thread_stats(),
        };

        if result_sender.send(batch_result).is_err() {
            return Ok(false);
        }

        ctx.tracker.retain(|k, _| {
            k.starts_with("__kelora_stats_") || k.starts_with("__op___kelora_stats_")
        });

        Ok(true)
    }

    /// Write CSV header if the output format requires it
    fn write_csv_header_if_needed<W: std::io::Write>(
        output: &mut W,
        config: &crate::config::KeloraConfig,
    ) -> Result<()> {
        // Only write headers for CSV formats that normally include headers
        match config.output.format {
            crate::config::OutputFormat::Csv | crate::config::OutputFormat::Tsv => {
                // Create a temporary formatter to generate the header
                let keys = config.output.get_effective_keys();
                if keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "CSV output format requires --keys to specify field order"
                    ));
                }

                let formatter = match config.output.format {
                    crate::config::OutputFormat::Csv => crate::formatters::CsvFormatter::new(keys),
                    crate::config::OutputFormat::Tsv => {
                        crate::formatters::CsvFormatter::new_tsv(keys)
                    }
                    _ => unreachable!(),
                };

                // Generate and write the header
                let header = formatter.format_header();
                writeln!(output, "{}", header)?;
            }
            _ => {
                // Non-CSV formats don't need headers
            }
        }
        Ok(())
    }

    /// Pipeline result sink thread: handles output ordering and merges global state
    /// Results are already formatted by the pipeline, so we just need to output them
    fn pipeline_result_sink_thread<W: std::io::Write>(
        result_receiver: Receiver<BatchResult>,
        preserve_order: bool,
        global_tracker: GlobalTracker,
        output: &mut W,
        config: &crate::config::KeloraConfig,
        take_limit: Option<usize>,
    ) -> Result<()> {
        // Write CSV header if needed (before any worker results)
        Self::write_csv_header_if_needed(output, config)?;

        let gap_marker_use_colors =
            crate::tty::should_use_colors_with_mode(&config.output.color);
        let mut gap_tracker = config.output.mark_gaps.map(|threshold| {
            GapTracker::new(threshold, gap_marker_use_colors)
        });

        if preserve_order {
            Self::pipeline_ordered_result_sink(
                result_receiver,
                global_tracker,
                output,
                take_limit,
                &mut gap_tracker,
            )
        } else {
            Self::pipeline_unordered_result_sink(
                result_receiver,
                global_tracker,
                output,
                take_limit,
                &mut gap_tracker,
            )
        }
    }

    fn pipeline_ordered_result_sink<W: std::io::Write>(
        result_receiver: Receiver<BatchResult>,
        global_tracker: GlobalTracker,
        output: &mut W,
        take_limit: Option<usize>,
        gap_tracker: &mut Option<GapTracker>,
    ) -> Result<()> {
        let mut pending_batches: HashMap<u64, BatchResult> = HashMap::new();
        let mut next_expected_id = 0u64;
        let mut events_output = 0usize;

        let mut termination_detected = false;
        while let Ok(mut batch_result) = result_receiver.recv() {
            // Check for termination signal, but don't break immediately
            // Continue processing to collect final stats from workers
            if SHOULD_TERMINATE.load(Ordering::Relaxed) {
                termination_detected = true;
            }

            let batch_id = batch_result.batch_id;
            let internal_tracked_updates =
                std::mem::take(&mut batch_result.internal_tracked_updates);

            // Merge global state and stats
            global_tracker.merge_worker_state(internal_tracked_updates)?;
            global_tracker.merge_worker_stats(&batch_result.worker_stats)?;

            // Handle special batches
            if batch_id == u64::MAX {
                // This is a final stats batch from a terminated worker
                // If we're terminating, we might want to exit soon after collecting these
                if termination_detected {
                    // Continue processing a bit more to collect other final stats
                    continue;
                }
                continue;
            } else if batch_id == u64::MAX - 1 {
                // This is a flush batch from a worker - process it immediately
                if !termination_detected {
                    let remaining_limit =
                        take_limit.map(|limit| limit.saturating_sub(events_output));
                    let events_this_batch = Self::pipeline_output_batch_results(
                        output,
                        &batch_result.results,
                        remaining_limit,
                        gap_tracker,
                    )?;
                    events_output += events_this_batch;

                    // Check if we've reached the take limit
                    if let Some(limit) = take_limit {
                        if events_output >= limit {
                            // Set termination signal to stop further processing
                            SHOULD_TERMINATE.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }
                continue;
            }

            // If terminating, skip output processing but continue stats collection
            if termination_detected {
                continue;
            }

            // Store batch for ordering
            pending_batches.insert(batch_id, batch_result);

            // Output all consecutive batches starting from next_expected_id
            while let Some(batch) = pending_batches.remove(&next_expected_id) {
                let remaining_limit = take_limit.map(|limit| limit.saturating_sub(events_output));
                let events_this_batch = Self::pipeline_output_batch_results(
                    output,
                    &batch.results,
                    remaining_limit,
                    gap_tracker,
                )?;
                events_output += events_this_batch;
                next_expected_id += 1;

                // Check if we've reached the take limit
                if let Some(limit) = take_limit {
                    if events_output >= limit {
                        // Set termination signal to stop further processing
                        SHOULD_TERMINATE.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
        }

        // Output any remaining batches (shouldn't happen with proper shutdown)
        for (_, batch) in pending_batches {
            let remaining_limit = take_limit.map(|limit| limit.saturating_sub(events_output));
            events_output += Self::pipeline_output_batch_results(
                output,
                &batch.results,
                remaining_limit,
                gap_tracker,
            )?;

            // Check if we've reached the take limit even in cleanup
            if let Some(limit) = take_limit {
                if events_output >= limit {
                    break;
                }
            }
        }

        Ok(())
    }

    fn pipeline_unordered_result_sink<W: std::io::Write>(
        result_receiver: Receiver<BatchResult>,
        global_tracker: GlobalTracker,
        output: &mut W,
        take_limit: Option<usize>,
        gap_tracker: &mut Option<GapTracker>,
    ) -> Result<()> {
        let mut termination_detected = false;
        let mut events_output = 0usize;
        while let Ok(batch_result) = result_receiver.recv() {
            // Check for termination signal, but don't break immediately
            // Continue processing to collect final stats from workers
            if SHOULD_TERMINATE.load(Ordering::Relaxed) {
                termination_detected = true;
            }

            // Merge global state and stats
            global_tracker.merge_worker_state(batch_result.internal_tracked_updates)?;
            global_tracker.merge_worker_stats(&batch_result.worker_stats)?;

            // Handle special batches
            if batch_result.batch_id == u64::MAX {
                // This is a final stats batch from a terminated worker
                if termination_detected {
                    // Continue processing a bit more to collect other final stats
                    continue;
                }
                continue;
            } else if batch_result.batch_id == u64::MAX - 1 {
                // This is a flush batch from a worker - process it immediately
                if !termination_detected {
                    let remaining_limit =
                        take_limit.map(|limit| limit.saturating_sub(events_output));
                    let events_this_batch = Self::pipeline_output_batch_results(
                        output,
                        &batch_result.results,
                        remaining_limit,
                        gap_tracker,
                    )?;
                    events_output += events_this_batch;

                    // Check if we've reached the take limit
                    if let Some(limit) = take_limit {
                        if events_output >= limit {
                            // Set termination signal to stop further processing
                            SHOULD_TERMINATE.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }
                continue;
            }

            // If terminating, skip output processing but continue stats collection
            if termination_detected {
                continue;
            }

            // Output immediately
            let remaining_limit = take_limit.map(|limit| limit.saturating_sub(events_output));
            let events_this_batch = Self::pipeline_output_batch_results(
                output,
                &batch_result.results,
                remaining_limit,
                gap_tracker,
            )?;
            events_output += events_this_batch;

            // Check if we've reached the take limit
            if let Some(limit) = take_limit {
                if events_output >= limit {
                    // Set termination signal to stop further processing
                    SHOULD_TERMINATE.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }

        Ok(())
    }

    fn pipeline_output_batch_results<W: std::io::Write>(
        output: &mut W,
        results: &[ProcessedEvent],
        remaining_limit: Option<usize>,
        gap_tracker: &mut Option<GapTracker>,
    ) -> Result<usize> {
        let mut events_output = 0usize;

        for processed in results {
            // Check if we've reached the limit
            if let Some(limit) = remaining_limit {
                if events_output >= limit {
                    break;
                }
            }

            // Output captured messages in order, preserving stdout/stderr streams
            if !processed.captured_messages.is_empty() {
                // Use the new ordered message system
                for message in &processed.captured_messages {
                    match message {
                        crate::rhai_functions::strings::CapturedMessage::Stdout(msg) => {
                            println!("{}", msg);
                        }
                        crate::rhai_functions::strings::CapturedMessage::Stderr(msg) => {
                            eprintln!("{}", msg);
                        }
                    }
                }
            } else {
                // Fallback to old system for compatibility
                // First output any captured prints for this specific event (to stdout, not file)
                for print_msg in &processed.captured_prints {
                    println!("{}", print_msg);
                }

                // Output any captured eprints for this specific event (to stderr)
                for eprint_msg in &processed.captured_eprints {
                    eprintln!("{}", eprint_msg);
                }
            }

            // Then output the event itself to the designated output, skip empty strings
            if !processed.event.original_line.is_empty() {
                let marker = match gap_tracker.as_mut() {
                    Some(tracker) => tracker.check(processed.timestamp),
                    None => None,
                };

                if let Some(marker_line) = marker {
                    writeln!(output, "{}", marker_line).unwrap_or(());
                }

                writeln!(output, "{}", &processed.event.original_line).unwrap_or(());
                events_output += 1;
            }
        }

        output.flush().unwrap_or(());
        Ok(events_output)
    }

    /// Preprocess CSV headers and return a reader that includes the first line if it's data
    fn preprocess_csv_with_reader<R: std::io::BufRead + Send + 'static>(
        mut reader: R,
        mut pipeline_builder: PipelineBuilder,
        config: &crate::config::KeloraConfig,
    ) -> Result<(Box<dyn std::io::BufRead + Send>, PipelineBuilder, usize)> {
        let mut first_line = String::new();
        reader.read_line(&mut first_line)?;

        if first_line.trim().is_empty() {
            return Ok((Box::new(reader), pipeline_builder, 0)); // Empty line will be processed normally
        }

        // Remove trailing newline for processing, but keep original for reinsertion
        let first_line_trimmed = first_line.trim_end().to_string();

        // Create a temporary parser to extract headers
        let mut temp_parser = match config.input.format {
            crate::config::InputFormat::Csv => crate::parsers::CsvParser::new_csv(),
            crate::config::InputFormat::Tsv => crate::parsers::CsvParser::new_tsv(),
            crate::config::InputFormat::Csvnh => crate::parsers::CsvParser::new_csv_no_headers(),
            crate::config::InputFormat::Tsvnh => crate::parsers::CsvParser::new_tsv_no_headers(),
            _ => return Ok((Box::new(reader), pipeline_builder, 0)), // Not a CSV format
        };

        // Initialize headers from the first line
        let was_consumed = temp_parser.initialize_headers_from_line(&first_line_trimmed)?;

        // Get the initialized headers
        let headers = temp_parser.get_headers();

        // Add headers to pipeline builder
        pipeline_builder = pipeline_builder.with_csv_headers(headers);

        // Create a new reader that includes the first line if it should be processed as data
        let final_reader: Box<dyn std::io::BufRead + Send> = if was_consumed {
            // First line was a header, don't include it in processing
            Box::new(reader)
        } else {
            // First line is data, prepend it to the reader
            let first_line_bytes = first_line.into_bytes();
            let first_line_reader = std::io::Cursor::new(first_line_bytes);
            Box::new(first_line_reader.chain(reader))
        };

        // Only count the line as preprocessed if it was consumed (not re-inserted)
        let preprocessing_count = if was_consumed { 1 } else { 0 };
        Ok((final_reader, pipeline_builder, preprocessing_count))
    }
}
