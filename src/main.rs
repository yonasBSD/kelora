use anyhow::Result;
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use crossbeam_channel::{bounded, select, unbounded, Receiver, Sender};
use std::sync::atomic::Ordering;

use crate::rhai_functions::tracking::{self, TrackingSnapshot};

mod cli;
mod colors;
mod config;
mod config_file;
mod decompression;
mod engine;
mod event;
mod formatters;
mod parallel;
mod parsers;
mod pipeline;
mod platform;
mod readers;
mod rhai_functions;
mod stats;
mod timestamp;
mod tty;

use config::KeloraConfig;
use config_file::ConfigFile;
use platform::{
    Ctrl, ExitCode, ProcessCleanup, SafeFileOut, SafeStderr, SafeStdout, SignalHandler,
    SHOULD_TERMINATE, TERMINATED_BY_SIGNAL,
};

// Internal CLI imports
use cli::{Cli, FileOrder, InputFormat, OutputFormat};
use config::{MultilineConfig, MultilineStrategy, TimestampFilterConfig};

/// Detect format from a peekable reader
/// Returns the detected format without consuming the first line
fn detect_format_from_peekable_reader<R: std::io::BufRead>(
    reader: &mut readers::PeekableLineReader<R>,
) -> Result<config::InputFormat> {
    match reader.peek_first_line()? {
        None => {
            // Empty input, default to line format
            Ok(config::InputFormat::Line)
        }
        Some(line) => {
            // Remove newline for detection
            let trimmed_line = line.trim_end_matches(&['\r', '\n'][..]);
            let detected = parsers::detect_format(trimmed_line)?;
            Ok(detected)
        }
    }
}

/// Detect format for parallel mode processing
/// Returns the detected format
fn detect_format_for_parallel_mode(files: &[String]) -> Result<config::InputFormat> {
    use std::io;

    if files.is_empty() {
        // For stdin with potential gzip, handle decompression first
        let stdin_reader = readers::ChannelStdinReader::new()?;
        let processed_stdin = decompression::maybe_gzip(stdin_reader)?;
        let mut peekable_reader =
            readers::PeekableLineReader::new(io::BufReader::new(processed_stdin));

        match detect_format_from_peekable_reader(&mut peekable_reader)? {
            config::InputFormat::Auto => Ok(config::InputFormat::Line), // Fallback
            format => Ok(format),
        }
    } else {
        // For files, read first line from first file
        let sorted_files = pipeline::builders::sort_files(files, &config::FileOrder::Cli)?;

        if sorted_files.is_empty() {
            return Ok(config::InputFormat::Line);
        }

        let first_file = &sorted_files[0];
        let decompressed = decompression::DecompressionReader::new(first_file)?;
        let mut peekable_reader = readers::PeekableLineReader::new(decompressed);

        match detect_format_from_peekable_reader(&mut peekable_reader)? {
            config::InputFormat::Auto => Ok(config::InputFormat::Line), // Fallback
            format => Ok(format),
        }
    }
}

use parallel::{ParallelConfig, ParallelProcessor};
use pipeline::DEFAULT_MULTILINE_FLUSH_TIMEOUT_MS;
use pipeline::{
    create_input_reader, create_pipeline_builder_from_config, create_pipeline_from_config,
};
use stats::{
    get_thread_stats, stats_add_error, stats_add_line_filtered, stats_add_line_output,
    stats_add_line_read, stats_finish_processing, stats_start_timer, ProcessingStats,
};
use std::io::{self, BufRead, Write};
use std::thread;
use std::time::{Duration, Instant};

/// Result of pipeline processing
#[derive(Debug)]
struct PipelineResult {
    pub stats: Option<ProcessingStats>,
    pub tracking_data: TrackingSnapshot,
}

/// Core pipeline processing function using KeloraConfig  
fn run_pipeline_with_kelora_config<W: Write + Send + 'static>(
    config: &KeloraConfig,
    output: W,
    ctrl_rx: &Receiver<Ctrl>,
) -> Result<PipelineResult> {
    // Start statistics collection if enabled
    if config.output.stats {
        stats_start_timer();
    }

    let use_parallel = config.should_use_parallel();

    if use_parallel && matches!(config.output.format, config::OutputFormat::Levelmap) {
        return Err(anyhow::anyhow!(
            "levelmap output format is not supported with --parallel or thread overrides"
        ));
    }

    if use_parallel {
        run_pipeline_parallel(config, output, ctrl_rx)
    } else {
        let mut output = output;
        run_pipeline_sequential(config, &mut output, ctrl_rx.clone())?;
        let tracking_user = tracking::get_thread_tracking_state();
        let tracking_internal = tracking::get_thread_internal_state();
        let tracking_data = TrackingSnapshot::from_parts(tracking_user, tracking_internal);
        // Always collect stats for error reporting, even if --stats not used
        stats_finish_processing();
        let mut stats = get_thread_stats();
        stats.extract_discovered_from_tracking(&tracking_data.internal);
        let final_stats = Some(stats);

        Ok(PipelineResult {
            stats: final_stats,
            tracking_data,
        })
    }
}

