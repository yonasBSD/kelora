// Core library for Kelora log analysis tool

pub use config::{KeloraConfig, ScriptStageType};

// CLI types - these will eventually be moved to a separate module
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InputFormat {
    Jsonl,
    Line,
    Logfmt,
    Syslog,
    Cef,
    Csv,
    Tsv,
    Csvnh,
    Tsvnh,
    Apache,
    Nginx,
    Cols,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    Jsonl,
    #[default]
    Default,
    Logfmt,
    Csv,
    Tsv,
    Csvnh,
    Tsvnh,
    Hide,
    Null,
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

use anyhow::Result;
use clap::{ArgMatches, Parser};

// Temporary: CLI structure exposed for config - will be refactored in later iterations
#[derive(Parser)]
#[command(name = "kelora")]
#[command(about = "A command-line log analysis tool with embedded Rhai scripting")]
#[command(
    long_about = "A command-line log analysis tool with embedded Rhai scripting\n\nMODES:\n  (default)   Sequential processing - best for streaming/interactive use\n  --parallel  Parallel processing - best for high-throughput batch analysis"
)]
#[command(version = "0.2.0")]
#[command(author = "Dirk Loss <mail@dirk-loss.de>")]
pub struct Cli {
    pub files: Vec<String>,
    #[arg(short = 'f', long = "format", value_enum, default_value = "line", help_heading = "Input Options")]
    pub format: InputFormat,
    #[arg(long = "file-order", value_enum, default_value = "none", help_heading = "Input Options")]
    pub file_order: FileOrder,
    #[arg(long = "skip-lines", help_heading = "Input Options")]
    pub skip_lines: Option<usize>,
    #[arg(long = "ignore-lines", help_heading = "Input Options")]
    pub ignore_lines: Option<String>,
    #[arg(long = "ts-field", help_heading = "Input Options")]
    pub ts_field: Option<String>,
    #[arg(short = 'M', long = "multiline", help_heading = "Input Options")]
    pub multiline: Option<String>,
    #[arg(long = "begin", help_heading = "Processing Options")]
    pub begin: Option<String>,
    #[arg(long = "filter", help_heading = "Processing Options")]
    pub filters: Vec<String>,
    #[arg(short = 'e', long = "exec", help_heading = "Processing Options")]
    pub execs: Vec<String>,
    #[arg(short = 'E', long = "exec-file", help_heading = "Processing Options")]
    pub exec_files: Vec<String>,
    #[arg(long = "end", help_heading = "Processing Options")]
    pub end: Option<String>,
    #[arg(long = "window", help_heading = "Processing Options")]
    pub window_size: Option<usize>,
    #[arg(long = "on-error", value_enum, default_value = "print", help_heading = "Processing Options")]
    pub on_error: ErrorStrategy,
    #[arg(long = "no-inject", help_heading = "Processing Options")]
    pub no_inject_fields: bool,
    #[arg(long = "inject-prefix", help_heading = "Processing Options")]
    pub inject_prefix: Option<String>,
    #[arg(short = 'l', long = "levels", value_delimiter = ',', help_heading = "Filtering Options")]
    pub levels: Vec<String>,
    #[arg(short = 'L', long = "exclude-levels", value_delimiter = ',', help_heading = "Filtering Options")]
    pub exclude_levels: Vec<String>,
    #[arg(short = 'k', long = "keys", value_delimiter = ',', help_heading = "Filtering Options")]
    pub keys: Vec<String>,
    #[arg(short = 'K', long = "exclude-keys", value_delimiter = ',', help_heading = "Filtering Options")]
    pub exclude_keys: Vec<String>,
    #[arg(long = "since", help_heading = "Filtering Options")]
    pub since: Option<String>,
    #[arg(long = "until", help_heading = "Filtering Options")]
    pub until: Option<String>,
    #[arg(short = 'F', long = "output-format", value_enum, default_value = "default", help_heading = "Output Options")]
    pub output_format: OutputFormat,
    #[arg(short = 'm', long = "core", help_heading = "Output Options")]
    pub core: bool,
    #[arg(short = 'b', long = "brief", help_heading = "Output Options")]
    pub brief: bool,
    #[arg(short = 'o', long = "output-file", help_heading = "Output Options")]
    pub output_file: Option<String>,
    #[arg(long = "parallel", help_heading = "Performance Options")]
    pub parallel: bool,
    #[arg(long = "threads", default_value_t = 0, help_heading = "Performance Options")]
    pub threads: usize,
    #[arg(long = "batch-size", help_heading = "Performance Options")]
    pub batch_size: Option<usize>,
    #[arg(long = "batch-timeout", default_value_t = 200, help_heading = "Performance Options")]
    pub batch_timeout: u64,
    #[arg(long = "unordered", help_heading = "Performance Options")]
    pub no_preserve_order: bool,
    #[arg(long = "force-color", help_heading = "Display Options")]
    pub force_color: bool,
    #[arg(long = "no-color", help_heading = "Display Options")]
    pub no_color: bool,
    #[arg(long = "no-emoji", help_heading = "Display Options")]
    pub no_emoji: bool,
    #[arg(long = "summary", help_heading = "Display Options")]
    pub summary: bool,
    #[arg(short = 's', long = "stats", help_heading = "Display Options")]
    pub stats: bool,
    #[arg(short = 'S', long = "stats-only", help_heading = "Display Options")]
    pub stats_only: bool,
    #[arg(short = 'a', long = "alias", help_heading = "Configuration Options")]
    pub alias: Vec<String>,
    #[arg(long = "show-config", help_heading = "Configuration Options")]
    pub show_config: bool,
    #[arg(long = "ignore-config", help_heading = "Configuration Options")]
    pub ignore_config: bool,
}

