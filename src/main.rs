use anyhow::Result;
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use std::io::{self, BufRead};
use std::sync::atomic::Ordering;

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
mod readers;
mod rhai_functions;
mod stats;
mod timestamp;
mod tty;
mod unix;

use config::KeloraConfig;
use config_file::ConfigFile;
use parallel::{ParallelConfig, ParallelProcessor};
use pipeline::{
    create_input_reader, create_pipeline_builder_from_config, create_pipeline_from_config,
};
use stats::{
    get_thread_stats, stats_add_error, stats_add_line_filtered, stats_add_line_output,
    stats_add_line_read, stats_finish_processing, stats_start_timer, ProcessingStats,
};
use unix::{
    check_termination, ExitCode, ProcessCleanup, SafeFileOut, SafeStderr, SafeStdout,
    SignalHandler, SHOULD_TERMINATE,
};

/// Trait for output writing that works with both stdout and file output
trait OutputWriter {
    fn writeln(&mut self, data: &str) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
}

impl OutputWriter for SafeStdout {
    fn writeln(&mut self, data: &str) -> Result<()> {
        self.writeln(data)
    }

    fn flush(&mut self) -> Result<()> {
        self.flush()
    }
}

impl OutputWriter for SafeFileOut {
    fn writeln(&mut self, data: &str) -> Result<()> {
        self.writeln(data)
    }

    fn flush(&mut self) -> Result<()> {
        self.flush()
    }
}


// Use CLI types from library
use kelora::{InputFormat, OutputFormat, ErrorStrategy, FileOrder, Cli};
use config::ScriptStageType;