/// Run pipeline in parallel mode using KeloraConfig
fn run_pipeline_parallel<W: Write + Send + 'static>(
    config: &KeloraConfig,
    output: W,
    ctrl_rx: &Receiver<Ctrl>,
) -> Result<PipelineResult> {
    // Handle auto-detection for parallel mode
    let final_config = if matches!(config.input.format, config::InputFormat::Auto) {
        // For parallel mode, we need to detect format first
        let detected_format = detect_format_for_parallel_mode(&config.input.files)?;

        // Report detected format
        if config.processing.quiet_level == 0 {
            let format_name = format!("{:?}", detected_format).to_lowercase();
            let message =
                config.format_error_message(&format!("auto-detected format: {}", format_name));
            eprintln!("{}", message);
        }

        // Create new config with detected format
        let mut new_config = config.clone();
        new_config.input.format = detected_format;
        new_config
    } else {
        config.clone()
    };

    let config = &final_config;
    let batch_size = config.effective_batch_size();

    let parallel_config = ParallelConfig {
        num_workers: config.effective_threads(),
        batch_size,
        batch_timeout_ms: config.performance.batch_timeout,
        preserve_order: !config.performance.no_preserve_order,
        buffer_size: Some(10000),
    };

    let processor =
        ParallelProcessor::new(parallel_config).with_take_limit(config.processing.take_limit);

    // Create pipeline builder and components for begin/end stages
    let pipeline_builder = create_pipeline_builder_from_config(config);
    let (_pipeline, begin_stage, end_stage, mut ctx) = pipeline_builder
        .clone()
        .build(config.processing.stages.clone())?;

    // Execute begin stage sequentially if provided
    if let Err(e) = begin_stage.execute(&mut ctx) {
        return Err(anyhow::anyhow!("Begin stage error: {}", e));
    }

    // Get reader using pipeline builder
    let reader = create_input_reader(config)?;

    // Process stages in parallel
    processor.process_with_pipeline(
        reader,
        pipeline_builder,
        config.processing.stages.clone(),
        config,
        output,
        ctrl_rx.clone(),
    )?;

    // Merge the parallel metrics state with our pipeline context
    let parallel_snapshot = processor.get_final_tracked_state();

    // Extract internal stats from tracking system before merging
    // This is needed for error reporting, not just when --stats is enabled
    processor
        .extract_final_stats_from_tracking(&parallel_snapshot)
        .unwrap_or(());

    // Filter out stats and errors from user-visible context and merge the rest
    for (key, dynamic_value) in &parallel_snapshot.user {
        if !key.starts_with("__internal_")
            && !key.starts_with("__kelora_stats_")
            && !key.starts_with("__op___kelora_stats_")
            && !key.starts_with("__kelora_error_")
            && !key.starts_with("__op___kelora_error_")
        {
            ctx.tracker.insert(key.clone(), dynamic_value.clone());
        }
    }

    // Execute end stage sequentially with merged state
    if let Err(e) = end_stage.execute(&ctx) {
        return Err(anyhow::anyhow!("End stage error: {}", e));
    }

    // Return both stats and tracking data
    // Always collect stats for error reporting, even if --stats not used
    Ok(PipelineResult {
        stats: Some(processor.get_final_stats()),
        tracking_data: parallel_snapshot,
    })
}

/// Run pipeline in sequential mode using KeloraConfig
fn run_pipeline_sequential<W: Write>(
    config: &KeloraConfig,
    output: &mut W,
    ctrl_rx: Receiver<Ctrl>,
) -> Result<()> {
    if matches!(config.input.format, config::InputFormat::Auto) {
        return run_pipeline_sequential_with_auto_detection(config, output, ctrl_rx);
    }

    let input = if config.input.files.is_empty() {
        let stdin_reader = readers::ChannelStdinReader::new()?;
        let processed_stdin = decompression::maybe_gzip(stdin_reader)?;
        SequentialInput::Stdin(Box::new(io::BufReader::new(processed_stdin)))
    } else {
        let sorted_files =
            pipeline::builders::sort_files(&config.input.files, &config.input.file_order)?;
        SequentialInput::Files(readers::MultiFileReader::new(sorted_files)?)
    };

    run_pipeline_sequential_internal(config, output, ctrl_rx, input)
}

/// Run pipeline in sequential mode with auto-detection support
fn run_pipeline_sequential_with_auto_detection<W: Write>(
    config: &KeloraConfig,
    output: &mut W,
    ctrl_rx: Receiver<Ctrl>,
) -> Result<()> {
    if config.input.files.is_empty() {
        let stdin_reader = readers::ChannelStdinReader::new()?;
        let processed_stdin = decompression::maybe_gzip(stdin_reader)?;
        let mut peekable_reader =
            readers::PeekableLineReader::new(io::BufReader::new(processed_stdin));

        let detected_format = detect_format_from_peekable_reader(&mut peekable_reader)?;

        if config.processing.quiet_level == 0 {
            let format_name = format!("{:?}", detected_format).to_lowercase();
            let message =
                config.format_error_message(&format!("auto-detected format: {}", format_name));
            eprintln!("{}", message);
        }

        let mut final_config = config.clone();
        final_config.input.format = detected_format;

        let input = SequentialInput::Stdin(Box::new(peekable_reader));
        run_pipeline_sequential_internal(&final_config, output, ctrl_rx, input)
    } else {
        let sorted_files =
            pipeline::builders::sort_files(&config.input.files, &config.input.file_order)?;

        if sorted_files.is_empty() {
            return Ok(());
        }

        let first_file = &sorted_files[0];
        let detected_format = {
            let decompressed = decompression::DecompressionReader::new(first_file)?;
            let mut peekable_reader = readers::PeekableLineReader::new(decompressed);
            detect_format_from_peekable_reader(&mut peekable_reader)?
        };

        if config.processing.quiet_level == 0 {
            let format_name = format!("{:?}", detected_format).to_lowercase();
            let message =
                config.format_error_message(&format!("auto-detected format: {}", format_name));
            eprintln!("{}", message);
        }

        let mut final_config = config.clone();
        final_config.input.format = detected_format;
        let input = SequentialInput::Files(readers::MultiFileReader::new(sorted_files)?);
        run_pipeline_sequential_internal(&final_config, output, ctrl_rx, input)
    }
}

const LINE_CHANNEL_BOUND: usize = 1024;

enum SequentialInput {
    Stdin(Box<dyn BufRead + Send>),
    Files(readers::MultiFileReader),
}

