#![allow(dead_code)]
use anyhow::Result;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use rhai::Dynamic;
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::event::Event;
use crate::pipeline::PipelineBuilder;
use crate::stats::{get_thread_stats, stats_finish_processing, stats_start_timer, ProcessingStats};
use crate::unix::SHOULD_TERMINATE;

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
        // Don't merge output/filtered/errors - these are now handled by tracking system
        // Only merge timing and other safe stats
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
        tracked: &HashMap<String, Dynamic>,
    ) -> Result<()> {
        let mut stats = self.processing_stats.lock().unwrap();

        let output = tracked
            .get("__kelora_stats_output")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;
        // Note: Line-level filtering is not used - all filtering is done at event level
        let lines_errors = tracked
            .get("__kelora_stats_lines_errors")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;
        let events_created = tracked
            .get("__kelora_stats_events_created")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;
        let events_output = tracked
            .get("__kelora_stats_events_output")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0) as usize;
        let events_filtered = tracked
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
        if let Some(levels_dynamic) = tracked.get("__kelora_stats_discovered_levels") {
            if let Ok(levels_array) = levels_dynamic.clone().into_array() {
                for level in levels_array {
                    if let Ok(level_str) = level.into_string() {
                        stats.discovered_levels.insert(level_str);
                    }
                }
            }
        }

        // Extract discovered keys from tracking data
        if let Some(keys_dynamic) = tracked.get("__kelora_stats_discovered_keys") {
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
    ) -> Result<()> {
        // For file processing, try to use file-aware reader if available
        if !config.input.files.is_empty() {
            return self.process_with_file_aware_pipeline(pipeline_builder, stages, config, output);
        }

        // Fallback to original implementation for stdin
        self.process_with_generic_pipeline(reader, pipeline_builder, stages, config, output)
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

        // Start reader thread
        let reader_handle = {
            let batch_sender = batch_sender.clone();
            let batch_size = self.config.batch_size;
            let batch_timeout = Duration::from_millis(self.config.batch_timeout_ms);
            let ignore_lines = config.input.ignore_lines.clone();
            let skip_lines = config.input.skip_lines;

            let global_tracker_clone = self.global_tracker.clone();
            let input_format = config.input.format.clone();
            thread::spawn(move || {
                Self::reader_thread(
                    reader,
                    batch_sender,
                    batch_size,
                    batch_timeout,
                    global_tracker_clone,
                    ignore_lines,
                    skip_lines,
                    input_format,
                    preprocessing_line_count,
                )
            })
        };

        // Start worker threads
        let mut worker_handles = Vec::with_capacity(self.config.num_workers);

        for worker_id in 0..self.config.num_workers {
            let batch_receiver = batch_receiver.clone();
            let result_sender = result_sender.clone();
            let worker_pipeline_builder = pipeline_builder.clone();
            let worker_stages = stages.clone();

            let handle = thread::spawn(move || {
                Self::worker_thread(
                    worker_id,
                    batch_receiver,
                    result_sender,
                    worker_pipeline_builder,
                    worker_stages,
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
        reader_handle.join().unwrap()?;

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

        // Start file-aware reader thread
        let reader_handle = {
            let batch_sender = batch_sender.clone();
            let batch_size = self.config.batch_size;
            let batch_timeout = Duration::from_millis(self.config.batch_timeout_ms);
            let ignore_lines = config.input.ignore_lines.clone();
            let skip_lines = config.input.skip_lines;
            let global_tracker_clone = self.global_tracker.clone();
            let input_format = config.input.format.clone();

            thread::spawn(move || {
                Self::file_aware_reader_thread(
                    file_aware_reader,
                    batch_sender,
                    batch_size,
                    batch_timeout,
                    global_tracker_clone,
                    ignore_lines,
                    skip_lines,
                    input_format,
                )
            })
        };

        // Start worker threads
        let mut worker_handles = Vec::with_capacity(self.config.num_workers);

        for worker_id in 0..self.config.num_workers {
            let batch_receiver = batch_receiver.clone();
            let result_sender = result_sender.clone();
            let worker_pipeline_builder = file_aware_pipeline_builder.clone();
            let worker_stages = stages.clone();

            let handle = thread::spawn(move || {
                Self::worker_thread(
                    worker_id,
                    batch_receiver,
                    result_sender,
                    worker_pipeline_builder,
                    worker_stages,
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
        reader_handle.join().unwrap()?;

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

    /// File-aware reader thread: batches input lines with filename tracking
    #[allow(clippy::too_many_arguments)]
    fn file_aware_reader_thread(
        mut reader: Box<dyn crate::readers::FileAwareRead>,
        batch_sender: Sender<Batch>,
        batch_size: usize,
        batch_timeout: Duration,
        global_tracker: GlobalTracker,
        ignore_lines: Option<regex::Regex>,
        skip_lines: usize,
        input_format: crate::config::InputFormat,
    ) -> Result<()> {
        let mut batch_id = 0u64;
        let mut current_batch = Vec::with_capacity(batch_size);
        let mut current_filenames = Vec::with_capacity(batch_size);
        let mut line_num = 0usize;
        let mut batch_start_line = 1usize;
        let mut last_batch_time = Instant::now();
        let mut line_buffer = String::new();
        let mut skipped_lines = 0;
        let mut filtered_lines = 0;
        #[allow(unused_assignments)]
        let mut current_csv_parser: Option<crate::parsers::CsvParser> = None;
        let mut last_filename: Option<String> = None;
        let mut current_headers: Option<Vec<String>> = None;

        loop {
            // Check for termination signal
            if SHOULD_TERMINATE.load(Ordering::Relaxed) {
                break;
            }

            // Check if we should send current batch due to timeout
            if !current_batch.is_empty() && last_batch_time.elapsed() >= batch_timeout {
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
                last_batch_time = Instant::now();
            }

            line_buffer.clear();
            match reader.read_line(&mut line_buffer) {
                Ok(0) => {
                    // EOF reached
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
                    break;
                }
                Ok(_) => {
                    line_num += 1;
                    let line = line_buffer.trim_end().to_string();
                    let current_filename = reader.current_filename().map(|s| s.to_string());

                    // Skip the first N lines if configured
                    if skipped_lines < skip_lines {
                        skipped_lines += 1;
                        filtered_lines += 1;
                        continue;
                    }

                    // Skip empty lines for structured formats only, not for line format
                    if line.is_empty() && !matches!(input_format, crate::config::InputFormat::Line)
                    {
                        continue;
                    }

                    // Apply ignore-lines filter if configured
                    if let Some(ref ignore_regex) = ignore_lines {
                        if ignore_regex.is_match(&line) {
                            filtered_lines += 1;
                            continue;
                        }
                    }

                    // For CSV formats, detect file changes and reinitialize parser
                    if matches!(
                        input_format,
                        crate::config::InputFormat::Csv
                            | crate::config::InputFormat::Tsv
                            | crate::config::InputFormat::Csvnh
                            | crate::config::InputFormat::Tsvnh
                    ) && current_filename != last_filename
                    {
                        // File changed - send current batch before processing new file
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
                            last_batch_time = Instant::now();
                        }

                        // File changed, reinitialize CSV parser for this file
                        current_csv_parser = Self::create_csv_parser_for_file(&input_format, &line);
                        current_headers = current_csv_parser
                            .as_ref()
                            .map(|parser| parser.get_headers());
                        last_filename = current_filename.clone();

                        // Skip header lines for CSV/TSV (not for CSVNH/TSVNH)
                        if matches!(
                            input_format,
                            crate::config::InputFormat::Csv | crate::config::InputFormat::Tsv
                        ) {
                            // This line was consumed as a header, skip it
                            continue;
                        }
                    }

                    current_batch.push(line);
                    current_filenames.push(current_filename);

                    // Send batch when full
                    if current_batch.len() >= batch_size {
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
                        last_batch_time = Instant::now();
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Report final line count and filtered lines to global tracker
        global_tracker.set_total_lines_read(line_num)?;
        global_tracker.add_lines_filtered(filtered_lines)?;

        Ok(())
    }

    /// Create a CSV parser for a new file
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

        // Initialize headers from the first line
        if parser.initialize_headers_from_line(first_line).is_ok() {
            Some(parser)
        } else {
            None
        }
    }

    /// Reader thread: batches input lines with timeout - simpler approach
    #[allow(clippy::too_many_arguments)]
    fn reader_thread<R: std::io::BufRead>(
        mut reader: R,
        batch_sender: Sender<Batch>,
        batch_size: usize,
        batch_timeout: Duration,
        global_tracker: GlobalTracker,
        ignore_lines: Option<regex::Regex>,
        skip_lines: usize,
        input_format: crate::config::InputFormat,
        preprocessing_line_count: usize,
    ) -> Result<()> {
        let mut batch_id = 0u64;
        let mut current_batch = Vec::with_capacity(batch_size);
        let mut line_num = preprocessing_line_count;
        let mut batch_start_line = 1usize;
        let mut last_batch_time = Instant::now();
        let mut line_buffer = String::new();
        let mut skipped_lines = 0;
        let mut filtered_lines = 0;

        // For truly streaming behavior, we need to:
        // 1. Process lines immediately as they arrive
        // 2. Send single-line batches if timeout occurs
        // 3. Avoid blocking indefinitely on read_line

        loop {
            // Check for termination signal
            if SHOULD_TERMINATE.load(Ordering::Relaxed) {
                break;
            }

            // Check if we should send current batch due to timeout
            if !current_batch.is_empty() && last_batch_time.elapsed() >= batch_timeout {
                Self::send_batch(
                    &batch_sender,
                    &mut current_batch,
                    batch_id,
                    batch_start_line,
                )?;
                batch_id += 1;
                batch_start_line = line_num + 1;
                last_batch_time = Instant::now();
            }

            line_buffer.clear();
            match reader.read_line(&mut line_buffer) {
                Ok(0) => {
                    // EOF reached
                    if !current_batch.is_empty() {
                        Self::send_batch(
                            &batch_sender,
                            &mut current_batch,
                            batch_id,
                            batch_start_line,
                        )?;
                    }
                    break;
                }
                Ok(_) => {
                    line_num += 1;
                    let line = line_buffer.trim_end().to_string();

                    // Skip the first N lines if configured (applied before ignore-lines and parsing)
                    if skipped_lines < skip_lines {
                        skipped_lines += 1;
                        filtered_lines += 1;
                        continue;
                    }

                    // Skip empty lines for structured formats only, not for line format
                    if line.is_empty() && !matches!(input_format, crate::config::InputFormat::Line)
                    {
                        continue;
                    }

                    // Apply ignore-lines filter if configured (early filtering before parsing)
                    if let Some(ref ignore_regex) = ignore_lines {
                        if ignore_regex.is_match(&line) {
                            filtered_lines += 1;
                            continue;
                        }
                    }

                    current_batch.push(line);

                    // For true streaming: send immediately for batch_size=1 or when batch is full
                    if current_batch.len() >= batch_size {
                        Self::send_batch(
                            &batch_sender,
                            &mut current_batch,
                            batch_id,
                            batch_start_line,
                        )?;
                        batch_id += 1;
                        batch_start_line = line_num + 1;
                        last_batch_time = Instant::now();
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Report final line count and filtered lines to global tracker
        global_tracker.set_total_lines_read(line_num)?;
        global_tracker.add_lines_filtered(filtered_lines)?;

        Ok(())
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
    ) -> Result<()> {
        // Set parallel mode for print capturing
        crate::rhai_functions::strings::set_parallel_mode(true);

        stats_start_timer();

        // Create worker pipeline and context
        let (mut pipeline, mut ctx) = pipeline_builder.clone().build_worker(stages.clone())?;

        // Keep track of current CSV headers to avoid recreating parsers unnecessarily
        let mut current_csv_headers: Option<Vec<String>> = None;

        while let Ok(batch) = batch_receiver.recv() {
            // Check for termination signal
            if SHOULD_TERMINATE.load(Ordering::Relaxed) {
                break;
            }

            // If this batch has CSV headers and they're different from our current ones,
            // we need to rebuild the pipeline with the new headers
            if batch.csv_headers.is_some() && batch.csv_headers != current_csv_headers {
                current_csv_headers = batch.csv_headers.clone();

                // Rebuild the pipeline with the new headers
                let new_pipeline_builder = pipeline_builder
                    .clone()
                    .with_csv_headers(current_csv_headers.clone().unwrap());
                let (new_pipeline, new_ctx) = new_pipeline_builder.build_worker(stages.clone())?;
                pipeline = new_pipeline;
                // Note: We keep the existing ctx to preserve tracking state
                ctx.rhai = new_ctx.rhai; // Update the Rhai engine to match new parser
            }

            // Track stats before batch to calculate deltas
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

                // Update metadata
                ctx.meta.line_number = Some(current_line_num);
                ctx.meta.filename = batch.filenames.get(line_idx).cloned().flatten();

                // Clear any previous captured prints/eprints before processing this event
                crate::rhai_functions::strings::clear_captured_prints();
                crate::rhai_functions::strings::clear_captured_eprints();

                // Process line through pipeline
                match pipeline.process_line(line.clone(), &mut ctx) {
                    Ok(formatted_results) => {
                        // Count output lines
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
                        // Note: Empty results are now counted as either:
                        // 1. Parsing errors (counted by stats_add_line_error() in pipeline)
                        // 2. Filter rejections (counted by stats_add_event_filtered() in pipeline)
                        // So we don't need to count empty results as filtered here anymore
                        // Get any prints/eprints that were captured during processing this specific event
                        let captured_prints =
                            crate::rhai_functions::strings::take_captured_prints();
                        let captured_eprints =
                            crate::rhai_functions::strings::take_captured_eprints();

                        // Convert formatted strings back to events for the result sink
                        // Note: This is a temporary approach during the transition
                        for formatted_result in formatted_results {
                            // For now, we'll need to create a dummy event since the result sink expects events
                            // In a full refactor, we'd change the result sink to handle formatted strings
                            let mut dummy_event =
                                Event::default_with_line(formatted_result.clone());
                            dummy_event.set_metadata(current_line_num, None);

                            // Each formatted result gets its own copy of the captured prints/eprints
                            // since they all came from processing the same input line
                            batch_results.push(ProcessedEvent {
                                event: dummy_event,
                                captured_prints: captured_prints.clone(),
                                captured_eprints: captured_eprints.clone(),
                            });
                        }
                    }
                    Err(e) => {
                        // Error handling and stats tracking is already done in pipeline.process_line()
                        // based on the ctx.config.on_error strategy
                        match ctx.config.on_error {
                            crate::ErrorStrategy::Abort => return Err(e),
                            _ => continue, // Skip, Quarantine all continue
                        }
                    }
                }
            }

            // Calculate deltas for this batch
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

            // Include user tracking (non-stats)
            for (key, value) in &ctx.tracker {
                if !key.starts_with("__kelora_stats_") && !key.starts_with("__op___kelora_stats_") {
                    deltas.insert(key.clone(), value.clone());
                }
            }

            // Include thread-local tracking state (includes error tracking)
            let thread_tracking = crate::rhai_functions::tracking::get_thread_tracking_state();
            for (key, value) in thread_tracking {
                // Include error tracking, discovered levels/keys with their operations, and user tracking, but not other internal stats
                if (!key.starts_with("__op___kelora_stats_") || 
                    key == "__op___kelora_stats_discovered_levels" || 
                    key == "__op___kelora_stats_discovered_keys") && 
                   (!key.starts_with("__kelora_stats_") || 
                    key == "__kelora_stats_discovered_levels" || 
                    key == "__kelora_stats_discovered_keys") {
                    deltas.insert(key, value);
                }
            }

            // Send deltas only
            let batch_result = BatchResult {
                batch_id: batch.id,
                results: batch_results,
                internal_tracked_updates: deltas,
                worker_stats: get_thread_stats(),
            };

            if result_sender.send(batch_result).is_err() {
                // Channel closed, worker should exit
                break;
            }

            // Keep stats, clear user tracking for next batch
            ctx.tracker.retain(|k, _| {
                k.starts_with("__kelora_stats_") || k.starts_with("__op___kelora_stats_")
            });
        }

        stats_finish_processing();

        Ok(())
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

        if preserve_order {
            Self::pipeline_ordered_result_sink(result_receiver, global_tracker, output, take_limit)
        } else {
            Self::pipeline_unordered_result_sink(
                result_receiver,
                global_tracker,
                output,
                take_limit,
            )
        }
    }

    fn pipeline_ordered_result_sink<W: std::io::Write>(
        result_receiver: Receiver<BatchResult>,
        global_tracker: GlobalTracker,
        output: &mut W,
        take_limit: Option<usize>,
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

            // Handle special final stats batch (don't store for ordering)
            if batch_id == u64::MAX {
                // This is a final stats batch from a terminated worker
                // If we're terminating, we might want to exit soon after collecting these
                if termination_detected {
                    // Continue processing a bit more to collect other final stats
                    continue;
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
                let events_this_batch =
                    Self::pipeline_output_batch_results(output, &batch.results, remaining_limit)?;
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
            events_output +=
                Self::pipeline_output_batch_results(output, &batch.results, remaining_limit)?;

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

            // Handle special final stats batch (don't output)
            if batch_result.batch_id == u64::MAX {
                // This is a final stats batch from a terminated worker
                if termination_detected {
                    // Continue processing a bit more to collect other final stats
                    continue;
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
    ) -> Result<usize> {
        let mut events_output = 0usize;

        for processed in results {
            // Check if we've reached the limit
            if let Some(limit) = remaining_limit {
                if events_output >= limit {
                    break;
                }
            }

            // First output any captured prints for this specific event (to stdout, not file)
            for print_msg in &processed.captured_prints {
                println!("{}", print_msg);
            }

            // Output any captured eprints for this specific event (to stderr)
            for eprint_msg in &processed.captured_eprints {
                eprintln!("{}", eprint_msg);
            }

            // Then output the event itself to the designated output, skip empty strings
            if !processed.event.original_line.is_empty() {
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