fn main() -> Result<()> {
    // Initialize signal handling early
    let _signal_handler = SignalHandler::new()
        .map_err(|e| {
            eprintln!("Failed to initialize signal handling: {}", e);
            ExitCode::GeneralError.exit();
        })
        .unwrap();

    // Initialize process cleanup
    let _cleanup = ProcessCleanup::new();

    // Initialize safe I/O wrappers
    let mut stdout = SafeStdout::new();
    let mut stderr = SafeStderr::new();

    // Process command line arguments with config file support
    let (matches, cli) = process_args_with_config(&mut stderr);

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

    // Create configuration from CLI and set stages
    let mut config = KeloraConfig::from_cli(&cli);
    config.processing.stages = ordered_stages;

    // Parse timestamp filter arguments if provided
    if cli.since.is_some() || cli.until.is_some() {
        let since = if let Some(ref since_str) = cli.since {
            match crate::timestamp::parse_timestamp_arg(since_str) {
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
            match crate::timestamp::parse_timestamp_arg(until_str) {
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

        config.processing.timestamp_filter =
            Some(crate::config::TimestampFilterConfig { since, until });
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

    // Parse multiline configuration if provided, or apply format defaults
    if let Some(multiline_str) = &cli.multiline {
        match config::MultilineConfig::parse(multiline_str) {
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
    } else {
        // Apply format-specific default multiline configuration
        config.input.multiline = config.input.format.default_multiline();
    }

    // Validate arguments early
    if let Err(e) = validate_cli_args(&cli) {
        stderr
            .writeln(&config.format_error_message(&format!("Error: {}", e)))
            .unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    // Determine processing mode using config
    let use_parallel = config.should_use_parallel();

    // Start statistics collection if enabled
    if config.output.stats {
        stats_start_timer();
    }

    let final_stats =
        if use_parallel {
            // Get effective values from config for parallel mode
            let batch_size = config.effective_batch_size();

            // Handle output destination (stdout vs file)
            let stats = if let Some(ref output_file_path) = cli.output_file {
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
                run_parallel(&config, batch_size, file_output, &mut stderr)
            } else {
                // Use stdout output
                let stdout_output = SafeStdout::new();
                run_parallel(&config, batch_size, stdout_output, &mut stderr)
            };

            // Print parallel stats if enabled (only if not terminated, will be handled later)
            if config.output.stats && !SHOULD_TERMINATE.load(Ordering::Relaxed) {
                if let Some(ref s) = stats {
                    stderr
                        .writeln(&config.format_stats_message(
                            &s.format_stats(config.input.multiline.is_some()),
                        ))
                        .unwrap_or(());
                }
            }
            stats
        } else {
            // Handle output destination (stdout vs file)
            if let Some(ref output_file_path) = cli.output_file {
                // Use file output
                let mut file_output = match SafeFileOut::new(output_file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        stderr
                            .writeln(&config.format_error_message(&e.to_string()))
                            .unwrap_or(());
                        ExitCode::GeneralError.exit();
                    }
                };
                run_sequential(&config, &mut file_output, &mut stderr);
            } else {
                // Use stdout output
                run_sequential(&config, &mut stdout, &mut stderr);
            }

            // Print summary if enabled (only if not terminated)
            if config.output.summary && !SHOULD_TERMINATE.load(Ordering::Relaxed) {
                let tracked = crate::rhai_functions::tracking::get_thread_tracking_state();
                let summary_lines = config.format_tracked_summary(&tracked);
                stderr
                    .writeln(&config.format_summary_message(""))
                    .unwrap_or(());
                for line in summary_lines.lines() {
                    stderr.writeln(line).unwrap_or(());
                }
            }

            // Finish statistics collection and print stats if enabled (only if not terminated)
            if config.output.stats && !SHOULD_TERMINATE.load(Ordering::Relaxed) {
                stats_finish_processing();
                let stats = get_thread_stats();
                stderr
                    .writeln(&config.format_stats_message(
                        &stats.format_stats(config.input.multiline.is_some()),
                    ))
                    .unwrap_or(());
            }
            None
        };

    // Check if we were terminated by a signal and print stats
    if SHOULD_TERMINATE.load(Ordering::Relaxed) {
        if config.output.stats {
            if use_parallel {
                // For parallel mode, try to get stats from the processor if available
                if let Some(stats) = final_stats {
                    stderr
                        .writeln(&config.format_stats_message(
                            &stats.format_stats(config.input.multiline.is_some()),
                        ))
                        .unwrap_or(());
                } else {
                    stderr
                        .writeln(&config.format_stats_message("Processing interrupted"))
                        .unwrap_or(());
                }
            } else {
                // For sequential mode, we can still get stats from the current thread
                stats_finish_processing();
                let stats = get_thread_stats();
                stderr
                    .writeln(&config.format_stats_message(
                        &stats.format_stats(config.input.multiline.is_some()),
                    ))
                    .unwrap_or(());
            }
        }
        ExitCode::SignalInt.exit();
    }

    // Clean shutdown
    ExitCode::Success.exit();
}

/// Run parallel processing mode
/// Note: stdout parameter is currently unused as ParallelProcessor creates its own SafeStdout,
/// but kept for consistency with run_sequential and future flexibility
fn run_parallel<W: std::io::Write + Send + 'static>(
    config: &KeloraConfig,
    batch_size: usize,
    output: W,
    stderr: &mut SafeStderr,
) -> Option<ProcessingStats> {
    // Parallel processing mode with proper Unix behavior
    let parallel_config = ParallelConfig {
        num_workers: config.effective_threads(),
        batch_size,
        batch_timeout_ms: config.performance.batch_timeout,
        preserve_order: !config.performance.no_preserve_order,
        buffer_size: Some(10000),
    };

    let processor = ParallelProcessor::new(parallel_config);

    // Create pipeline builder and components for begin/end stages
    let pipeline_builder = create_pipeline_builder_from_config(config);
    let (_pipeline, begin_stage, end_stage, mut ctx) = match pipeline_builder
        .clone()
        .build(config.processing.stages.clone())
    {
        Ok(pipeline_components) => pipeline_components,
        Err(e) => {
            stderr
                .writeln(&config.format_error_message(&format!("Failed to create pipeline: {}", e)))
                .unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };

    // Execute begin stage sequentially if provided
    execute_begin_stage(&begin_stage, &mut ctx, config, stderr);

    // Get reader using pipeline builder
    let reader = match create_input_reader(config) {
        Ok(r) => r,
        Err(e) => {
            stderr
                .writeln(
                    &config.format_error_message(&format!("Failed to create input reader: {}", e)),
                )
                .unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };

    // Process stages in parallel
    if let Err(e) = processor.process_with_pipeline(
        reader,
        pipeline_builder,
        config.processing.stages.clone(),
        config,
        output,
    ) {
        stderr
            .writeln(&config.format_error_message(&format!("Parallel processing error: {}", e)))
            .unwrap_or(());
        ExitCode::GeneralError.exit();
    }

    // Merge the parallel tracked state with our pipeline context
    let parallel_tracked = processor.get_final_tracked_state();

    // Extract internal stats from tracking system before merging (if stats enabled)
    if config.output.stats {
        processor
            .extract_final_stats_from_tracking(&parallel_tracked)
            .unwrap_or(());
    }

    // Filter out stats from user-visible context and merge the rest
    for (key, dynamic_value) in parallel_tracked {
        if !key.starts_with("__internal_")
            && !key.starts_with("__kelora_stats_")
            && !key.starts_with("__op___kelora_stats_")
        {
            ctx.tracker.insert(key, dynamic_value);
        }
    }

    // Print summary if enabled (only if not terminated)
    if config.output.summary && !SHOULD_TERMINATE.load(Ordering::Relaxed) {
        let summary_lines = config.format_tracked_summary(&ctx.tracker);
        stderr
            .writeln(&config.format_summary_message(""))
            .unwrap_or(());
        for line in summary_lines.lines() {
            stderr.writeln(line).unwrap_or(());
        }
    }

    // Execute end stage sequentially with merged state
    execute_end_stage(&end_stage, &ctx, config, stderr);

    // Get final stats if enabled (even if terminated) - do this after end stage
    // to ensure we capture all worker statistics that may have been accumulated
    if config.output.stats {
        Some(processor.get_final_stats())
    } else {
        None
    }
}

/// Process a single line in sequential mode with filename tracking and CSV schema detection
#[allow(clippy::too_many_arguments)]
fn process_line<W: OutputWriter>(
    line_result: io::Result<String>,
    line_num: &mut usize,
    skipped_lines: &mut usize,
    pipeline: &mut pipeline::Pipeline,
    ctx: &mut pipeline::PipelineContext,
    config: &KeloraConfig,
    output: &mut W,
    stderr: &mut SafeStderr,
    current_filename: Option<String>,
    current_csv_headers: &mut Option<Vec<String>>,
    last_filename: &mut Option<String>,
) {
    let line = line_result
        .map_err(|e| {
            stderr
                .writeln(&config.format_error_message(&format!("Failed to read input line: {}", e)))
                .unwrap_or(());
            ExitCode::GeneralError.exit();
        })
        .unwrap();
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
        return;
    }

    // Apply ignore-lines filter if configured (early filtering before parsing)
    if let Some(ref ignore_regex) = config.input.ignore_lines {
        if ignore_regex.is_match(&line) {
            // Count filtered line for stats
            if config.output.stats {
                stats_add_line_filtered();
            }
            return;
        }
    }

    if line.trim().is_empty() {
        // Only skip empty lines for structured formats, not for line format
        if !matches!(config.input.format, config::InputFormat::Line) {
            return;
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
            config::InputFormat::Csv => crate::parsers::CsvParser::new_csv(),
            config::InputFormat::Tsv => crate::parsers::CsvParser::new_tsv(),
            config::InputFormat::Csvnh => crate::parsers::CsvParser::new_csv_no_headers(),
            config::InputFormat::Tsvnh => crate::parsers::CsvParser::new_tsv_no_headers(),
            _ => unreachable!(),
        };

        // Initialize headers from the first line
        let was_consumed =
            temp_parser
                .initialize_headers_from_line(&line)
                .unwrap_or_else(|e| {
                    stderr
                        .writeln(&config.format_error_message(&format!(
                            "Failed to initialize CSV headers: {}",
                            e
                        )))
                        .unwrap_or(());
                    ExitCode::GeneralError.exit();
                });

        // Get the initialized headers
        let headers = temp_parser.get_headers();
        *current_csv_headers = Some(headers.clone());
        *last_filename = current_filename.clone();

        // Rebuild the pipeline with new headers
        let mut pipeline_builder = create_pipeline_builder_from_config(config);
        pipeline_builder = pipeline_builder.with_csv_headers(headers);

        let (new_pipeline, _new_begin_stage, _new_end_stage, new_ctx) = pipeline_builder
            .build(config.processing.stages.clone())
            .unwrap_or_else(|e| {
                stderr
                    .writeln(&config.format_error_message(&format!(
                        "Failed to rebuild pipeline with CSV headers: {}",
                        e
                    )))
                    .unwrap_or(());
                ExitCode::GeneralError.exit();
            });

        *pipeline = new_pipeline;
        // Keep the existing context's tracking state but update the Rhai engine
        ctx.rhai = new_ctx.rhai;

        // If the first line was consumed as a header, don't process it as data
        if was_consumed {
            return;
        }
    }

    // Update metadata with filename tracking
    ctx.meta.line_number = Some(*line_num);
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
            for result in results {
                if !result.is_empty() {
                    output.writeln(&result).unwrap_or_else(|e| {
                        stderr
                            .writeln(&config.format_error_message(&format!("Output error: {}", e)))
                            .unwrap_or(());
                        ExitCode::GeneralError.exit();
                    });
                }
            }
            output.flush().unwrap_or_else(|e| {
                stderr
                    .writeln(&config.format_error_message(&format!("Flush error: {}", e)))
                    .unwrap_or(());
                ExitCode::GeneralError.exit();
            });
        }
        Err(e) => {
            // Count errors for stats
            if config.output.stats {
                stats_add_error();
            }

            stderr
                .writeln(
                    &config.format_error_message(&format!(
                        "Pipeline error on line {}: {}",
                        line_num, e
                    )),
                )
                .unwrap_or(());
            if let config::ErrorStrategy::Abort = config.processing.on_error {
                ExitCode::GeneralError.exit()
            }
        }
    }
}

/// Run sequential processing mode
fn run_sequential<W: OutputWriter>(config: &KeloraConfig, output: &mut W, stderr: &mut SafeStderr) {
    // Sequential processing mode using new pipeline architecture
    let (mut pipeline, begin_stage, end_stage, mut ctx) = match create_pipeline_from_config(config)
    {
        Ok(pipeline_components) => pipeline_components,
        Err(e) => {
            stderr
                .writeln(&config.format_error_message(&format!("Failed to create pipeline: {}", e)))
                .unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };

    // Execute begin stage
    execute_begin_stage(&begin_stage, &mut ctx, config, stderr);

    // For CSV formats, we need to track per-file schema
    let mut current_csv_headers: Option<Vec<String>> = None;
    let mut last_filename: Option<String> = None;

    // Process lines using pipeline
    let mut line_num = 0;
    let mut skipped_lines = 0;

    // Handle filename tracking by creating the appropriate reader
    if config.input.files.is_empty() {
        // Stdin processing - no filename tracking
        let stdin = io::stdin();
        let reader = stdin.lock();

        for line_result in reader.lines() {
            // Check for termination signal between lines
            if check_termination().is_err() {
                // Return early to allow graceful shutdown with stats
                return;
            }

            process_line(
                line_result,
                &mut line_num,
                &mut skipped_lines,
                &mut pipeline,
                &mut ctx,
                config,
                output,
                stderr,
                None,
                &mut current_csv_headers,
                &mut last_filename,
            );
        }
    } else {
        // File processing - with filename tracking
        let sorted_files =
            pipeline::builders::sort_files(&config.input.files, &config.input.file_order)
                .unwrap_or_else(|e| {
                    stderr
                        .writeln(
                            &config.format_error_message(&format!("Failed to sort files: {}", e)),
                        )
                        .unwrap_or(());
                    ExitCode::GeneralError.exit();
                });

        let mut multi_reader =
            crate::readers::MultiFileReader::new(sorted_files).unwrap_or_else(|e| {
                stderr
                    .writeln(&config.format_error_message(&format!(
                        "Failed to create multi-file reader: {}",
                        e
                    )))
                    .unwrap_or(());
                ExitCode::GeneralError.exit();
            });

        let mut line_buf = String::new();
        loop {
            // Check for termination signal between lines
            if check_termination().is_err() {
                // Return early to allow graceful shutdown with stats
                return;
            }

            line_buf.clear();
            let bytes_read = match multi_reader.read_line(&mut line_buf) {
                Ok(0) => break, // EOF
                Ok(n) => n,
                Err(e) => {
                    let line_result = Err(e);
                    let current_filename = multi_reader.current_filename().map(|s| s.to_string());
                    process_line(
                        line_result,
                        &mut line_num,
                        &mut skipped_lines,
                        &mut pipeline,
                        &mut ctx,
                        config,
                        output,
                        stderr,
                        current_filename,
                        &mut current_csv_headers,
                        &mut last_filename,
                    );
                    continue;
                }
            };

            if bytes_read > 0 {
                let current_filename = multi_reader.current_filename().map(|s| s.to_string());
                process_line(
                    Ok(line_buf.clone()),
                    &mut line_num,
                    &mut skipped_lines,
                    &mut pipeline,
                    &mut ctx,
                    config,
                    output,
                    stderr,
                    current_filename,
                    &mut current_csv_headers,
                    &mut last_filename,
                );
            }
        }
    }

    // Flush any remaining chunks
    match pipeline.flush(&mut ctx) {
        Ok(results) => {
            for result in results {
                if !result.is_empty() {
                    output.writeln(&result).unwrap_or_else(|e| {
                        stderr
                            .writeln(&config.format_error_message(&format!("Output error: {}", e)))
                            .unwrap_or(());
                        ExitCode::GeneralError.exit();
                    });
                }
            }
        }
        Err(e) => {
            stderr
                .writeln(&config.format_error_message(&format!("Pipeline flush error: {}", e)))
                .unwrap_or(());
        }
    }

    // Execute end stage
    execute_end_stage(&end_stage, &ctx, config, stderr);
}

/// Execute begin stage with shared error handling
fn execute_begin_stage(
    begin_stage: &pipeline::BeginStage,
    ctx: &mut pipeline::PipelineContext,
    config: &KeloraConfig,
    stderr: &mut SafeStderr,
) {
    if let Err(e) = begin_stage.execute(ctx) {
        stderr
            .writeln(&config.format_error_message(&format!("Begin stage error: {}", e)))
            .unwrap_or(());
        ExitCode::GeneralError.exit();
    }
}

/// Execute end stage with shared error handling
fn execute_end_stage(
    end_stage: &pipeline::EndStage,
    ctx: &pipeline::PipelineContext,
    config: &KeloraConfig,
    stderr: &mut SafeStderr,
) {
    if let Err(e) = end_stage.execute(ctx) {
        stderr
            .writeln(&config.format_error_message(&format!("End stage error: {}", e)))
            .unwrap_or(());
        ExitCode::GeneralError.exit();
    }
}

/// Validate CLI arguments for early error detection
fn validate_cli_args(cli: &Cli) -> Result<()> {
    // Check if input files exist (if specified)
    for file_path in &cli.files {
        if !std::path::Path::new(file_path).exists() {
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

/// Process command line arguments with config file support
fn process_args_with_config(stderr: &mut SafeStderr) -> (ArgMatches, Cli) {
    // Get raw command line arguments
    let raw_args: Vec<String> = std::env::args().collect();

    // Check for --show-config first, before any other processing
    if raw_args.iter().any(|arg| arg == "--show-config") {
        ConfigFile::show_config();
        std::process::exit(0);
    }

    // Check for --ignore-config
    let ignore_config = raw_args.iter().any(|arg| arg == "--ignore-config");

    let processed_args = if ignore_config {
        // Skip config file processing
        raw_args
    } else {
        // Load config file and process aliases
        match ConfigFile::load() {
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
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| {
        stderr
            .writeln(&format!("kelora: Error: {}", e))
            .unwrap_or(());
        std::process::exit(1);
    });

    // Apply config file defaults to CLI if not ignoring config
    let cli = if ignore_config {
        cli
    } else {
        match ConfigFile::load() {
            Ok(config_file) => apply_config_defaults(cli, &config_file),
            Err(_) => cli, // Already handled error above
        }
    };

    // Show usage if on TTY and no input files provided
    if crate::tty::is_stdin_tty() && cli.files.is_empty() {
        // Print brief usage with description and help hint
        println!("{}", Cli::command().render_usage());
        println!("A command-line log analysis tool with embedded Rhai scripting");
        println!("Try 'kelora --help' for more information.");
        std::process::exit(0);
    }

    (matches, cli)
}

/// Apply configuration file defaults to CLI arguments
fn apply_config_defaults(mut cli: Cli, config_file: &ConfigFile) -> Cli {
    // Apply defaults only if the CLI value is still at its default
    // This ensures CLI arguments take precedence over config file

    if let Some(format) = config_file.defaults.get("input_format") {
        // Only apply if format is still at default ("line")
        if matches!(cli.format, crate::InputFormat::Line) {
            cli.format = match format.as_str() {
                "jsonl" => crate::InputFormat::Jsonl,
                "line" => crate::InputFormat::Line,
                "logfmt" => crate::InputFormat::Logfmt,
                "syslog" => crate::InputFormat::Syslog,
                "cef" => crate::InputFormat::Cef,
                "csv" => crate::InputFormat::Csv,
                "apache" => crate::InputFormat::Apache,
                "nginx" => crate::InputFormat::Nginx,
                "cols" => crate::InputFormat::Cols,
                _ => cli.format, // Keep original if invalid
            };
        }
    }

    if let Some(output_format) = config_file.defaults.get("output_format") {
        if matches!(cli.output_format, crate::OutputFormat::Default) {
            cli.output_format = match output_format.as_str() {
                "jsonl" => crate::OutputFormat::Jsonl,
                "default" => crate::OutputFormat::Default,
                "logfmt" => crate::OutputFormat::Logfmt,
                "csv" => crate::OutputFormat::Csv,
                _ => cli.output_format,
            };
        }
    }

    if let Some(on_error) = config_file.defaults.get("on_error") {
        if matches!(cli.on_error, crate::ErrorStrategy::Print) {
            cli.on_error = match on_error.as_str() {
                "skip" => crate::ErrorStrategy::Skip,
                "abort" => crate::ErrorStrategy::Abort,
                "print" => crate::ErrorStrategy::Print,
                "stub" => crate::ErrorStrategy::Stub,
                _ => cli.on_error,
            };
        }
    }

    if let Some(file_order) = config_file.defaults.get("file_order") {
        if matches!(cli.file_order, crate::FileOrder::None) {
            cli.file_order = match file_order.as_str() {
                "none" => crate::FileOrder::None,
                "name" => crate::FileOrder::Name,
                "mtime" => crate::FileOrder::Mtime,
                _ => cli.file_order,
            };
        }
    }

    // Apply boolean flags from config if they weren't explicitly set
    if let Some(parallel) = config_file.defaults.get("parallel") {
        if !cli.parallel && parallel.parse::<bool>().unwrap_or(false) {
            cli.parallel = true;
        }
    }

    if let Some(core) = config_file.defaults.get("core") {
        if !cli.core && core.parse::<bool>().unwrap_or(false) {
            cli.core = true;
        }
    }

    if let Some(brief) = config_file.defaults.get("brief") {
        if !cli.brief && brief.parse::<bool>().unwrap_or(false) {
            cli.brief = true;
        }
    }

    if let Some(summary) = config_file.defaults.get("summary") {
        if !cli.summary && summary.parse::<bool>().unwrap_or(false) {
            cli.summary = true;
        }
    }

    if let Some(skip_lines) = config_file.defaults.get("skip_lines") {
        if cli.skip_lines.is_none() {
            if let Ok(value) = skip_lines.parse::<usize>() {
                cli.skip_lines = Some(value);
            }
        }
    }

    if let Some(stats) = config_file.defaults.get("stats") {
        if !cli.stats && stats.parse::<bool>().unwrap_or(false) {
            cli.stats = true;
        }
    }

    if let Some(stats_only) = config_file.defaults.get("stats_only") {
        if !cli.stats_only && stats_only.parse::<bool>().unwrap_or(false) {
            cli.stats_only = true;
        }
    }

    if let Some(no_emoji) = config_file.defaults.get("no_emoji") {
        if !cli.no_emoji && no_emoji.parse::<bool>().unwrap_or(false) {
            cli.no_emoji = true;
        }
    }

    if let Some(force_color) = config_file.defaults.get("force_color") {
        if !cli.force_color && force_color.parse::<bool>().unwrap_or(false) {
            cli.force_color = true;
        }
    }

    if let Some(no_color) = config_file.defaults.get("no_color") {
        if !cli.no_color && no_color.parse::<bool>().unwrap_or(false) {
            cli.no_color = true;
        }
    }

    // Apply numeric values
    if let Some(threads) = config_file.defaults.get("threads") {
        if cli.threads == 0 {
            if let Ok(thread_count) = threads.parse::<usize>() {
                cli.threads = thread_count;
            }
        }
    }

    if let Some(batch_size) = config_file.defaults.get("batch_size") {
        if cli.batch_size.is_none() {
            if let Ok(size) = batch_size.parse::<usize>() {
                cli.batch_size = Some(size);
            }
        }
    }

    if let Some(batch_timeout) = config_file.defaults.get("batch_timeout") {
        if cli.batch_timeout == 200 {
            // default value
            if let Ok(timeout) = batch_timeout.parse::<u64>() {
                cli.batch_timeout = timeout;
            }
        }
    }

    // Apply string values
    if let Some(ignore_lines) = config_file.defaults.get("ignore_lines") {
        if cli.ignore_lines.is_none() {
            cli.ignore_lines = Some(ignore_lines.clone());
        }
    }

    if let Some(multiline) = config_file.defaults.get("multiline") {
        if cli.multiline.is_none() {
            cli.multiline = Some(multiline.clone());
        }
    }

    if let Some(begin) = config_file.defaults.get("begin") {
        if cli.begin.is_none() {
            cli.begin = Some(begin.clone());
        }
    }

    if let Some(end) = config_file.defaults.get("end") {
        if cli.end.is_none() {
            cli.end = Some(end.clone());
        }
    }

    if let Some(inject_prefix) = config_file.defaults.get("inject_prefix") {
        if cli.inject_prefix.is_none() {
            cli.inject_prefix = Some(inject_prefix.clone());
        }
    }

    // Apply list values (only if CLI lists are empty)
    if let Some(filters) = config_file.defaults.get("filters") {
        if cli.filters.is_empty() {
            cli.filters = filters.split(',').map(|s| s.trim().to_string()).collect();
        }
    }

    if let Some(execs) = config_file.defaults.get("execs") {
        if cli.execs.is_empty() {
            cli.execs = execs.split(',').map(|s| s.trim().to_string()).collect();
        }
    }

    if let Some(levels) = config_file.defaults.get("levels") {
        if cli.levels.is_empty() {
            cli.levels = levels.split(',').map(|s| s.trim().to_string()).collect();
        }
    }

    if let Some(exclude_levels) = config_file.defaults.get("exclude_levels") {
        if cli.exclude_levels.is_empty() {
            cli.exclude_levels = exclude_levels
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
    }

    if let Some(keys) = config_file.defaults.get("keys") {
        if cli.keys.is_empty() {
            cli.keys = keys.split(',').map(|s| s.trim().to_string()).collect();
        }
    }

    if let Some(exclude_keys) = config_file.defaults.get("exclude_keys") {
        if cli.exclude_keys.is_empty() {
            cli.exclude_keys = exclude_keys
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
    }

    // Apply window_size from config if not explicitly set
    if let Some(window_size) = config_file.defaults.get("window_size") {
        if cli.window_size.is_none() {
            if let Ok(size) = window_size.parse::<usize>() {
                cli.window_size = Some(size);
            }
        }
    }

    cli
}
