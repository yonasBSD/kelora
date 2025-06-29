use anyhow::Result;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use rhai::Dynamic;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::event::Event;
use crate::pipeline::PipelineBuilder;
use crate::unix::SafeStdout;



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
    pub internal_tracked_updates: HashMap<String, Dynamic>,
}

/// An event that has been processed and is ready for output
#[derive(Debug)]
pub struct ProcessedEvent {
    pub event: Event,
}

/// Thread-safe statistics tracker for merging worker states
#[derive(Debug, Default, Clone)]
pub struct GlobalTracker {
    internal_tracked: Arc<Mutex<HashMap<String, Dynamic>>>,
}

impl GlobalTracker {
    pub fn new() -> Self {
        Self {
            internal_tracked: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn merge_worker_state(&self, worker_state: HashMap<String, Dynamic>) -> Result<()> {
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
                    .and_then(|v| v.clone().into_string().ok())
                    .unwrap_or_else(|| "replace".to_string()); // default operation
                
                match operation.as_str() {
                    "count" => {
                        // Sum counts
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
                        if let (Ok(existing_arr), Ok(new_arr)) = (existing.clone().into_array(), value.clone().into_array()) {
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
                        if let (Some(existing_map), Some(new_map)) = (existing.clone().try_cast::<rhai::Map>(), value.clone().try_cast::<rhai::Map>()) {
                            let mut merged = existing_map;
                            for (bucket_key, bucket_value) in new_map {
                                if let Ok(bucket_count) = bucket_value.as_int() {
                                    let existing_count = merged.get(&bucket_key)
                                        .and_then(|v| v.as_int().ok())
                                        .unwrap_or(0);
                                    merged.insert(bucket_key, Dynamic::from(existing_count + bucket_count));
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
}

impl ParallelProcessor {
    pub fn new(config: ParallelConfig) -> Self {
        Self {
            config,
            global_tracker: GlobalTracker::new(),
        }
    }

    /// Process input using the parallel pipeline with new pipeline architecture
    pub fn process_with_pipeline<R: std::io::BufRead + Send + 'static>(
        &self,
        reader: R,
        pipeline_builder: PipelineBuilder,
    ) -> Result<()> {
        // Create channels
        let (batch_sender, batch_receiver) = if let Some(size) = self.config.buffer_size {
            bounded(size)
        } else {
            unbounded()
        };

        let (result_sender, result_receiver) = if self.config.preserve_order {
            bounded(self.config.num_workers * 4)  // Increased from 2x to 4x workers
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
        let mut worker_handles = Vec::with_capacity(self.config.num_workers);
        
        for worker_id in 0..self.config.num_workers {
            let batch_receiver = batch_receiver.clone();
            let result_sender = result_sender.clone();
            let worker_pipeline_builder = pipeline_builder.clone();
            
            let handle = thread::spawn(move || {
                Self::worker_thread(
                    worker_id,
                    batch_receiver,
                    result_sender,
                    worker_pipeline_builder,
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
            
            thread::spawn(move || {
                Self::pipeline_result_sink_thread(
                    result_receiver,
                    preserve_order,
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
    pub fn get_final_tracked_state(&self) -> HashMap<String, Dynamic> {
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
        let mut current_batch = Vec::with_capacity(batch_size);
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

    /// Worker thread: processes batches in parallel using new pipeline architecture
    fn worker_thread(
        _worker_id: usize,
        batch_receiver: Receiver<Batch>,
        result_sender: Sender<BatchResult>,
        pipeline_builder: PipelineBuilder,
    ) -> Result<()> {
        // Create worker pipeline and context
        let (mut pipeline, mut ctx) = pipeline_builder.build_worker()?;

        while let Ok(batch) = batch_receiver.recv() {
            // Reset tracking state for this batch
            ctx.tracker.clear();
            
            let mut batch_results = Vec::with_capacity(batch.lines.len());
            
            for (line_idx, line) in batch.lines.iter().enumerate() {
                let current_line_num = batch.start_line_num + line_idx;
                
                // Update metadata
                ctx.meta.line_number = Some(current_line_num);
                
                // Process line through pipeline
                match pipeline.process_line(line.clone(), &mut ctx) {
                    Ok(formatted_results) => {
                        // Convert formatted strings back to events for the result sink
                        // Note: This is a temporary approach during the transition
                        for formatted_result in formatted_results {
                            // For now, we'll need to create a dummy event since the result sink expects events
                            // In a full refactor, we'd change the result sink to handle formatted strings
                            let mut dummy_event = Event::default_with_line(formatted_result.clone());
                            dummy_event.set_metadata(current_line_num, None);
                            
                            batch_results.push(ProcessedEvent {
                                event: dummy_event,
                            });
                        }
                    }
                    Err(e) => {
                        // Error handling is already done in pipeline.process_line()
                        // based on the ctx.config.on_error strategy
                        match ctx.config.on_error {
                            crate::ErrorStrategy::FailFast => return Err(e),
                            _ => continue, // Skip, EmitErrors, DefaultValue all continue
                        }
                    }
                }
            }

            // Send batch result with worker's tracking state
            let batch_result = BatchResult {
                batch_id: batch.id,
                results: batch_results,
                internal_tracked_updates: ctx.tracker.clone(),
            };

            if result_sender.send(batch_result).is_err() {
                // Channel closed, worker should exit
                break;
            }
        }

        Ok(())
    }

    /// Pipeline result sink thread: handles output ordering and merges global state
    /// Results are already formatted by the pipeline, so we just need to output them
    fn pipeline_result_sink_thread(
        result_receiver: Receiver<BatchResult>,
        preserve_order: bool,
        global_tracker: GlobalTracker,
    ) -> Result<()> {
        if preserve_order {
            Self::pipeline_ordered_result_sink(result_receiver, global_tracker)
        } else {
            Self::pipeline_unordered_result_sink(result_receiver, global_tracker)
        }
    }


    fn pipeline_ordered_result_sink(
        result_receiver: Receiver<BatchResult>,
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
                Self::pipeline_output_batch_results(&batch.results)?;
                next_expected_id += 1;
            }
        }

        // Output any remaining batches (shouldn't happen with proper shutdown)
        for (_, batch) in pending_batches {
            Self::pipeline_output_batch_results(&batch.results)?;
        }

        Ok(())
    }

    fn pipeline_unordered_result_sink(
        result_receiver: Receiver<BatchResult>,
        global_tracker: GlobalTracker,
    ) -> Result<()> {
        while let Ok(batch_result) = result_receiver.recv() {
            // Merge global state
            global_tracker.merge_worker_state(batch_result.internal_tracked_updates)?;

            // Output immediately
            Self::pipeline_output_batch_results(&batch_result.results)?;
        }

        Ok(())
    }

    fn pipeline_output_batch_results(results: &[ProcessedEvent]) -> Result<()> {
        for processed in results {
            // In the pipeline version, the event.original_line contains the formatted output
            let mut stdout = SafeStdout::new();
            stdout.writeln(&processed.event.original_line).unwrap_or(());
            stdout.flush().unwrap_or(());
        }
        Ok(())
    }






}