impl Cli {
    /// Extract filter and exec stages in the order they appeared on the command line
    pub fn get_ordered_script_stages(&self, matches: &ArgMatches) -> Result<Vec<ScriptStageType>> {
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
use parallel::{ParallelConfig, ParallelProcessor};
use pipeline::{
    create_input_reader, create_pipeline_builder_from_config, create_pipeline_from_config,
};
use stats::{
    get_thread_stats, stats_add_error, stats_add_line_filtered, stats_add_line_output,
    stats_add_line_read, stats_finish_processing, stats_start_timer, ProcessingStats,
};
use unix::{check_termination, ExitCode, SafeStderr, SHOULD_TERMINATE};
use std::io::{self, BufRead, Write};
use std::sync::atomic::Ordering;

/// Result of pipeline processing
#[derive(Debug)]
pub struct PipelineResult {
    pub stats: Option<ProcessingStats>,
    pub success: bool,
}

/// Core pipeline processing function
/// This is the main entry point for processing log data with the given configuration
pub fn run_pipeline<W: Write + Send + 'static>(
    config: &KeloraConfig,
    output: W,
) -> Result<PipelineResult> {
    // Start statistics collection if enabled
    if config.output.stats {
        stats_start_timer();
    }

    let use_parallel = config.should_use_parallel();
    
    let final_stats = if use_parallel {
        run_pipeline_parallel(config, output)?
    } else {
        let mut output = output;
        run_pipeline_sequential(config, &mut output)?;
        if config.output.stats {
            stats_finish_processing();
            Some(get_thread_stats())
        } else {
            None
        }
    };

    Ok(PipelineResult {
        stats: final_stats,
        success: !SHOULD_TERMINATE.load(Ordering::Relaxed),
    })
}

/// Run pipeline in parallel mode
fn run_pipeline_parallel<W: Write + Send + 'static>(
    config: &KeloraConfig,
    output: W,
) -> Result<Option<ProcessingStats>> {
    let batch_size = config.effective_batch_size();
    
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
    )?;

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
    if let Err(e) = end_stage.execute(&ctx) {
        return Err(anyhow::anyhow!("End stage error: {}", e));
    }

    // Get final stats if enabled
    if config.output.stats {
        Ok(Some(processor.get_final_stats()))
    } else {
        Ok(None)
    }
}

