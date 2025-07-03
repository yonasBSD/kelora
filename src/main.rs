use anyhow::Result;
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser};
use std::io::BufRead;
use std::sync::atomic::Ordering;

mod colors;
mod config;
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
mod tty;
mod unix;

use config::{KeloraConfig, ScriptStageType};
use parallel::{ParallelConfig, ParallelProcessor};
use pipeline::{
    create_input_reader, create_pipeline_builder_from_config, create_pipeline_from_config,
    create_sequential_input_reader,
};
use stats::{
    get_thread_stats, stats_add_error, stats_add_line_filtered, stats_add_line_output,
    stats_add_line_read, stats_finish_processing, stats_start_timer, ProcessingStats,
};
use unix::{
    check_termination, ExitCode, ProcessCleanup, SafeStderr, SafeStdout, SignalHandler,
    SHOULD_TERMINATE,
};

#[derive(Parser)]
#[command(name = "kelora")]
#[command(about = "A command-line log analysis tool with embedded Rhai scripting")]
#[command(
    long_about = "A command-line log analysis tool with embedded Rhai scripting\n\nMODES:\n  (default)   Sequential processing - best for streaming/interactive use\n  --parallel  Parallel processing - best for high-throughput batch analysis"
)]
#[command(version = "0.2.0")]
#[command(author = "Dirk Loss <mail@dirk-loss.de>")]
pub struct Cli {
    /// Input files (stdin if not specified)
    pub files: Vec<String>,

