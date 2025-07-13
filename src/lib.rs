// Core library for Kelora log analysis tool

pub use config::{KeloraConfig, MultilineConfig, ScriptStageType, TimestampFilterConfig};

/// Core pipeline configuration - contains only what's needed for processing
/// Separated from CLI-specific concerns like colors, stats output, etc.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Input configuration
    pub input: PipelineInputConfig,
    /// Processing configuration  
    pub processing: PipelineProcessingConfig,
    /// Performance configuration
    pub performance: PipelinePerformanceConfig,
    /// Output format (for data transformation, not display)
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone)]
pub struct PipelineInputConfig {
    pub files: Vec<String>,
    pub format: InputFormat,
    pub file_order: FileOrder,
    pub skip_lines: usize,
    pub ignore_lines: Option<regex::Regex>,
    pub multiline: Option<config::MultilineConfig>,
    pub ts_field: Option<String>,
    pub ts_format: Option<String>,
    pub default_timezone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineProcessingConfig {
    pub begin: Option<String>,
    pub stages: Vec<ScriptStageType>,
    pub end: Option<String>,
    pub no_inject_fields: bool,
    pub inject_prefix: Option<String>,
    pub on_error: ErrorStrategy,
    pub error_report: config::ErrorReportConfig,
    pub levels: Vec<String>,
    pub exclude_levels: Vec<String>,
    pub window_size: usize,
    pub timestamp_filter: Option<config::TimestampFilterConfig>,
    pub take_limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct PipelinePerformanceConfig {
    pub parallel: bool,
    pub threads: usize,
    pub batch_size: Option<usize>,
    pub batch_timeout: u64,
    pub no_preserve_order: bool,
}

impl PipelineConfig {
    /// Convert from KeloraConfig to PipelineConfig
    pub fn from_kelora_config(config: &KeloraConfig) -> Self {
        Self {
            input: PipelineInputConfig {
                files: config.input.files.clone(),
                format: config.input.format.clone().into(),
                file_order: config.input.file_order.clone().into(),
                skip_lines: config.input.skip_lines,
                ignore_lines: config.input.ignore_lines.clone(),
                multiline: config.input.multiline.clone(),
                ts_field: config.input.ts_field.clone(),
                ts_format: config.input.ts_format.clone(),
                default_timezone: config.input.default_timezone.clone(),
            },
            processing: PipelineProcessingConfig {
                begin: config.processing.begin.clone(),
                stages: config.processing.stages.clone(),
                end: config.processing.end.clone(),
                no_inject_fields: config.processing.no_inject_fields,
                inject_prefix: config.processing.inject_prefix.clone(),
                on_error: config.processing.on_error.clone().into(),
                error_report: config.processing.error_report.clone(),
                levels: config.processing.levels.clone(),
                exclude_levels: config.processing.exclude_levels.clone(),
                window_size: config.processing.window_size,
                timestamp_filter: config.processing.timestamp_filter.clone(),
                take_limit: config.processing.take_limit,
            },
            performance: PipelinePerformanceConfig {
                parallel: config.performance.parallel,
                threads: config.performance.threads,
                batch_size: config.performance.batch_size,
                batch_timeout: config.performance.batch_timeout,
                no_preserve_order: config.performance.no_preserve_order,
            },
            output_format: config.output.format.clone().into(),
        }
    }

    /// Check if parallel processing should be used
    pub fn should_use_parallel(&self) -> bool {
        self.performance.parallel
            || self.performance.threads > 0
            || self.performance.batch_size.is_some()
    }

    /// Get effective batch size with defaults
    pub fn effective_batch_size(&self) -> usize {
        self.performance.batch_size.unwrap_or(1000)
    }

    /// Get effective thread count with defaults
    pub fn effective_threads(&self) -> usize {
        if self.performance.threads == 0 {
            num_cpus::get()
        } else {
            self.performance.threads
        }
    }
}

// CLI module for command-line interface types
pub mod cli;

mod colors;
mod config;
mod config_file;
mod decompression;
mod engine;
mod error_handling;
mod event;
mod formatters;
mod parallel;
mod parsers;
pub mod pipeline;
mod readers;
mod rhai_functions;
mod stats;
mod timestamp;
mod tty;
mod unix;

use anyhow::Result;

// Re-export CLI types for convenience (they live in cli module now)
pub use cli::{Cli, ErrorStrategy, FileOrder, InputFormat, OutputFormat};

use parallel::{ParallelConfig, ParallelProcessor};
use pipeline::{
    create_input_reader, create_pipeline_builder_from_config, create_pipeline_from_config,
};
use stats::{
    get_thread_stats, stats_add_error, stats_add_line_filtered, stats_add_line_output,
    stats_add_line_read, stats_finish_processing, stats_start_timer, ProcessingStats,
};
use std::io::{self, BufRead, Write};
use std::sync::atomic::Ordering;
use unix::{check_termination, SHOULD_TERMINATE};

/// Result of pipeline processing
#[derive(Debug)]
pub struct PipelineResult {
    pub stats: Option<ProcessingStats>,
    pub success: bool,
}

/// Core pipeline processing function (new API using PipelineConfig)
/// This is the preferred entry point for processing log data with clean configuration
pub fn run_pipeline<W: Write + Send + 'static>(
    config: &PipelineConfig,
    output: W,
    collect_stats: bool,
) -> Result<PipelineResult> {
    // Start statistics collection if enabled
    if collect_stats {
        stats_start_timer();
    }

    let use_parallel = config.should_use_parallel();

    let final_stats = if use_parallel {
        run_pipeline_parallel_with_config(config, output)?
    } else {
        let mut output = output;
        run_pipeline_sequential_with_config(config, &mut output)?;
        if collect_stats {
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

/// Legacy API for backward compatibility - will be removed in future increments
pub fn run_pipeline_with_kelora_config<W: Write + Send + 'static>(
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

/// Run pipeline in parallel mode using PipelineConfig
pub fn run_pipeline_parallel_with_config<W: Write + Send + 'static>(
    config: &PipelineConfig,
    output: W,
) -> Result<Option<ProcessingStats>> {
    // Convert to KeloraConfig temporarily - will be removed when all core functions are updated
    let kelora_config = KeloraConfig {
        input: config::InputConfig {
            files: config.input.files.clone(),
            format: config.input.format.clone().into(),
            file_order: config.input.file_order.clone().into(),
            skip_lines: config.input.skip_lines,
            ignore_lines: config.input.ignore_lines.clone(),
            multiline: config.input.multiline.clone(),
            ts_field: config.input.ts_field.clone(),
            ts_format: config.input.ts_format.clone(),
            default_timezone: config.input.default_timezone.clone(),
        },
        output: config::OutputConfig {
            format: config.output_format.clone().into(),
            keys: Vec::new(),
            exclude_keys: Vec::new(),
            core: false,
            brief: false,
            color: config::ColorMode::Auto,
            no_emoji: false,
            no_section_headers: false,
            stats: false, // Stats handled at higher level
            metrics: false,
            metrics_file: None,
            timestamp_formatting: config::TimestampFormatConfig::default(),
        },
        processing: config::ProcessingConfig {
            begin: config.processing.begin.clone(),
            stages: config.processing.stages.clone(),
            end: config.processing.end.clone(),
            no_inject_fields: config.processing.no_inject_fields,
            inject_prefix: config.processing.inject_prefix.clone(),
            on_error: config.processing.on_error.clone().into(),
            error_report: config.processing.error_report.clone(),
            levels: config.processing.levels.clone(),
            exclude_levels: config.processing.exclude_levels.clone(),
            window_size: config.processing.window_size,
            timestamp_filter: config.processing.timestamp_filter.clone(),
            take_limit: config.processing.take_limit,
        },
        performance: config::PerformanceConfig {
            parallel: config.performance.parallel,
            threads: config.performance.threads,
            batch_size: config.performance.batch_size,
            batch_timeout: config.performance.batch_timeout,
            no_preserve_order: config.performance.no_preserve_order,
        },
    };

    run_pipeline_parallel(&kelora_config, output)
}

/// Run pipeline in parallel mode using KeloraConfig (legacy)
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

    let processor = ParallelProcessor::new(parallel_config)
        .with_take_limit(config.processing.take_limit);

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

    // Generate error summary before merging if in summary mode
    if matches!(
        config.processing.error_report.style,
        crate::config::ErrorReportStyle::Summary
    ) {
        if let Some(summary) =
            crate::rhai_functions::tracking::extract_error_summary(&parallel_tracked)
        {
            eprintln!("Error Summary:\n{}", summary);
        }
    }

    // Write error summary to file if configured
    if let Some(ref file_path) = config.processing.error_report.file {
        crate::rhai_functions::tracking::write_error_summary_to_file(&parallel_tracked, file_path)
            .unwrap_or_else(|e| eprintln!("Failed to write error summary to file: {}", e));
    }

    // Extract internal stats from tracking system before merging (if stats enabled)
    if config.output.stats {
        processor
            .extract_final_stats_from_tracking(&parallel_tracked)
            .unwrap_or(());
    }

    // Filter out stats and errors from user-visible context and merge the rest
    for (key, dynamic_value) in parallel_tracked {
        if !key.starts_with("__internal_")
            && !key.starts_with("__kelora_stats_")
            && !key.starts_with("__op___kelora_stats_")
            && !key.starts_with("__kelora_error_")
            && !key.starts_with("__op___kelora_error_")
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

/// Run pipeline in sequential mode using PipelineConfig
pub fn run_pipeline_sequential_with_config<W: Write>(
    config: &PipelineConfig,
    output: &mut W,
) -> Result<()> {
    // Convert to KeloraConfig temporarily - will be removed when all core functions are updated
    let kelora_config = KeloraConfig {
        input: config::InputConfig {
            files: config.input.files.clone(),
            format: config.input.format.clone().into(),
            file_order: config.input.file_order.clone().into(),
            skip_lines: config.input.skip_lines,
            ignore_lines: config.input.ignore_lines.clone(),
            multiline: config.input.multiline.clone(),
            ts_field: config.input.ts_field.clone(),
            ts_format: config.input.ts_format.clone(),
            default_timezone: config.input.default_timezone.clone(),
        },
        output: config::OutputConfig {
            format: config.output_format.clone().into(),
            keys: Vec::new(),
            exclude_keys: Vec::new(),
            core: false,
            brief: false,
            color: config::ColorMode::Auto,
            no_emoji: false,
            no_section_headers: false,
            stats: false, // Stats handled at higher level
            metrics: false,
            metrics_file: None,
            timestamp_formatting: config::TimestampFormatConfig::default(),
        },
        processing: config::ProcessingConfig {
            begin: config.processing.begin.clone(),
            stages: config.processing.stages.clone(),
            end: config.processing.end.clone(),
            no_inject_fields: config.processing.no_inject_fields,
            inject_prefix: config.processing.inject_prefix.clone(),
            on_error: config.processing.on_error.clone().into(),
            error_report: config.processing.error_report.clone(),
            levels: config.processing.levels.clone(),
            exclude_levels: config.processing.exclude_levels.clone(),
            window_size: config.processing.window_size,
            timestamp_filter: config.processing.timestamp_filter.clone(),
            take_limit: config.processing.take_limit,
        },
        performance: config::PerformanceConfig {
            parallel: config.performance.parallel,
            threads: config.performance.threads,
            batch_size: config.performance.batch_size,
            batch_timeout: config.performance.batch_timeout,
            no_preserve_order: config.performance.no_preserve_order,
        },
    };

    run_pipeline_sequential(&kelora_config, output)
}

/// Run pipeline in sequential mode using KeloraConfig (legacy)
pub fn run_pipeline_sequential<W: Write>(config: &KeloraConfig, output: &mut W) -> Result<()> {
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
        let sorted_files =
            pipeline::builders::sort_files(&config.input.files, &config.input.file_order)?;
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

    // Merge thread-local tracking state (including errors) into context for sequential mode
    crate::rhai_functions::tracking::merge_thread_tracking_to_context(&mut ctx);

    // Generate error summary if in summary mode
    if matches!(
        config.processing.error_report.style,
        crate::config::ErrorReportStyle::Summary
    ) {
        if let Some(summary) = crate::rhai_functions::tracking::extract_error_summary(&ctx.tracker)
        {
            eprintln!("Error Summary:\n{}", summary);
        }
    }

    // Write error summary to file if configured
    if let Some(ref file_path) = config.processing.error_report.file {
        crate::rhai_functions::tracking::write_error_summary_to_file(&ctx.tracker, file_path)
            .unwrap_or_else(|e| eprintln!("Failed to write error summary to file: {}", e));
    }

    Ok(())
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
) -> Result<()> {
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
        return Ok(());
    }

    // Apply ignore-lines filter if configured (early filtering before parsing)
    if let Some(ref ignore_regex) = config.input.ignore_lines {
        if ignore_regex.is_match(&line) {
            // Count filtered line for stats
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
        let was_consumed = temp_parser.initialize_headers_from_line(&line)?;

        // Get the initialized headers
        let headers = temp_parser.get_headers();
        *current_csv_headers = Some(headers.clone());
        *last_filename = current_filename.clone();

        // Rebuild the pipeline with new headers
        let mut pipeline_builder = create_pipeline_builder_from_config(config);
        pipeline_builder = pipeline_builder.with_csv_headers(headers);

        let (new_pipeline, _new_begin_stage, _new_end_stage, new_ctx) =
            pipeline_builder.build(config.processing.stages.clone())?;

        *pipeline = new_pipeline;
        // Keep the existing context's tracking state but update the Rhai engine
        ctx.rhai = new_ctx.rhai;

        // If the first line was consumed as a header, don't process it as data
        if was_consumed {
            return Ok(());
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
