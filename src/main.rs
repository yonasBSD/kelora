use anyhow::Result;
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use crossbeam_channel::{bounded, select, unbounded, Receiver, Sender};
use std::sync::atomic::Ordering;

use crate::engine::RhaiEngine;
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

use crate::rhai_functions::file_ops::{self, FileOpMode};
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
        // For stdin with potential gzip/zstd, handle decompression first
        let stdin_reader = readers::ChannelStdinReader::new()?;
        let processed_stdin = decompression::maybe_decompress(stdin_reader)?;
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

    let preserve_order = !config.performance.no_preserve_order;
    let parallel_config = ParallelConfig {
        num_workers: config.effective_threads(),
        batch_size,
        batch_timeout_ms: config.performance.batch_timeout,
        preserve_order,
        buffer_size: Some(10000),
    };

    let processor =
        ParallelProcessor::new(parallel_config).with_take_limit(config.processing.take_limit);

    // Create pipeline builder and components for begin/end stages
    let pipeline_builder = create_pipeline_builder_from_config(config);
    let (_pipeline, begin_stage, end_stage, mut ctx) = pipeline_builder
        .clone()
        .build(config.processing.stages.clone())?;

    file_ops::set_mode(FileOpMode::Sequential);

    // Execute begin stage sequentially if provided
    if let Err(e) = begin_stage.execute(&mut ctx) {
        return Err(anyhow::anyhow!("Begin stage error: {}", e));
    }

    // Get reader using pipeline builder
    let reader = create_input_reader(config)?;

    // Process stages in parallel
    if preserve_order {
        file_ops::set_mode(FileOpMode::ParallelOrdered);
    } else {
        file_ops::set_mode(FileOpMode::ParallelUnordered);
    }

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
    file_ops::set_mode(FileOpMode::Sequential);
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
        let processed_stdin = decompression::maybe_decompress(stdin_reader)?;
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
        let processed_stdin = decompression::maybe_decompress(stdin_reader)?;
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
            match ctrl_rx.try_recv() {
                Ok(Ctrl::Shutdown { immediate }) => {
                    let _ = sender.send(ReaderMessage::Eof);
                    if immediate {
                        return Ok(());
                    }
                    break;
                }
                Ok(Ctrl::PrintStats) => {
                    // Reader thread doesn't have stats to print, ignore
                }
                Err(_) => {
                    // No message, continue
                }
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
            match ctrl_rx.try_recv() {
                Ok(Ctrl::Shutdown { immediate }) => {
                    let _ = sender.send(ReaderMessage::Eof);
                    if immediate {
                        return Ok(());
                    }
                    break;
                }
                Ok(Ctrl::PrintStats) => {
                    // Reader thread doesn't have stats to print, ignore
                }
                Err(_) => {
                    // No message, continue
                }
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

    file_ops::set_mode(FileOpMode::Sequential);

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
                        Ok(Ctrl::PrintStats) => {
                            // Print current stats to stderr (sequential mode)
                            let mut current_stats = get_thread_stats();
                            let internal_tracking = RhaiEngine::get_thread_internal_state();
                            current_stats.extract_discovered_from_tracking(&internal_tracking);
                            let stats_message = config.format_stats_message(
                                &current_stats.format_stats_for_signal(config.input.multiline.is_some())
                            );
                            let _ = SafeStderr::new().writeln(&stats_message);
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
                        Ok(Ctrl::PrintStats) => {
                            // Print current stats to stderr (sequential mode)
                            let mut current_stats = get_thread_stats();
                            let internal_tracking = RhaiEngine::get_thread_internal_state();
                            current_stats.extract_discovered_from_tracking(&internal_tracking);
                            let stats_message = config.format_stats_message(
                                &current_stats.format_stats_for_signal(config.input.multiline.is_some())
                            );
                            let _ = SafeStderr::new().writeln(&stats_message);
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
    if let Err(err) = file_ops::execute_ops(&formatted.file_ops) {
        return Err(io::Error::other(err));
    }

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

    // Set processed begin/end scripts with includes applied
    let (processed_begin, processed_end) = cli.get_processed_begin_end(&matches)?;
    config.processing.begin = processed_begin;
    config.processing.end = processed_end;

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

/// Extract --save-alias argument from raw args
fn extract_save_alias_arg(args: &[String]) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == "--save-alias" && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}

/// Handle --save-alias command
fn handle_save_alias(raw_args: &[String], alias_name: &str, no_emoji: bool) {
    use crate::config_file::ConfigFile;

    // Extract --config-file if specified
    let mut config_file_path: Option<String> = None;
    let mut command_args = Vec::new();
    let mut i = 0;
    while i < raw_args.len() {
        if raw_args[i] == "--save-alias" {
            // Skip --save-alias and its argument
            i += 2;
        } else if raw_args[i] == "--no-emoji" {
            // Skip --no-emoji as it's only for the save operation display
            i += 1;
        } else if raw_args[i] == "--config-file" && i + 1 < raw_args.len() {
            // Extract --config-file for saving
            config_file_path = Some(raw_args[i + 1].clone());
            i += 2;
        } else {
            command_args.push(raw_args[i].clone());
            i += 1;
        }
    }

    // Skip the program name (first argument)
    if !command_args.is_empty() {
        command_args.remove(0);
    }

    // Check if we have any command left to save
    if command_args.is_empty() {
        let prefix = if no_emoji { "kelora:" } else { "⚠️" };
        eprintln!("{} No command to save as alias '{}'", prefix, alias_name);
        std::process::exit(2);
    }

    // Join the arguments back into a single string
    let alias_value = shell_words::join(command_args);

    // Save the alias to the specified config file or auto-detect
    let target_path = config_file_path.as_ref().map(std::path::Path::new);
    match ConfigFile::save_alias(alias_name, &alias_value, target_path) {
        Ok((config_path, previous_value)) => {
            let success_prefix = if no_emoji { "kelora:" } else { "🔹" };
            println!(
                "{} Alias '{}' saved to {}",
                success_prefix,
                alias_name,
                config_path.display()
            );

            if let Some(prev) = previous_value {
                let info_prefix = if no_emoji { "kelora:" } else { "🔹" };
                println!("{} Replaced previous alias:", info_prefix);
                println!("    {} = {}", alias_name, prev);
            }
        }
        Err(e) => {
            let error_prefix = if no_emoji { "kelora:" } else { "⚠️" };
            eprintln!(
                "{} Failed to save alias '{}': {}",
                error_prefix, alias_name, e
            );
            std::process::exit(1);
        }
    }
}

/// Process command line arguments with config file support
fn process_args_with_config(stderr: &mut SafeStderr) -> (ArgMatches, Cli) {
    // Get raw command line arguments
    let raw_args: Vec<String> = std::env::args().collect();

    // Extract --config-file argument early for use by config commands
    let config_file_path = extract_config_file_arg(&raw_args);

    // Check for config-related option conflicts
    let has_show_config = raw_args.iter().any(|arg| arg == "--show-config");
    let has_edit_config = raw_args.iter().any(|arg| arg == "--edit-config");
    let has_ignore_config = raw_args.iter().any(|arg| arg == "--ignore-config");

    if has_show_config && has_edit_config {
        stderr
            .writeln("kelora: Error: --show-config and --edit-config are mutually exclusive")
            .unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    if has_ignore_config && has_edit_config {
        stderr
            .writeln("kelora: Error: --ignore-config and --edit-config are mutually exclusive")
            .unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    // Check for --show-config first, before any other processing
    if has_show_config {
        ConfigFile::show_config();
        std::process::exit(0);
    }

    // Check for --edit-config
    if has_edit_config {
        ConfigFile::edit_config(config_file_path.as_deref());
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

    // Check for --help-examples
    if raw_args.iter().any(|arg| arg == "--help-examples") {
        print_examples_help();
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

    // Check for --save-alias before other processing
    if let Some(alias_name) = extract_save_alias_arg(&raw_args) {
        let no_emoji =
            raw_args.iter().any(|arg| arg == "--no-emoji") || std::env::var("NO_EMOJI").is_ok();
        handle_save_alias(&raw_args, &alias_name, no_emoji);
        std::process::exit(0);
    }

    // Check for --ignore-config
    let ignore_config = has_ignore_config;

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

Use with:
  --ts-format <FMT>     Describe how timestamps are parsed
  --input-tz <TZ>       Supply a timezone for inputs without offsets (e.g., --input-tz UTC)
  --multiline timestamp Align multiline detection with the same leading pattern

Basic date/time components:
%Y  Year with century (e.g., 2024)
%y  Year without century (00-99)
%m  Month as zero-padded decimal (01-12)
%b  Month as abbreviated name (Jan, Feb, ..., Dec)
%B  Month as full name (January, February, ..., December)
%d  Day of month as zero-padded decimal (01-31)
%j  Day of year as zero-padded decimal (001-366)
%H  Hour (24-hour) as zero-padded decimal (00-23)
%I  Hour (12-hour) as zero-padded decimal (01-12)
%p  AM/PM indicator
%M  Minute as zero-padded decimal (00-59)
%S  Second as zero-padded decimal (00-59)

Subsecond precision cheatsheet:
%f   Microseconds (000000-999999)
%3f  Milliseconds (000-999)
%6f  Microseconds (000000-999999)
%9f  Nanoseconds (000000000-999999999)
%.f  Auto-match subseconds with flexible precision

Time zone tokens:
%z  UTC offset (+HHMM or -HHMM)
%Z  Time zone name (if available)
%:z UTC offset with colon (+HH:MM or -HH:MM)

Weekday helpers:
%w  Weekday as decimal (0=Sunday, 6=Saturday)
%a  Weekday as abbreviated name (Sun, Mon, ..., Sat)
%A  Weekday as full name (Sunday, Monday, ..., Saturday)

Week numbers:
%W  Week number (Monday as first day of week)
%U  Week number (Sunday as first day of week)

Common examples:
%Y-%m-%d %H:%M:%S           2024-01-15 14:30:45
%Y-%m-%dT%H:%M:%S%z         2024-01-15T14:30:45+0000
%Y-%m-%d %H:%M:%S%.f        2024-01-15 14:30:45.123456
%b %d %H:%M:%S              Jan 15 14:30:45 (syslog format)
%d/%b/%Y:%H:%M:%S %z        15/Jan/2024:14:30:45 +0000 (Apache access log)
%Y-%m-%d %H:%M:%S,%3f       2024-01-15 14:30:45,123 (Python logging)

Naive timestamp + timezone example:
  kelora app.log --ts-format "%Y-%m-%d %H:%M:%S" --input-tz Europe/Berlin
  (parses local timestamps and normalises them internally)

Shell tip: wrap the entire format in single quotes or escape % symbols to keep
  your shell from expanding them.

For the full chrono format reference, see:
https://docs.rs/chrono/latest/chrono/format/strftime/index.html
"#;
    println!("{}", help_text);
}

/// Print available Rhai functions help
fn print_functions_help() {
    let help_text = rhai_functions::docs::generate_help_text();
    println!("{}", help_text);
}

/// Print practical Rhai examples
fn print_examples_help() {
    let help_text = rhai_functions::docs::generate_examples_text();
    println!("{}", help_text);
}

/// Print Rhai scripting guide
fn print_rhai_help() {
    let help_text = r#"
Rhai Language Guide for Kelora:

This guide covers Rhai language fundamentals for programmers familiar with Python, JavaScript, or Bash.
For practical examples: kelora --help-examples
For complete function reference: kelora --help-functions
For Rhai language details: https://rhai.rs

VARIABLES & TYPES:
  let x = 42;                          Variable declaration (required for new vars)
  let name = "alice";                  String (double quotes only)
  let active = true;                   Boolean (true/false)
  let tags = [1, 2, 3];                Array (dynamic, mixed types ok)
  let user = #{name: "bob", age: 30};  Map/object literal
  let empty = ();                      Unit type (Rhai's "nothing", not null/undefined)

  type_of(x)                           Returns type as string: "i64", "string", "array", "map", "()"
  x = "hello";                         Dynamic typing: variables can change type

OPERATORS:
  Arithmetic:  +  -  *  /  %  **       (power: 2**3 == 8)
  Comparison:  ==  !=  <  >  <=  >=
  Logical:     &&  ||  !
  Bitwise:     &  |  ^  <<  >>
  Assignment:  =  +=  -=  *=  /=  %=  &=  |=  ^=  <<=  >>=
  Range:       1..5  1..=5            (exclusive/inclusive, for loops only)
  Membership:  "key" in map            (check map key existence)

CONTROL FLOW:
  if x > 10 {                          If-else (braces required)
      print("big");
  } else if x > 5 {
      print("medium");
  } else {
      print("small");
  }

  switch x {                           Switch expression (returns value)
      1 => "one",
      2 | 3 => "two or three",
      4..=6 => "four to six",
      _ => "other"                     (underscore = default)
  }

LOOPS:
  for i in 0..10 { print(i); }         Range loop (0..10 excludes 10, 0..=10 includes)
  for item in array { print(item); }   Array iteration
  for (key, value) in map { ... }      Map iteration

  while condition { ... }              While loop
  loop { if done { break; } }          Infinite loop (use break/continue)

FUNCTIONS & CLOSURES:
  fn add(a, b) { a + b }               Function definition (last expr is return value)
  fn greet(name) {                     Explicit return
      return "Hello, " + name;
  }

  let double = |x| x * 2;              Closure syntax
  [1,2,3].map(|x| x * 2)               Common in array methods
  [1,2,3].filter(|x| x > 1)            Predicate closures

FUNCTION-AS-METHOD SYNTAX (Rhai special feature):
  extract_re(e.line, "\d+")            Function call style
  e.line.extract_re("\d+")             Method call style (same thing!)

  Rhai allows calling any function as a method on its first argument.
  Use method style for chaining: e.url.extract_domain().lower().strip()

RHAI QUIRKS & GOTCHAS:
  • Strings use double quotes only: "hello" (not 'hello')
  • Semicolons recommended (optional at end of blocks, required for multiple statements)
  • No null/undefined: use unit type () to represent "nothing"
  • No implicit type conversion: "5" + 3 is error (use "5".to_int() + 3)
  • let required for new variables (x = 1 errors if x not declared)
  • Arrays/maps are reference types: modifying copies affects original
  • Last expression in block is return value (no return needed)
  • Single-line comments: // ... (multi-line: /* ... */)
  • Function calls without parens ok if no args: e.len (same as e.len())

KELORA PIPELINE STAGES:
  --begin         Pre-run once before parsing; populate global `conf` map (becomes read-only)
  --filter        Boolean gate per event (true keeps, false drops); repeatable, ordered
  --exec / -e     Transform per event; repeatable, ordered
  --exec-file     Same as --exec, reads script from file
  --end           Post-run once after processing; access global `metrics` map for reports

Prerequisites: --allow-fs-writes (file I/O), --window N (windowing), --metrics (tracking)

KELORA EVENT ACCESS:
  e                                    Current event (global variable in --filter/--exec)
  e.field                              Direct field access
  e.nested.field                       Nested field traversal (maps)
  e.scores[1]                          Array indexing (0-based, negative ok: -1 = last)
  e.headers["user-agent"]              Bracket notation for special chars in keys

  "field" in e                         Check top-level field exists
  e.has_path("user.role")              Check nested path exists (safe)
  e.get_path("user.role", "guest")     Get nested with default fallback

  e.field = ()                         Remove field (unit assignment)
  e = ()                               Remove entire event (becomes empty, filtered out)

ARRAY & MAP OPERATIONS:
  JSON arrays → native Rhai arrays (full functionality)
  sorted(e.scores)                     Sort numerically/lexicographically
  reversed(e.items)                    Reverse order
  unique(e.tags)                       Remove duplicates
  sorted_by(e.users, "age")            Sort objects by field
  e.tags.join(", ")                    Join to string

  emit_each(e.items)                   Fan out: each array element → separate event
  emit_each(e.items, #{ctx: "x"})      Fan out with base fields added to each

COMMON PATTERNS:
  # Safe nested access
  let role = e.get_path("user.role", "guest");

  # Type conversion with fallback
  let port = to_int_or(e.port, 8080);

  # Array safety
  if e.items.len() > 0 { e.first = e.items[0]; }

  # Conditional field removal
  if e.level != "DEBUG" { e.stack_trace = (); }

  # Method chaining
  e.domain = e.url.extract_domain().to_lower().strip();

  # Map iteration
  for (key, val) in e { print(key + " = " + val); }

GLOBAL CONTEXT:
  conf                                 Global config map (read-only after --begin)
  metrics                              Global metrics map (from track_* calls, read in --end)
  get_env("VAR", "default")            Environment variable access

ERROR HANDLING MODES:
  Default (resilient):
    • Parse errors → skip line, continue
    • Filter errors → treat as false, drop event
    • Exec errors → rollback, keep original event
  --strict mode:
    • Any error → abort with exit code 1

QUIET MODE SIDE EFFECTS:
  print("msg")                         Levels -q/-qq: visible, -qqq: suppressed
  eprint("err")                        Levels -q/-qq: visible, -qqq: suppressed
  File ops (append_file, etc.)         Always work (needs --allow-fs-writes)

See also:
  kelora --help-examples   Practical log analysis patterns
  kelora --help-functions  Complete function catalogue
  kelora --help-time       Timestamp format reference
  https://rhai.rs          Full Rhai language documentation
"#;
    println!("{}", help_text);
}

/// Print multiline strategy help
fn print_multiline_help() {
    let help_text = r#"
Multiline Strategy Reference for --multiline:

Quick usage:
  kelora sample.log -M stacktrace
  kelora sample.log -M "timestamp:pattern=^\d{4}-" --ts-format "%Y-%m-%d %H:%M:%S"

Multiline stays off unless you enable it with -M/--multiline. Choose a preset or
craft a custom strategy, then layer your parser (for example, --format raw) so
multiline reconstruction happens before structured parsing.

QUICK PRESETS (recommended):

stacktrace
  Timestamp anchored framing for application logs and stack traces
  Equivalent to: -M timestamp
  Best for: services that start each event with an ISO-like timestamp

docker
  RFC3339 timestamp framing used by Docker JSON logs
  Equivalent to: -M timestamp:pattern=^\d{4}-\d{2}-\d{2}T
  Best for: container runtimes emitting JSON lines

syslog
  RFC3164/5424 style headers ("Jan  2", "2024-01-02T...")
  Equivalent to: -M timestamp:pattern=^(<\d+>\d\s+\d{4}-\d{2}-\d{2}T|\w{3}\s+\d{1,2})
  Best for: network gear and daemons with classic syslog headers

combined
  Apache/Nginx access logs with remote host prefix
  Equivalent to: -M start:^\S+\s+\S+\s+\S+\s+\[
  Best for: web access logs parsed later via --format combined/logfmt/json

nginx
  Bracketed date headers like "[10/Oct/2000:13:55:36 +0000]"
  Equivalent to: -M timestamp:pattern=^\[[0-9]{2}/[A-Za-z]{3}/[0-9]{4}:
  Best for: nginx access/error logs

continuation
  Join lines ending with the continuation marker (default: \)
  Equivalent to: -M backslash
  Best for: languages that escape newlines with a trailing backslash

block
  Treat BEGIN...END style sections as one event
  Equivalent to: -M boundary:start=^BEGIN:end=^END
  Best for: tools that delimit multi-line records explicitly

ADVANCED RECIPES (build your own):

timestamp[:pattern=REGEX]
  Events start with a timestamp pattern (anchored at line start)
  Pair with --ts-format=<chrono fmt> to parse the captured prefix

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

INTERACTIONS:
- Combine with --extract-prefix for prefix cleanup before parsing
- --take N applies after multiline reconstruction, not to raw lines
- Parallel mode buffers per worker; tune --batch-size/--batch-timeout as needed

TROUBLESHOOTING:
- Run with --stats or --metrics to monitor buffered event counts
- Add --brief or --pretty to inspect reconstructed events quickly
- If buffers grow unbounded, tighten the regex or temporarily disable --parallel

PERFORMANCE NOTES:
- Multiline buffers until a boundary arrives; monitor memory on long runs
- --batch-size helps control memory in parallel mode
- The whole strategy buffers the entire input

For complete CLI reference: kelora --help
For Rhai scripting help: kelora --help-rhai
"#;
    println!("{}", help_text);
}