/// Run pipeline in sequential mode
fn run_pipeline_sequential<W: Write>(
    config: &KeloraConfig,
    output: &mut W,
) -> Result<()> {
    let (mut pipeline, begin_stage, end_stage, mut ctx) = create_pipeline_from_config(config)?;

    // Execute begin stage
    if let Err(e) = begin_stage.execute(&mut ctx) {
        return Err(anyhow::anyhow!("Begin stage error: {}", e));
    }

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
                return Ok(());
            }

            process_line_sequential(
                line_result,
                &mut line_num,
                &mut skipped_lines,
                &mut pipeline,
                &mut ctx,
                config,
                output,
                None,
                &mut current_csv_headers,
                &mut last_filename,
            )?;
        }
    } else {
        // File processing - with filename tracking
        let sorted_files = pipeline::builders::sort_files(&config.input.files, &config.input.file_order)?;
        let mut multi_reader = crate::readers::MultiFileReader::new(sorted_files)?;

        let mut line_buf = String::new();
        loop {
            // Check for termination signal between lines
            if check_termination().is_err() {
                return Ok(());
            }

            line_buf.clear();
            let bytes_read = match multi_reader.read_line(&mut line_buf) {
                Ok(0) => break, // EOF
                Ok(n) => n,
                Err(e) => {
                    let line_result = Err(e);
                    let current_filename = multi_reader.current_filename().map(|s| s.to_string());
                    process_line_sequential(
                        line_result,
                        &mut line_num,
                        &mut skipped_lines,
                        &mut pipeline,
                        &mut ctx,
                        config,
                        output,
                        current_filename,
                        &mut current_csv_headers,
                        &mut last_filename,
                    )?;
                    continue;
                }
            };

            if bytes_read > 0 {
                let current_filename = multi_reader.current_filename().map(|s| s.to_string());
                process_line_sequential(
                    Ok(line_buf.clone()),
                    &mut line_num,
                    &mut skipped_lines,
                    &mut pipeline,
                    &mut ctx,
                    config,
                    output,
                    current_filename,
                    &mut current_csv_headers,
                    &mut last_filename,
                )?;
            }
        }
    }

    // Flush any remaining chunks
    let results = pipeline.flush(&mut ctx)?;
    for result in results {
        if !result.is_empty() {
            writeln!(output, "{}", result)?;
        }
    }

    // Execute end stage
    if let Err(e) = end_stage.execute(&ctx) {
        return Err(anyhow::anyhow!("End stage error: {}", e));
    }

    Ok(())
}

/// Process a single line in sequential mode (simplified from main.rs)
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
) -> Result<()> {
    let line = line_result?;
    *line_num += 1;

    // Count line read for stats
    if config.output.stats {
        stats_add_line_read();
    }

    // Skip the first N lines if configured
    if *skipped_lines < config.input.skip_lines {
        *skipped_lines += 1;
        if config.output.stats {
            stats_add_line_filtered();
        }
        return Ok(());
    }

    // Apply ignore-lines filter if configured
    if let Some(ref ignore_regex) = config.input.ignore_lines {
        if ignore_regex.is_match(&line) {
            if config.output.stats {
                stats_add_line_filtered();
            }
            return Ok(());
        }
    }

    if line.trim().is_empty() {
        // Only skip empty lines for structured formats, not for line format
        if !matches!(config.input.format, config::InputFormat::Line) {
            return Ok(());
        }
    }

    // Note: Simplified CSV handling - full logic would be needed for production
    if matches!(
        config.input.format,
        config::InputFormat::Csv | config::InputFormat::Tsv | config::InputFormat::Csvnh | config::InputFormat::Tsvnh
    ) && (current_filename != *last_filename || (current_filename.is_none() && current_csv_headers.is_none())) {
        *last_filename = current_filename.clone();
        // TODO: Add full CSV header reinitialization logic
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

            // Output all results
            for result in results {
                if !result.is_empty() {
                    writeln!(output, "{}", result)?;
                }
            }
        }
        Err(e) => {
            // Count errors for stats
            if config.output.stats {
                stats_add_error();
            }
            
            // Handle error based on strategy
            if let config::ErrorStrategy::Abort = config.processing.on_error {
                return Err(e);
            }
            // For other strategies, we continue processing
        }
    }

    Ok(())
}