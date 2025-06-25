use anyhow::Result;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use rhai::Dynamic;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::event::Event;
use crate::formatters::{Formatter, JsonFormatter, TextFormatter};
use crate::parsers::{JsonlParser, Parser};

/// Configuration for worker threads
#[derive(Debug, Clone)]
struct WorkerConfig {
    input_format: crate::InputFormat,
    filters: Vec<String>,
    evals: Vec<String>,
    on_error: crate::ErrorStrategy,
}

/// Request parameters for parallel processing
#[derive(Debug)]
pub struct ProcessRequest {
    pub input_format: crate::InputFormat,
    pub filters: Vec<String>,
    pub evals: Vec<String>,
    pub output_format: crate::OutputFormat,
    pub on_error: crate::ErrorStrategy,
    pub keys: Vec<String>,
}

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
}

/// Result of processing a batch
#[derive(Debug)]
pub struct BatchResult {
    pub batch_id: u64,
    pub results: Vec<ProcessedEvent>,
    pub internal_tracked_updates: HashMap<String, Value>,
}

/// An event that has been processed and is ready for output
#[derive(Debug)]
pub struct ProcessedEvent {
    pub event: Event,
}

/// Thread-safe statistics tracker for merging worker states
#[derive(Debug, Default, Clone)]
pub struct GlobalTracker {
    internal_tracked: Arc<Mutex<HashMap<String, Value>>>,
}