enum ReaderMessage {
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

fn spawn_stdin_reader(
    mut reader: Box<dyn BufRead + Send>,
    sender: Sender<ReaderMessage>,
    ctrl_rx: Receiver<Ctrl>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        let mut buffer = String::new();
        loop {
            if let Ok(Ctrl::Shutdown { immediate }) = ctrl_rx.try_recv() {
                let _ = sender.send(ReaderMessage::Eof);
                if immediate {
                    return Ok(());
                }
                break;
            }

            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => {
                    let _ = sender.send(ReaderMessage::Eof);
                    break;
                }
                Ok(_) => {
                    let line = buffer.trim_end_matches(&['\n', '\r'][..]).to_string();
                    if sender
                        .send(ReaderMessage::Line {
                            line,
                            filename: None,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    if sender
                        .send(ReaderMessage::Error {
                            error: e,
                            filename: None,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
        Ok(())
    })
}

fn spawn_file_reader(
    mut reader: readers::MultiFileReader,
    sender: Sender<ReaderMessage>,
    ctrl_rx: Receiver<Ctrl>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        let mut buffer = String::new();
        loop {
            if let Ok(Ctrl::Shutdown { immediate }) = ctrl_rx.try_recv() {
                let _ = sender.send(ReaderMessage::Eof);
                if immediate {
                    return Ok(());
                }
                break;
            }

            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => {
                    let _ = sender.send(ReaderMessage::Eof);
                    break;
                }
                Ok(_) => {
                    let filename = reader.current_filename().map(|s| s.to_string());
                    let line = buffer.clone();
                    if sender.send(ReaderMessage::Line { line, filename }).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let filename = reader.current_filename().map(|s| s.to_string());
                    if sender
                        .send(ReaderMessage::Error { error: e, filename })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
        Ok(())
    })
}

fn run_pipeline_sequential_internal<W: Write>(
    config: &KeloraConfig,
    output: &mut W,
    ctrl_rx: Receiver<Ctrl>,
    input: SequentialInput,
) -> Result<()> {
    let (mut pipeline, begin_stage, end_stage, mut ctx) = create_pipeline_from_config(config)?;

    if let Err(e) = begin_stage.execute(&mut ctx) {
        return Err(anyhow::anyhow!("Begin stage error: {}", e));
    }

    let (line_tx, line_rx) = bounded::<ReaderMessage>(LINE_CHANNEL_BOUND);
    let reader_ctrl = ctrl_rx.clone();
    let reader_handle = match input {
        SequentialInput::Stdin(reader) => spawn_stdin_reader(reader, line_tx, reader_ctrl),
        SequentialInput::Files(reader) => spawn_file_reader(reader, line_tx, reader_ctrl),
    };

    let multiline_timeout = config
        .input
        .multiline
        .as_ref()
        .map(|_| Duration::from_millis(DEFAULT_MULTILINE_FLUSH_TIMEOUT_MS));

    let mut current_csv_headers: Option<Vec<String>> = None;
    let mut last_filename: Option<String> = None;
    let mut line_num = 0usize;
    let mut skipped_lines = 0usize;
    let mut pending_deadline: Option<Instant> = None;
    let mut shutdown_requested = false;
    let mut immediate_shutdown = false;
    let gap_marker_use_colors = crate::tty::should_use_colors_with_mode(&config.output.color);
    let mut gap_tracker = config
        .output
        .mark_gaps
        .map(|threshold| crate::formatters::GapTracker::new(threshold, gap_marker_use_colors));

    let ctrl_rx = ctrl_rx;
    let line_rx = line_rx;

    loop {
        if immediate_shutdown || shutdown_requested {
            break;
        }

        let deadline_duration = pending_deadline.map(|deadline| {
            let now = Instant::now();
            if deadline <= now {
                Duration::from_millis(0)
            } else {
                deadline.saturating_duration_since(now)
            }
        });

        if let Some(duration) = deadline_duration {
            if duration.is_zero() {
                let results = pipeline.flush(&mut ctx)?;
                for formatted in results {
                    write_formatted_output(formatted, output, &mut gap_tracker)?;
                }
                pending_deadline = None;
                continue;
            }

            let timeout = crossbeam_channel::after(duration);
            select! {
                recv(ctrl_rx) -> msg => {
                    match msg {
                        Ok(Ctrl::Shutdown { immediate }) => {
                            if immediate {
                                immediate_shutdown = true;
                            } else {
                                shutdown_requested = true;
                            }
                        }
                        Err(_) => {
                            shutdown_requested = true;
                        }
                    }
                }
                recv(line_rx) -> msg => {
                    match msg {
                        Ok(message) => {
                            if handle_reader_message(
                                message,
                                ReaderContext {
                                    pipeline: &mut pipeline,
                                    ctx: &mut ctx,
                                    config,
                                    output,
                                    line_num: &mut line_num,
                                    skipped_lines: &mut skipped_lines,
                                    current_csv_headers: &mut current_csv_headers,
                                    last_filename: &mut last_filename,
                                    gap_tracker: &mut gap_tracker,
                                },
                            )? {
                                shutdown_requested = true;
                            }
                            pending_deadline = multiline_timeout
                                .and_then(|timeout| pipeline
                                    .has_pending_chunk()
                                    .then(|| Instant::now() + timeout));
                        }
                        Err(_) => {
                            shutdown_requested = true;
                        }
                    }
                }
                recv(timeout) -> _ => {
                    let results = pipeline.flush(&mut ctx)?;
                    for formatted in results {
                        write_formatted_output(formatted, output, &mut gap_tracker)?;
                    }
                    pending_deadline = None;
                }
            }
        } else {
            select! {
                recv(ctrl_rx) -> msg => {
                    match msg {
                        Ok(Ctrl::Shutdown { immediate }) => {
                            if immediate {
                                immediate_shutdown = true;
                            } else {
                                shutdown_requested = true;
                            }
                        }
                        Err(_) => {
                            shutdown_requested = true;
                        }
                    }
                }
                recv(line_rx) -> msg => {
                    match msg {
                        Ok(message) => {
                            if handle_reader_message(
                                message,
                                ReaderContext {
                                    pipeline: &mut pipeline,
                                    ctx: &mut ctx,
                                    config,
                                    output,
                                    line_num: &mut line_num,
                                    skipped_lines: &mut skipped_lines,
                                    current_csv_headers: &mut current_csv_headers,
                                    last_filename: &mut last_filename,
                                    gap_tracker: &mut gap_tracker,
                                },
                            )? {
                                shutdown_requested = true;
                            }
                            pending_deadline = multiline_timeout
                                .and_then(|timeout| pipeline
                                    .has_pending_chunk()
                                    .then(|| Instant::now() + timeout));
                        }
                        Err(_) => {
                            shutdown_requested = true;
                        }
                    }
                }
            }
        }

        if rhai_functions::process::is_exit_requested() {
            let exit_code = rhai_functions::process::get_exit_code();
            std::process::exit(exit_code);
        }
    }

    drop(line_rx);

    match reader_handle.join() {
        Ok(result) => result?,
        Err(_) => return Err(anyhow::anyhow!("Reader thread panicked")),
    }

    if immediate_shutdown {
        return Ok(());
    }

    let results = pipeline.flush(&mut ctx)?;
    for formatted in results {
        write_formatted_output(formatted, output, &mut gap_tracker)?;
    }

    if let Some(result) = pipeline.finish_formatter() {
        write_formatted_output(result, output, &mut gap_tracker)?;
    }

    if let Err(e) = end_stage.execute(&ctx) {
        return Err(anyhow::anyhow!("End stage error: {}", e));
    }

    rhai_functions::tracking::merge_thread_tracking_to_context(&mut ctx);

    Ok(())
}

struct ReaderContext<'a, W: Write> {
    pipeline: &'a mut pipeline::Pipeline,
    ctx: &'a mut pipeline::PipelineContext,
    config: &'a KeloraConfig,
    output: &'a mut W,
    line_num: &'a mut usize,
    skipped_lines: &'a mut usize,
    current_csv_headers: &'a mut Option<Vec<String>>,
    last_filename: &'a mut Option<String>,
    gap_tracker: &'a mut Option<crate::formatters::GapTracker>,
}

fn handle_reader_message<W: Write>(
    message: ReaderMessage,
    ctx: ReaderContext<'_, W>,
) -> Result<bool> {
    let ReaderContext {
        pipeline,
        ctx: pipeline_ctx,
        config,
        output,
        line_num,
        skipped_lines,
        current_csv_headers,
        last_filename,
        gap_tracker,
    } = ctx;
    match message {
        ReaderMessage::Line { line, filename } => {
            match process_line_sequential(
                Ok(line),
                line_num,
                skipped_lines,
                pipeline,
                pipeline_ctx,
                config,
                output,
                filename,
                current_csv_headers,
                last_filename,
                gap_tracker,
            )? {
                ProcessingResult::Continue => Ok(false),
                ProcessingResult::TakeLimitExhausted => Ok(true),
            }
        }
        ReaderMessage::Error { error, filename } => {
            match process_line_sequential(
                Err(error),
                line_num,
                skipped_lines,
                pipeline,
                pipeline_ctx,
                config,
                output,
                filename,
                current_csv_headers,
                last_filename,
                gap_tracker,
            )? {
                ProcessingResult::Continue => Ok(false),
                ProcessingResult::TakeLimitExhausted => Ok(true),
            }
        }
        ReaderMessage::Eof => Ok(true),
    }
}

/// Processing result for sequential pipeline
enum ProcessingResult {
    Continue,
    TakeLimitExhausted,
}

/// Process a single line in sequential mode with filename tracking and CSV schema detection
#[allow(clippy::too_many_arguments)]
fn process_line_sequential<W: Write>(
    line_result: io::Result<String>,
    line_num: &mut usize,
    skipped_lines: &mut usize,
    pipeline: &mut pipeline::Pipeline,
    ctx: &mut pipeline::PipelineContext,
    config: &KeloraConfig,
    output: &mut W,
    current_filename: Option<String>,
    current_csv_headers: &mut Option<Vec<String>>,
    last_filename: &mut Option<String>,
    gap_tracker: &mut Option<crate::formatters::GapTracker>,
) -> Result<ProcessingResult> {
    let line = line_result?;
    *line_num += 1;

    // Count line read for stats
    if config.output.stats {
        stats_add_line_read();
    }

    // Skip the first N lines if configured (applied before ignore-lines and parsing)
    if *skipped_lines < config.input.skip_lines {
        *skipped_lines += 1;
        // Count skipped line for stats
        if config.output.stats {
            stats_add_line_filtered();
        }
        return Ok(ProcessingResult::Continue);
    }

    // Apply keep-lines filter if configured (early filtering before parsing)
    if let Some(ref keep_regex) = config.input.keep_lines {
        if !keep_regex.is_match(&line) {
            // Count filtered line for stats
            if config.output.stats {
                stats_add_line_filtered();
            }
            return Ok(ProcessingResult::Continue);
        }
    }

    // Apply ignore-lines filter if configured (early filtering before parsing)
    if let Some(ref ignore_regex) = config.input.ignore_lines {
        if ignore_regex.is_match(&line) {
            // Count filtered line for stats
            if config.output.stats {
                stats_add_line_filtered();
            }
            return Ok(ProcessingResult::Continue);
        }
    }

    if line.trim().is_empty() {
        // Only skip empty lines for structured formats, not for line format
        if !matches!(config.input.format, config::InputFormat::Line) {
            return Ok(ProcessingResult::Continue);
        }
        // For line format, continue processing the empty line
    }

    // For CSV formats, detect file changes and reinitialize parser, or handle first line for stdin
    if matches!(
        config.input.format,
        config::InputFormat::Csv
            | config::InputFormat::Tsv
            | config::InputFormat::Csvnh
            | config::InputFormat::Tsvnh
    ) && (current_filename != *last_filename
        || (current_filename.is_none() && current_csv_headers.is_none()))
    {
        // File changed, reinitialize CSV parser for this file
        let mut temp_parser = match config.input.format {
            config::InputFormat::Csv => parsers::CsvParser::new_csv(),
            config::InputFormat::Tsv => parsers::CsvParser::new_tsv(),
            config::InputFormat::Csvnh => parsers::CsvParser::new_csv_no_headers(),
            config::InputFormat::Tsvnh => parsers::CsvParser::new_tsv_no_headers(),
            _ => unreachable!(),
        };

        // Initialize headers from the first line
        let was_consumed = temp_parser.initialize_headers_from_line(&line)?;

        // Get the initialized headers
        let headers = temp_parser.get_headers();
        *current_csv_headers = Some(headers.clone());
        *last_filename = current_filename.clone();

        // Rebuild the pipeline with new headers
        let mut pipeline_builder = create_pipeline_builder_from_config(config);
        pipeline_builder = pipeline_builder.with_csv_headers(headers);

        // Note: We rebuild the entire pipeline including begin/end stages, but only use
        // the pipeline and context. The begin stage was already executed at session start
        // and the end stage will be executed when the original session completes.
        let (new_pipeline, _unused_begin_stage, _unused_end_stage, new_ctx) =
            pipeline_builder.build(config.processing.stages.clone())?;

        *pipeline = new_pipeline;
        // Keep the existing context's tracking state but update the Rhai engine
        ctx.rhai = new_ctx.rhai;

        // If the first line was consumed as a header, don't process it as data
        if was_consumed {
            return Ok(ProcessingResult::Continue);
        }
    }

    // Update metadata with filename tracking
    ctx.meta.line_num = Some(*line_num);
    ctx.meta.filename = current_filename;

    // Process line through pipeline
    match pipeline.process_line(line, ctx) {
        Ok(results) => {
            // Count output lines for stats
            if config.output.stats && !results.is_empty() {
                stats_add_line_output();
            }
            // Note: Empty results are now counted as either:
            // 1. Parsing errors (counted by stats_add_line_error() in pipeline)
            // 2. Filter rejections (counted by stats_add_event_filtered() in pipeline)
            // So we don't need to count empty results as filtered here anymore

            // Output all results (usually just one), skip empty strings
            for formatted in results {
                write_formatted_output(formatted, output, gap_tracker)?;
            }

            // Check if take limit is exhausted after processing
            if pipeline.is_take_limit_exhausted() {
                return Ok(ProcessingResult::TakeLimitExhausted);
            }
        }
        Err(e) => {
            // Count errors for stats
            if config.output.stats {
                stats_add_error();
            }

            // Handle error based on new resiliency model
            if config.processing.strict {
                return Err(e);
            }
            // Default resilient mode: continue processing
        }
    }

    Ok(ProcessingResult::Continue)
}

fn write_formatted_output<W: Write>(
    formatted: pipeline::FormattedOutput,
    output: &mut W,
    gap_tracker: &mut Option<crate::formatters::GapTracker>,
) -> io::Result<()> {
    let marker = match gap_tracker.as_mut() {
        Some(tracker) => tracker.check(formatted.timestamp),
        None => None,
    };

    if let Some(marker_line) = marker {
        writeln!(output, "{}", marker_line)?;
    }

    if !formatted.line.is_empty() {
        writeln!(output, "{}", formatted.line)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    // Broadcast channel for shutdown requests from signal handler or other sources
    let (ctrl_tx, ctrl_rx) = unbounded::<Ctrl>();

    // Initialize signal handling early
    let _signal_handler = match SignalHandler::new(ctrl_tx.clone()) {
        Ok(handler) => handler,
        Err(e) => {
            eprintln!("Failed to initialize signal handling: {}", e);
            ExitCode::GeneralError.exit();
        }
    };

    // Initialize process cleanup
    let _cleanup = ProcessCleanup::new();

    // Initialize safe I/O wrappers
    let mut stderr = SafeStderr::new();

    // Process command line arguments with config file support
    let (matches, cli) = process_args_with_config(&mut stderr);

    // Validate CLI argument combinations
    if let Err(e) = validate_cli_args(&cli) {
        stderr
            .writeln(&format!("kelora: Error: {}", e))
            .unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    // Extract ordered script stages
    let ordered_stages = match cli.get_ordered_script_stages(&matches) {
        Ok(stages) => stages,
        Err(e) => {
            stderr
                .writeln(&format!("kelora: Error: {}", e))
                .unwrap_or(());
            ExitCode::InvalidUsage.exit();
        }
    };

    // Create configuration from CLI and set stages (using lib config directly)
    let mut config = KeloraConfig::from_cli(&cli)?;
    // Set the ordered stages directly
    config.processing.stages = ordered_stages;

    // Parse timestamp filter arguments if provided
    if cli.since.is_some() || cli.until.is_some() {
        // Use the same timezone logic as the main configuration
        let cli_timezone = config.input.default_timezone.as_deref();

        let since = if let Some(ref since_str) = cli.since {
            match crate::timestamp::parse_timestamp_arg_with_timezone(since_str, cli_timezone) {
                Ok(dt) => Some(dt),
                Err(e) => {
                    stderr
                        .writeln(&config.format_error_message(&format!(
                            "Invalid --since timestamp '{}': {}",
                            since_str, e
                        )))
                        .unwrap_or(());
                    ExitCode::InvalidUsage.exit();
                }
            }
        } else {
            None
        };

        let until = if let Some(ref until_str) = cli.until {
            match crate::timestamp::parse_timestamp_arg_with_timezone(until_str, cli_timezone) {
                Ok(dt) => Some(dt),
                Err(e) => {
                    stderr
                        .writeln(&config.format_error_message(&format!(
                            "Invalid --until timestamp '{}': {}",
                            until_str, e
                        )))
                        .unwrap_or(());
                    ExitCode::InvalidUsage.exit();
                }
            }
        } else {
            None
        };

        config.processing.timestamp_filter = Some(TimestampFilterConfig { since, until });
    }

    // Compile ignore-lines regex if provided
    if let Some(ignore_pattern) = &cli.ignore_lines {
        match regex::Regex::new(ignore_pattern) {
            Ok(regex) => {
                config.input.ignore_lines = Some(regex);
            }
            Err(e) => {
                stderr
                    .writeln(&config.format_error_message(&format!(
                        "Invalid ignore-lines regex pattern '{}': {}",
                        ignore_pattern, e
                    )))
                    .unwrap_or(());
                ExitCode::InvalidUsage.exit();
            }
        }
    }

    // Compile keep-lines regex if provided
    if let Some(keep_pattern) = &cli.keep_lines {
        match regex::Regex::new(keep_pattern) {
            Ok(regex) => {
                config.input.keep_lines = Some(regex);
            }
            Err(e) => {
                stderr
                    .writeln(&config.format_error_message(&format!(
                        "Invalid keep-lines regex pattern '{}': {}",
                        keep_pattern, e
                    )))
                    .unwrap_or(());
                ExitCode::InvalidUsage.exit();
            }
        }
    }

    // Parse multiline configuration if provided
    if let Some(multiline_str) = &cli.multiline {
        match MultilineConfig::parse(multiline_str) {
            Ok(multiline_config) => {
                config.input.multiline = Some(multiline_config);
            }
            Err(e) => {
                stderr
                    .writeln(&config.format_error_message(&format!(
                        "Invalid multiline configuration '{}': {}",
                        multiline_str, e
                    )))
                    .unwrap_or(());
                ExitCode::InvalidUsage.exit();
            }
        }
    }

    // Share --ts-format hint with multiline timestamp strategies so regex fallback stays optional
    if let (Some(multiline_config), Some(ts_format)) =
        (config.input.multiline.as_mut(), &config.input.ts_format)
    {
        if let MultilineStrategy::Timestamp { chrono_format, .. } = &mut multiline_config.strategy {
            *chrono_format = Some(ts_format.clone());
        }
    }

    // Validate arguments early
    if let Err(e) = validate_cli_args(&cli) {
        stderr
            .writeln(&config.format_error_message(&format!("Error: {}", e)))
            .unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    if let Some(ref gap_str) = cli.mark_gaps {
        match crate::rhai_functions::datetime::to_duration(gap_str) {
            Ok(duration) => {
                if duration.inner.is_zero() {
                    stderr
                        .writeln(&config.format_error_message(
                            "--mark-gaps requires a duration greater than zero",
                        ))
                        .unwrap_or(());
                    ExitCode::InvalidUsage.exit();
                }
                config.output.mark_gaps = Some(duration.inner);
            }
            Err(e) => {
                stderr
                    .writeln(&config.format_error_message(&format!(
                        "Invalid --mark-gaps duration '{}': {}",
                        gap_str, e
                    )))
                    .unwrap_or(());
                ExitCode::InvalidUsage.exit();
            }
        }
    }

    // Handle output destination and run pipeline
    let result = if let Some(ref output_file_path) = cli.output_file {
        // Use file output
        let file_output = match SafeFileOut::new(output_file_path) {
            Ok(file) => file,
            Err(e) => {
                stderr
                    .writeln(&config.format_error_message(&e.to_string()))
                    .unwrap_or(());
                ExitCode::GeneralError.exit();
            }
        };
        run_pipeline_with_kelora_config(&config, file_output, &ctrl_rx)
    } else {
        // Use stdout output
        let stdout_output = SafeStdout::new();
        run_pipeline_with_kelora_config(&config, stdout_output, &ctrl_rx)
    };

    let (final_stats, tracking_data) = match result {
        Ok(pipeline_result) => {
            // Print metrics if enabled (only if not terminated)
            if config.output.metrics && !SHOULD_TERMINATE.load(Ordering::Relaxed) {
                let metrics_output = crate::rhai_functions::tracking::format_metrics_output(
                    &pipeline_result.tracking_data.user,
                );
                if !metrics_output.is_empty() && metrics_output != "No metrics tracked" {
                    stderr
                        .writeln(&config.format_metrics_message(&metrics_output))
                        .unwrap_or(());
                }
            }

            // Write metrics to file if configured
            if let Some(ref metrics_file) = config.output.metrics_file {
                if let Ok(json_output) = crate::rhai_functions::tracking::format_metrics_json(
                    &pipeline_result.tracking_data.user,
                ) {
                    if let Err(e) = std::fs::write(metrics_file, json_output) {
                        stderr
                            .writeln(&config.format_error_message(&format!(
                                "Failed to write metrics file: {}",
                                e
                            )))
                            .unwrap_or(());
                    }
                }
            }

            // Print output based on configuration (only if not terminated)
            if !SHOULD_TERMINATE.load(Ordering::Relaxed) {
                if let Some(ref s) = pipeline_result.stats {
                    if config.output.stats && config.processing.quiet_level == 0 {
                        // Full stats when --stats flag is used (unless quiet level > 0)
                        stderr
                            .writeln(&config.format_stats_message(
                                &s.format_stats(config.input.multiline.is_some()),
                            ))
                            .unwrap_or(());
                    } else if config.processing.quiet_level == 0 {
                        // Error summary by default when errors occur (unless quiet level > 0)
                        if let Some(error_summary) =
                            crate::rhai_functions::tracking::extract_error_summary_from_tracking(
                                &pipeline_result.tracking_data,
                                config.processing.verbose,
                                Some(&config),
                            )
                        {
                            stderr
                                .writeln(&config.format_error_message(&error_summary))
                                .unwrap_or(());
                        }
                    }
                }
            }
            (pipeline_result.stats, Some(pipeline_result.tracking_data))
        }
        Err(e) => {
            stderr
                .writeln(&config.format_error_message(&format!("Pipeline error: {}", e)))
                .unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };

    // Check if we were terminated by a signal and print output
    if TERMINATED_BY_SIGNAL.load(Ordering::Relaxed) {
        if let Some(stats) = final_stats {
            if config.output.stats && config.processing.quiet_level == 0 {
                // Full stats when --stats flag is used (unless quiet level > 0)
                stderr
                    .writeln(&config.format_stats_message(
                        &stats.format_stats(config.input.multiline.is_some()),
                    ))
                    .unwrap_or(());
            } else if stats.has_errors() && config.processing.quiet_level == 0 {
                // Error summary by default when errors occur (unless quiet level > 0)
                stderr
                    .writeln(&config.format_error_message(&stats.format_error_summary()))
                    .unwrap_or(());
            }
        } else if config.output.stats && config.processing.quiet_level == 0 {
            stderr
                .writeln(&config.format_stats_message("Processing interrupted"))
                .unwrap_or(());
        }
        ExitCode::SignalInt.exit();
    }

    // Determine exit code based on whether any errors occurred during processing
    let had_errors = if let Some(ref tracking) = tracking_data {
        // Check tracking data for errors from processing
        crate::rhai_functions::tracking::has_errors_in_tracking(tracking)
    } else if let Some(ref stats) = final_stats {
        // Check stats for errors from parallel processing or termination case
        stats.has_errors()
    } else {
        // No processing results available, assume no errors
        false
    };

    if had_errors {
        ExitCode::GeneralError.exit();
    } else {
        ExitCode::Success.exit();
    }
}

/// Validate CLI arguments for early error detection
fn validate_cli_args(cli: &Cli) -> Result<()> {
    // Check if input files exist (if specified), skip "-" which represents stdin
    let mut stdin_count = 0;
    for file_path in &cli.files {
        if file_path == "-" {
            stdin_count += 1;
            if stdin_count > 1 {
                return Err(anyhow::anyhow!("stdin (\"-\") can only be specified once"));
            }
        } else if !std::path::Path::new(file_path).exists() {
            return Err(anyhow::anyhow!("File not found: {}", file_path));
        }
    }

    // Check if exec files exist (if specified)
    for exec_file in &cli.exec_files {
        if !std::path::Path::new(exec_file).exists() {
            return Err(anyhow::anyhow!("Exec file not found: {}", exec_file));
        }
    }

    // Validate batch size
    if let Some(batch_size) = cli.batch_size {
        if batch_size == 0 {
            return Err(anyhow::anyhow!("Batch size must be greater than 0"));
        }
    }

    // Validate thread count
    if cli.threads > 1000 {
        return Err(anyhow::anyhow!("Thread count too high (max 1000)"));
    }

    // Check for --core with CSV/TSV formats (not allowed with these formats)
    if cli.core {
        match cli.output_format {
            OutputFormat::Csv => {
                return Err(anyhow::anyhow!(
                    "csv output format does not support --core flag. Use --keys to specify field names"
                ));
            }
            OutputFormat::Tsv => {
                return Err(anyhow::anyhow!(
                    "tsv output format does not support --core flag. Use --keys to specify field names"
                ));
            }
            OutputFormat::Csvnh => {
                return Err(anyhow::anyhow!(
                    "csvnh output format does not support --core flag. Use --keys to specify field names"
                ));
            }
            OutputFormat::Tsvnh => {
                return Err(anyhow::anyhow!(
                    "tsvnh output format does not support --core flag. Use --keys to specify field names"
                ));
            }
            _ => {
                // Other formats are fine with --core
            }
        }
    }

    Ok(())
}

/// Validate configuration for consistency
#[allow(dead_code)]
fn validate_config(config: &KeloraConfig) -> Result<()> {
    // Check if files exist (if specified)
    for file_path in &config.input.files {
        if !std::path::Path::new(file_path).exists() {
            return Err(anyhow::anyhow!("File not found: {}", file_path));
        }
    }

    // Validate batch size
    if let Some(batch_size) = config.performance.batch_size {
        if batch_size == 0 {
            return Err(anyhow::anyhow!("Batch size must be greater than 0"));
        }
    }

    // Validate thread count
    if config.performance.threads > 1000 {
        return Err(anyhow::anyhow!("Thread count too high (max 1000)"));
    }

    Ok(())
}

/// Extract --config-file argument from raw args
fn extract_config_file_arg(args: &[String]) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == "--config-file" && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}

/// Process command line arguments with config file support
fn process_args_with_config(stderr: &mut SafeStderr) -> (ArgMatches, Cli) {
    // Get raw command line arguments
    let raw_args: Vec<String> = std::env::args().collect();

    // Check for --show-config first, before any other processing
    if raw_args.iter().any(|arg| arg == "--show-config") {
        ConfigFile::show_config();
        std::process::exit(0);
    }

    // Check for --help-time
    if raw_args.iter().any(|arg| arg == "--help-time") {
        print_time_format_help();
        std::process::exit(0);
    }

    // Check for --help-functions
    if raw_args.iter().any(|arg| arg == "--help-functions") {
        print_functions_help();
        std::process::exit(0);
    }

    // Check for --help-rhai
    if raw_args.iter().any(|arg| arg == "--help-rhai") {
        print_rhai_help();
        std::process::exit(0);
    }

    // Check for --help-multiline
    if raw_args.iter().any(|arg| arg == "--help-multiline") {
        print_multiline_help();
        std::process::exit(0);
    }

    // Check for --ignore-config
    let ignore_config = raw_args.iter().any(|arg| arg == "--ignore-config");

    // Extract --config-file argument if present
    let config_file_path = extract_config_file_arg(&raw_args);

    let processed_args = if ignore_config {
        // Skip config file processing
        raw_args
    } else {
        // Load config file and process aliases
        match ConfigFile::load_with_custom_path(config_file_path.as_deref()) {
            Ok(config_file) => match config_file.process_args(raw_args) {
                Ok(processed) => processed,
                Err(e) => {
                    stderr
                        .writeln(&format!("kelora: Config error: {}", e))
                        .unwrap_or(());
                    std::process::exit(1);
                }
            },
            Err(e) => {
                stderr
                    .writeln(&format!("kelora: Config file error: {}", e))
                    .unwrap_or(());
                std::process::exit(1);
            }
        }
    };

    // Parse with potentially modified arguments
    let matches = Cli::command().get_matches_from(processed_args);
    let mut cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| {
        stderr
            .writeln(&format!("kelora: Error: {}", e))
            .unwrap_or(());
        std::process::exit(1);
    });

    // Resolve inverted boolean flags
    cli.resolve_boolean_flags();

    // Config file defaults and aliases are already applied in process_args above

    // Show usage if on TTY and no input files provided (but not if "-" is explicitly specified)
    if crate::tty::is_stdin_tty() && cli.files.is_empty() {
        // Print brief usage with description and help hint
        println!("{}", Cli::command().render_usage());
        println!("A command-line log analysis tool with embedded Rhai scripting");
        println!("Try 'kelora --help' for more information.");
        std::process::exit(0);
    }

    (matches, cli)
}

/// Print time format help message adapted for Rust/chrono
fn print_time_format_help() {
    let help_text = r#"
Time Format Reference for --ts-format:

Basic Date/Time Components:
%Y - Year with century (e.g., 2024)
%y - Year without century (00-99)
%m - Month as zero-padded decimal (01-12)
%b - Month as abbreviated name (Jan, Feb, ..., Dec)
%B - Month as full name (January, February, ..., December)
%d - Day of month as zero-padded decimal (01-31)
%j - Day of year as zero-padded decimal (001-366)
%H - Hour (24-hour) as zero-padded decimal (00-23)
%I - Hour (12-hour) as zero-padded decimal (01-12)
%p - AM/PM indicator
%M - Minute as zero-padded decimal (00-59)
%S - Second as zero-padded decimal (00-59)

Subsecond Precision:
%f - Microseconds (000000-999999)
%3f - Milliseconds (000-999)
%6f - Microseconds (000000-999999)
%9f - Nanoseconds (000000000-999999999)
%. - Subseconds with automatic precision

Time Zone:
%z - UTC offset (+HHMM or -HHMM)
%Z - Time zone name (if available)
%:z - UTC offset with colon (+HH:MM or -HH:MM)

Weekday:
%w - Weekday as decimal (0=Sunday, 6=Saturday)
%a - Weekday as abbreviated name (Sun, Mon, ..., Sat)
%A - Weekday as full name (Sunday, Monday, ..., Saturday)

Week Numbers:
%W - Week number (Monday as first day of week)
%U - Week number (Sunday as first day of week)

Common Examples:
%Y-%m-%d %H:%M:%S           # 2024-01-15 14:30:45
%Y-%m-%dT%H:%M:%S%z         # 2024-01-15T14:30:45+0000
%Y-%m-%d %H:%M:%S%.f        # 2024-01-15 14:30:45.123456
%b %d %H:%M:%S              # Jan 15 14:30:45 (syslog format)
%d/%b/%Y:%H:%M:%S %z        # 15/Jan/2024:14:30:45 +0000 (Apache format)
%Y-%m-%d %H:%M:%S,%3f       # 2024-01-15 14:30:45,123 (Python logging)

For complete format reference, see:
https://docs.rs/chrono/latest/chrono/format/strftime/index.html
"#;
    println!("{}", help_text);
}

/// Print available Rhai functions help
fn print_functions_help() {
    let help_text = rhai_functions::docs::generate_help_text();
    println!("{}", help_text);
}

/// Print Rhai scripting guide
fn print_rhai_help() {
    let help_text = r#"
Rhai Scripting Guide for Kelora:

For complete Rhai language documentation, visit: https://rhai.rs

BASIC CONCEPTS:
  e                                    Current event (renamed from 'event')
  e.field                              Access field directly
  e.nested.field                       Access nested fields
  e.scores[1]                          Array access (supports negative indexing)
  e.headers["user-agent"]              Field access with special characters

VARIABLE DECLARATION:
  let myfield = e.col("1,2")           Always use 'let' for new variables
  let result = e.user.name.lower()     Chain operations and store result

FIELD EXISTENCE AND SAFETY:
  "field" in e                         Check if field exists
  e.has_path("user.role")               Check nested field existence
  e.scores.len() > 0                   Check if array has elements
  type_of(e.field) != "()"             Check if field has a value

FIELD AND EVENT REMOVAL:
  e.field = ()                         Remove individual field
  e = ()                               Remove entire event (filters out)

KELORA-SPECIFIC FUNCTIONS:
  Use --help-functions to see all available functions for log processing:
  regex operations, IP handling, text manipulation, JSON parsing, 
  key-value extraction, array processing, safe field access, and utilities.

METHOD CHAINING EXAMPLES:
  e.message.extract_re("user=(\\w+)").upper()
  e.client_ip.mask_ip(2)
  e.url.extract_domain().lower()
  e.timestamp.to_datetime().format("%H:%M")

FUNCTION VS METHOD SYNTAX:
  extract_re(e.line, "\\d+")           Function style (avoids conflicts)
  e.line.extract_re("\\d+")            Method style (better for chaining)

Both syntaxes work identically. Use method syntax for readability and chaining,
function syntax when method names conflict with field names.

COMMON PATTERNS:
  # Safe field access with defaults
  let user_role = e.get_path("user.role", "guest");
  
  # Process arrays safely
  if e.events.len() > 0 {
      e.latest_event = e.events[-1];
      e.event_types = unique(e.events.map(|event| event.type));
  }
  
  # Conditional event removal
  if e.level != "ERROR" { e = (); }
  
  # Field cleanup and transformation
  e.password = (); e.ssn = ();  // Remove sensitive fields
  e.summary = e.method + " " + e.status;

ARRAY PROCESSING:
  sorted(e.scores)                     Sort array numerically/lexicographically
  reversed(e.items)                    Reverse array order
  unique(e.tags)                       Remove duplicate elements
  sorted_by(e.users, "age")            Sort array of objects by field
  e.tags.join(", ")                    Join array elements

JSON ARRAY HANDLING:
  JSON arrays are automatically converted to native Rhai arrays with full
  functionality (sorted, reversed, unique, etc.) while maintaining proper
  JSON types in output formats.

SIDE EFFECTS IN QUIET MODE:
  print("debug info")                  Levels -q/-qq: visible, -qqq: suppressed
  eprint("error details")              Levels -q/-qq: visible, -qqq: suppressed
  # File operations preserved at all quiet levels

ERROR HANDLING:
  Kelora uses resilient processing by default:
  • Parse errors: Skip malformed lines, continue processing
  • Filter errors: Evaluate to false, skip event
  • Transform errors: Return original event unchanged
  Use --strict for fail-fast behavior on any error.

For complete function reference: kelora --help-functions
For usage examples: kelora --help (see examples section)
For time format help: kelora --help-time
For multiline strategy help: kelora --help-multiline
"#;
    println!("{}", help_text);
}

/// Print multiline strategy help
fn print_multiline_help() {
    let help_text = r#"
Multiline Strategy Reference for --multiline:

Kelora supports several multiline strategies. Multiline remains off unless you
request it with -M/--multiline. Pick a preset below or supply a custom
strategy string to control how lines are grouped.

QUICK PRESETS (recommended):

stacktrace
  Timestamp anchored framing for typical application logs and stack traces
  Equivalent to: -M timestamp

docker
  RFC3339 timestamp framing used by Docker JSON logs
  Equivalent to: -M timestamp:pattern=^\d{4}-\d{2}-\d{2}T

syslog
  RFC3164/5424 style headers ("Jan  2", "2024-01-02T...")
  Equivalent to: -M timestamp:pattern=^(<\d+>\d\s+\d{4}-\d{2}-\d{2}T|\w{3}\s+\d{1,2})

combined
  Apache/Nginx access logs with remote host prefix
  Equivalent to: -M start:^\S+\s+\S+\s+\S+\s+\[

nginx
  Bracketed date headers like "[10/Oct/2000:13:55:36 +0000]"
  Equivalent to: -M timestamp:pattern=^\[[0-9]{2}/[A-Za-z]{3}/[0-9]{4}:

continuation
  Join lines ending with the continuation marker (default: \)
  Equivalent to: -M backslash

block
  Treat BEGIN...END style sections as one event
  Equivalent to: -M boundary:start=^BEGIN:end=^END

ADVANCED RECIPES (build your own):

timestamp[:pattern=REGEX]
  Events start with timestamp pattern (anchored to the beginning of the line)
  Combine with --ts-format=<chrono fmt> to align with your exact timestamp prefix

indent[:spaces=N|tabs|mixed]
  Continuation lines are indented, new events start at column 1

start:REGEX
  Events start when the line matches the regular expression

end:REGEX
  Events end when the line matches the regular expression

boundary:start=START_REGEX:end=END_REGEX
  Events start at START_REGEX and close at END_REGEX

backslash[:char=C]
  Lines ending with the continuation character continue the event

whole
  Read the entire input as a single event (loads everything into memory)

PERFORMANCE NOTES:
- Multiline buffers events until a boundary arrives; watch memory usage
- --batch-size helps control memory in parallel mode  
- --take N applies after multiline reconstruction, not to raw lines
- The whole strategy buffers the entire input

For complete CLI reference: kelora --help
For Rhai scripting help: kelora --help-rhai
"#;
    println!("{}", help_text);
}