    /// Input format
    #[arg(
        short = 'f',
        long = "format",
        value_enum,
        default_value = "line",
        help_heading = "Input Options"
    )]
    pub format: InputFormat,

    /// File processing order: none (CLI order), name (alphabetical), mtime (modification time, oldest first)
    #[arg(
        long = "file-order",
        value_enum,
        default_value = "none",
        help_heading = "Input Options"
    )]
    pub file_order: FileOrder,

    /// Ignore input lines matching this regex pattern (applied before parsing)
    #[arg(long = "ignore-lines", help_heading = "Input Options")]
    pub ignore_lines: Option<String>,

    /// Run once before processing
    #[arg(long = "begin", help_heading = "Processing Options")]
    pub begin: Option<String>,

    /// Boolean filter expressions (can be repeated)
    #[arg(long = "filter", help_heading = "Processing Options")]
    pub filters: Vec<String>,

    /// Transform/process exec scripts (can be repeated)
    #[arg(short = 'e', long = "exec", help_heading = "Processing Options")]
    pub execs: Vec<String>,

    /// Execute script from file (can be repeated)
    #[arg(short = 'E', long = "exec-file", help_heading = "Processing Options")]
    pub exec_files: Vec<String>,

    /// Run once after processing
    #[arg(long = "end", help_heading = "Processing Options")]
    pub end: Option<String>,

    /// Error handling strategy
    #[arg(
        long = "on-error",
        value_enum,
        default_value = "print",
        help_heading = "Processing Options"
    )]
    pub on_error: ErrorStrategy,

    /// Disable field auto-injection
    #[arg(long = "no-inject", help_heading = "Processing Options")]
    pub no_inject_fields: bool,

    /// Prefix for injected variables
    #[arg(long = "inject-prefix", help_heading = "Processing Options")]
    pub inject_prefix: Option<String>,

    /// Include only events with these log levels (comma-separated, case-insensitive, e.g. debug,warn,error)
    #[arg(
        short = 'l',
        long = "levels",
        value_delimiter = ',',
        help_heading = "Filtering Options"
    )]
    pub levels: Vec<String>,

    /// Exclude events with these log levels (comma-separated, case-insensitive, higher priority than --levels)
    #[arg(
        short = 'L',
        long = "exclude-levels",
        value_delimiter = ',',
        help_heading = "Filtering Options"
    )]
    pub exclude_levels: Vec<String>,

    /// Output only specific fields (comma-separated)
    #[arg(
        short = 'k',
        long = "keys",
        value_delimiter = ',',
        help_heading = "Filtering Options"
    )]
    pub keys: Vec<String>,

    /// Exclude specific fields from output (comma-separated, higher priority than --keys)
    #[arg(
        short = 'K',
        long = "exclude-keys",
        value_delimiter = ',',
        help_heading = "Filtering Options"
    )]
    pub exclude_keys: Vec<String>,

    /// Output format
    #[arg(
        short = 'F',
        long = "output-format",
        value_enum,
        default_value = "default",
        help_heading = "Output Options"
    )]
    pub output_format: OutputFormat,

    /// Output only core fields (timestamp, level, message) plus any explicitly specified --keys
    #[arg(short = 'm', long = "core", help_heading = "Output Options")]
    pub core: bool,

    /// Output only field values (no keys), space-separated
    #[arg(short = 'b', long = "brief", help_heading = "Output Options")]
    pub brief: bool,

    /// Enable parallel processing for high-throughput analysis (batch-size=1000 by default)
    #[arg(long = "parallel", help_heading = "Performance Options")]
    pub parallel: bool,

    /// Number of worker threads for parallel processing
    #[arg(
        long = "threads",
        default_value_t = 0,
        help_heading = "Performance Options"
    )]
    pub threads: usize,

    /// Batch size for parallel processing (default: 1000)
    #[arg(long = "batch-size", help_heading = "Performance Options")]
    pub batch_size: Option<usize>,

    /// Batch timeout in milliseconds
    #[arg(
        long = "batch-timeout",
        default_value_t = 200,
        help_heading = "Performance Options"
    )]
    pub batch_timeout: u64,

    /// Disable ordered output (faster but may reorder results)
    #[arg(long = "unordered", help_heading = "Performance Options")]
    pub no_preserve_order: bool,

    /// Force colored output even when not on TTY (overrides NO_COLOR environment variable)
    #[arg(long = "force-color", help_heading = "Display Options")]
    pub force_color: bool,

    /// Disable colored output (takes precedence over --force-color)
    #[arg(long = "no-color", help_heading = "Display Options")]
    pub no_color: bool,

    /// Disable emoji prefixes in error messages
    #[arg(long = "no-emoji", help_heading = "Display Options")]
    pub no_emoji: bool,

    /// Show processing statistics
    #[arg(long = "stats", help_heading = "Display Options")]
    pub stats: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InputFormat {
    Jsonl,
    Line,
    Logfmt,
    Syslog,
    Csv,
    Apache,
    Nginx,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    Jsonl,
    #[default]
    Default,
    Logfmt,
    Csv,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ErrorStrategy {
    Skip,
    Abort,
    Print,
    Stub,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum FileOrder {
    None,
    Name,
    Mtime,
}

impl Cli {
    /// Extract filter and exec stages in the order they appeared on the command line
    fn get_ordered_script_stages(&self, matches: &ArgMatches) -> Result<Vec<ScriptStageType>> {
        let mut stages_with_indices = Vec::new();

        // Get filter stages with their indices
        if let Some(filter_indices) = matches.indices_of("filters") {
            let filter_values: Vec<&String> =
                matches.get_many::<String>("filters").unwrap().collect();
            for (pos, index) in filter_indices.enumerate() {
                stages_with_indices
                    .push((index, ScriptStageType::Filter(filter_values[pos].clone())));
            }
        }

        // Get exec stages with their indices
        if let Some(exec_indices) = matches.indices_of("execs") {
            let exec_values: Vec<&String> = matches.get_many::<String>("execs").unwrap().collect();
            for (pos, index) in exec_indices.enumerate() {
                stages_with_indices.push((index, ScriptStageType::Exec(exec_values[pos].clone())));
            }
        }

        // Get exec-file stages with their indices
        if let Some(exec_file_indices) = matches.indices_of("exec_files") {
            let exec_file_values: Vec<&String> =
                matches.get_many::<String>("exec_files").unwrap().collect();
            for (pos, index) in exec_file_indices.enumerate() {
                let file_path = &exec_file_values[pos];
                let script_content = std::fs::read_to_string(file_path).map_err(|e| {
                    anyhow::anyhow!("Failed to read exec file '{}': {}", file_path, e)
                })?;
                stages_with_indices.push((index, ScriptStageType::Exec(script_content)));
            }
        }

        // Sort by original command line position
        stages_with_indices.sort_by_key(|(index, _)| *index);

        // Extract just the stages
        Ok(stages_with_indices
            .into_iter()
            .map(|(_, stage)| stage)
            .collect())
    }
}

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

    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| {
        // Can't format with config yet, so use fallback
        stderr
            .writeln(&format!("kelora: Error: {}", e))
            .unwrap_or(());
        ExitCode::InvalidUsage.exit();
    });

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

    let final_stats = if use_parallel {
        // Get effective values from config for parallel mode
        let batch_size = config.effective_batch_size();
        let stats = run_parallel(&config, batch_size, &mut stdout, &mut stderr);

        // Print parallel stats if enabled (only if not terminated, will be handled later)
        if config.output.stats && !SHOULD_TERMINATE.load(Ordering::Relaxed) {
            if let Some(ref s) = stats {
                stderr
                    .writeln(&config.format_stats_message(&s.format_stats()))
                    .unwrap_or(());
            }
        }
        stats
    } else {
        run_sequential(&config, &mut stdout, &mut stderr);

        // Finish statistics collection and print stats if enabled (only if not terminated)
        if config.output.stats && !SHOULD_TERMINATE.load(Ordering::Relaxed) {
            stats_finish_processing();
            let stats = get_thread_stats();
            stderr
                .writeln(&config.format_stats_message(&stats.format_stats()))
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
                        .writeln(&config.format_stats_message(&stats.format_stats()))
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
                    .writeln(&config.format_stats_message(&stats.format_stats()))
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
fn run_parallel(
    config: &KeloraConfig,
    batch_size: usize,
    _stdout: &mut SafeStdout,
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

/// Run sequential processing mode
fn run_sequential(config: &KeloraConfig, stdout: &mut SafeStdout, stderr: &mut SafeStderr) {
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

    // Get input reader using pipeline builder
    let reader = match create_sequential_input_reader(config) {
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

    // Process lines using pipeline
    let mut line_num = 0;
    for line_result in reader.lines() {
        // Check for termination signal between lines
        if check_termination().is_err() {
            // Return early to allow graceful shutdown with stats
            return;
        }

        let line = line_result
            .map_err(|e| {
                stderr
                    .writeln(
                        &config.format_error_message(&format!("Failed to read input line: {}", e)),
                    )
                    .unwrap_or(());
                ExitCode::GeneralError.exit();
            })
            .unwrap();
        line_num += 1;

        // Count line read for stats
        if config.output.stats {
            stats_add_line_read();
        }

        // Apply ignore-lines filter if configured (early filtering before parsing)
        if let Some(ref ignore_regex) = config.input.ignore_lines {
            if ignore_regex.is_match(&line) {
                // Count filtered line for stats
                if config.output.stats {
                    stats_add_line_filtered();
                }
                continue;
            }
        }

        if line.trim().is_empty() {
            continue;
        }

        // Update metadata
        ctx.meta.line_number = Some(line_num);

        // Process line through pipeline
        match pipeline.process_line(line, &mut ctx) {
            Ok(results) => {
                // Count output lines for stats
                if config.output.stats && !results.is_empty() {
                    stats_add_line_output();
                } else if config.output.stats && results.is_empty() {
                    stats_add_line_filtered();
                }

                // Output all results (usually just one)
                for result in results {
                    stdout.writeln(&result).unwrap_or_else(|e| {
                        stderr
                            .writeln(&config.format_error_message(&format!("Output error: {}", e)))
                            .unwrap_or(());
                        ExitCode::GeneralError.exit();
                    });
                }
                stdout.flush().unwrap_or_else(|e| {
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
                    .writeln(&config.format_error_message(&format!(
                        "Pipeline error on line {}: {}",
                        line_num, e
                    )))
                    .unwrap_or(());
                match config.processing.on_error {
                    config::ErrorStrategy::Abort => ExitCode::GeneralError.exit(),
                    _ => continue, // Skip, Print, and Stub all continue processing
                }
            }
        }
    }

    // Flush any remaining chunks
    match pipeline.flush(&mut ctx) {
        Ok(results) => {
            for result in results {
                stdout.writeln(&result).unwrap_or_else(|e| {
                    stderr
                        .writeln(&config.format_error_message(&format!("Output error: {}", e)))
                        .unwrap_or(());
                    ExitCode::GeneralError.exit();
                });
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