impl GlobalTracker {
    pub fn new() -> Self {
        Self {
            internal_tracked: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn merge_worker_state(&self, worker_state: HashMap<String, Value>) -> Result<()> {
        let mut global = self.internal_tracked.lock().unwrap();
        
        for (key, value) in &worker_state {
            // Skip operation metadata keys - they're just for merge logic
            if key.starts_with("__op_") {
                global.insert(key.clone(), value.clone());
                continue;
            }
            
            if let Some(existing) = global.get(key) {
                // Check operation metadata to determine merge strategy
                let op_key = format!("__op_{}", key);
                let operation = worker_state.get(&op_key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("replace"); // default operation
                
                match operation {
                    "count" => {
                        // Sum counts
                        if let (Some(a), Some(b)) = (existing.as_i64(), value.as_i64()) {
                            global.insert(key.clone(), Value::Number(serde_json::Number::from(a + b)));
                            continue;
                        }
                    }
                    "min" => {
                        // Take minimum
                        if let (Some(a), Some(b)) = (existing.as_i64(), value.as_i64()) {
                            global.insert(key.clone(), Value::Number(serde_json::Number::from(a.min(b))));
                            continue;
                        }
                    }
                    "max" => {
                        // Take maximum
                        if let (Some(a), Some(b)) = (existing.as_i64(), value.as_i64()) {
                            global.insert(key.clone(), Value::Number(serde_json::Number::from(a.max(b))));
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

    pub fn get_final_state(&self) -> HashMap<String, Value> {
        self.internal_tracked.lock().unwrap().clone()
    }
}

/// Main parallel processor
pub struct ParallelProcessor {
    config: ParallelConfig,
    global_tracker: GlobalTracker,
}

impl ParallelProcessor {
    pub fn new(config: ParallelConfig) -> Self {
        Self {
            config,
            global_tracker: GlobalTracker::new(),
        }
    }

    /// Process input using the parallel pipeline
    /// Only parallelizes --filter and --eval stages, --begin and --end run sequentially
    pub fn process<R: std::io::BufRead + Send + 'static>(
        &self,
        reader: R,
        request: ProcessRequest,
    ) -> Result<()> {
        let worker_config = WorkerConfig {
            input_format: request.input_format.clone(),
            filters: request.filters,
            evals: request.evals,
            on_error: request.on_error.clone(),
        };
        // Create channels
        let (batch_sender, batch_receiver) = if let Some(size) = self.config.buffer_size {
            bounded(size)
        } else {
            unbounded()
        };

        let (result_sender, result_receiver) = if self.config.preserve_order {
            bounded(self.config.num_workers * 2)
        } else {
            unbounded()
        };

        // Start reader thread
        let reader_handle = {
            let batch_sender = batch_sender.clone();
            let batch_size = self.config.batch_size;
            let batch_timeout = Duration::from_millis(self.config.batch_timeout_ms);
            
            thread::spawn(move || {
                Self::reader_thread(reader, batch_sender, batch_size, batch_timeout)
            })
        };

        // Start worker threads
        let mut worker_handles = Vec::new();
        
        for worker_id in 0..self.config.num_workers {
            let batch_receiver = batch_receiver.clone();
            let result_sender = result_sender.clone();
            let worker_config = worker_config.clone();
            
            let handle = thread::spawn(move || {
                Self::worker_thread(
                    worker_id,
                    batch_receiver,
                    result_sender,
                    worker_config,
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
            let output_format = request.output_format.clone();
            let keys = request.keys;
            
            thread::spawn(move || {
                Self::result_sink_thread(
                    result_receiver,
                    output_format,
                    preserve_order,
                    keys,
                    global_tracker,
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
    pub fn get_final_tracked_state(&self) -> HashMap<String, Value> {
        self.global_tracker.get_final_state()
    }

    /// Reader thread: batches input lines with timeout - simpler approach
    fn reader_thread<R: std::io::BufRead>(
        mut reader: R,
        batch_sender: Sender<Batch>,
        batch_size: usize,
        batch_timeout: Duration,
    ) -> Result<()> {
        let mut batch_id = 0u64;
        let mut current_batch = Vec::new();
        let mut line_num = 0usize;
        let mut batch_start_line = 1usize;
        let mut last_batch_time = Instant::now();
        let mut line_buffer = String::new();

        // For truly streaming behavior, we need to:
        // 1. Process lines immediately as they arrive
        // 2. Send single-line batches if timeout occurs
        // 3. Avoid blocking indefinitely on read_line

        loop {
            // Check if we should send current batch due to timeout
            if !current_batch.is_empty() && last_batch_time.elapsed() >= batch_timeout {
                Self::send_batch(&batch_sender, &mut current_batch, batch_id, batch_start_line)?;
                batch_id += 1;
                batch_start_line = line_num + 1;
                last_batch_time = Instant::now();
            }

            line_buffer.clear();
            match reader.read_line(&mut line_buffer) {
                Ok(0) => {
                    // EOF reached
                    if !current_batch.is_empty() {
                        Self::send_batch(&batch_sender, &mut current_batch, batch_id, batch_start_line)?;
                    }
                    break;
                }
                Ok(_) => {
                    line_num += 1;
                    let line = line_buffer.trim_end().to_string();
                    
                    // Skip empty lines
                    if line.is_empty() {
                        continue;
                    }
                    
                    current_batch.push(line);
                    
                    // For true streaming: send immediately for batch_size=1 or when batch is full
                    if current_batch.len() >= batch_size {
                        Self::send_batch(&batch_sender, &mut current_batch, batch_id, batch_start_line)?;
                        batch_id += 1;
                        batch_start_line = line_num + 1;
                        last_batch_time = Instant::now();
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

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
        
        let batch = Batch {
            id: batch_id,
            lines: std::mem::take(current_batch),
            start_line_num: batch_start_line,
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
        config: WorkerConfig,
    ) -> Result<()> {
        // Create thread-local parser and engine
        let parser = Self::create_parser(&config.input_format);
        let mut engine = crate::engine::RhaiEngine::new();
        engine.compile_expressions(&config.filters, &config.evals, None, None)?;
        
        // Thread-local tracking state will be initialized automatically
        
        // Worker tracking state - syncs with thread-local storage
        let mut worker_tracked: HashMap<String, Dynamic> = HashMap::new();

        while let Ok(batch) = batch_receiver.recv() {
            // Reset thread-local tracking state for this batch
            crate::engine::RhaiEngine::clear_thread_tracking_state();
            
            let mut batch_results = Vec::new();
            
            for (line_idx, line) in batch.lines.iter().enumerate() {
                let current_line_num = batch.start_line_num + line_idx;
                
                // Parse the line
                let mut event = match parser.parse(line) {
                    Ok(event) => event,
                    Err(e) => match config.on_error {
                        crate::ErrorStrategy::Skip => continue,
                        crate::ErrorStrategy::FailFast => return Err(e),
                        crate::ErrorStrategy::EmitErrors => {
                            eprintln!("Parse error on line {}: {}", current_line_num, e);
                            continue;
                        }
                        crate::ErrorStrategy::DefaultValue => Event::default_with_line(line.clone()),
                    },
                };

                // Set metadata
                event.set_metadata(current_line_num, None);

                // Apply filters
                let should_output = match engine.execute_filters(&event, &mut worker_tracked) {
                    Ok(result) => result,
                    Err(e) => match config.on_error {
                        crate::ErrorStrategy::Skip => false,
                        crate::ErrorStrategy::FailFast => return Err(e),
                        crate::ErrorStrategy::EmitErrors => {
                            eprintln!("Filter error on line {}: {}", current_line_num, e);
                            false
                        }
                        crate::ErrorStrategy::DefaultValue => true,
                    },
                };

                if !should_output {
                    continue;
                }

                // Apply eval expressions
                if let Err(e) = engine.execute_evals(&mut event, &mut worker_tracked) {
                    match config.on_error {
                        crate::ErrorStrategy::Skip => continue,
                        crate::ErrorStrategy::FailFast => return Err(e),
                        crate::ErrorStrategy::EmitErrors => {
                            eprintln!("Eval error on line {}: {}", current_line_num, e);
                            continue;
                        }
                        crate::ErrorStrategy::DefaultValue => {}
                    }
                }

                batch_results.push(ProcessedEvent {
                    event,
                });
            }

            // Get thread-local tracking state and convert to JSON for sending
            let thread_local_state = crate::engine::RhaiEngine::get_thread_tracking_state();
            let internal_tracked_json = Self::convert_tracked_to_json(&thread_local_state);
            
            // Send batch result
            let batch_result = BatchResult {
                batch_id: batch.id,
                results: batch_results,
                internal_tracked_updates: internal_tracked_json,
            };

            if result_sender.send(batch_result).is_err() {
                // Channel closed, worker should exit
                break;
            }
        }

        Ok(())
    }

    /// Result sink thread: handles output ordering and merges global state
    fn result_sink_thread(
        result_receiver: Receiver<BatchResult>,
        output_format: crate::OutputFormat,
        preserve_order: bool,
        keys: Vec<String>,
        global_tracker: GlobalTracker,
    ) -> Result<()> {
        let formatter = Self::create_formatter(&output_format);
        if preserve_order {
            Self::ordered_result_sink(result_receiver, formatter, keys, global_tracker)
        } else {
            Self::unordered_result_sink(result_receiver, formatter, keys, global_tracker)
        }
    }

    fn ordered_result_sink(
        result_receiver: Receiver<BatchResult>,
        formatter: Box<dyn Formatter>,
        keys: Vec<String>,
        global_tracker: GlobalTracker,
    ) -> Result<()> {
        let mut pending_batches: HashMap<u64, BatchResult> = HashMap::new();
        let mut next_expected_id = 0u64;

        while let Ok(mut batch_result) = result_receiver.recv() {
            let batch_id = batch_result.batch_id;
            let internal_tracked_updates = std::mem::take(&mut batch_result.internal_tracked_updates);
            
            // Merge global state
            global_tracker.merge_worker_state(internal_tracked_updates)?;

            // Store batch for ordering
            pending_batches.insert(batch_id, batch_result);

            // Output all consecutive batches starting from next_expected_id
            while let Some(batch) = pending_batches.remove(&next_expected_id) {
                Self::output_batch_results(&batch.results, formatter.as_ref(), &keys)?;
                next_expected_id += 1;
            }
        }

        // Output any remaining batches (shouldn't happen with proper shutdown)
        for (_, batch) in pending_batches {
            Self::output_batch_results(&batch.results, formatter.as_ref(), &keys)?;
        }

        Ok(())
    }

    fn unordered_result_sink(
        result_receiver: Receiver<BatchResult>,
        formatter: Box<dyn Formatter>,
        keys: Vec<String>,
        global_tracker: GlobalTracker,
    ) -> Result<()> {
        while let Ok(batch_result) = result_receiver.recv() {
            // Merge global state
            global_tracker.merge_worker_state(batch_result.internal_tracked_updates)?;

            // Output immediately
            Self::output_batch_results(&batch_result.results, formatter.as_ref(), &keys)?;
        }

        Ok(())
    }

    fn output_batch_results(
        results: &[ProcessedEvent],
        formatter: &dyn Formatter,
        keys: &[String],
    ) -> Result<()> {
        for processed in results {
            let mut event = processed.event.clone();
            
            // Filter keys if specified
            if !keys.is_empty() {
                event.filter_keys(keys);
            }

            println!("{}", formatter.format(&event));
            io::stdout().flush().unwrap_or(());
        }
        Ok(())
    }

    fn create_parser(format: &crate::InputFormat) -> Box<dyn Parser> {
        match format {
            crate::InputFormat::Json => Box::new(JsonlParser::new()),
            crate::InputFormat::Line => todo!("Line parser not implemented yet"),
            crate::InputFormat::Csv => todo!("CSV parser not implemented yet"),
            crate::InputFormat::Apache => todo!("Apache parser not implemented yet"),
        }
    }

    fn create_formatter(format: &crate::OutputFormat) -> Box<dyn Formatter> {
        match format {
            crate::OutputFormat::Json => Box::new(JsonFormatter::new()),
            crate::OutputFormat::Text => Box::new(TextFormatter::new()),
            crate::OutputFormat::Csv => todo!("CSV formatter not implemented yet"),
        }
    }

    fn convert_tracked_to_json(tracked: &HashMap<String, Dynamic>) -> HashMap<String, Value> {
        let mut result = HashMap::new();
        for (key, dynamic_val) in tracked {
            let json_val = Self::dynamic_to_json(dynamic_val);
            result.insert(key.clone(), json_val);
        }
        result
    }

    fn dynamic_to_json(dynamic: &Dynamic) -> Value {
        if dynamic.is_unit() {
            Value::Null
        } else if dynamic.is_bool() {
            Value::Bool(dynamic.as_bool().unwrap_or(false))
        } else if dynamic.is_int() {
            Value::Number(serde_json::Number::from(dynamic.as_int().unwrap_or(0)))
        } else if dynamic.is_float() {
            serde_json::Number::from_f64(dynamic.as_float().unwrap_or(0.0))
                .map(Value::Number)
                .unwrap_or(Value::Null)
        } else if dynamic.is_string() {
            Value::String(dynamic.clone().into_string().unwrap_or_default())
        } else {
            Value::String(dynamic.to_string())
        }
    }
}