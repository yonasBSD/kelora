use anyhow::Result;
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser};
use std::io::BufRead;

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
mod tty;
mod unix;

use config::{KeloraConfig, ScriptStageType};
use parallel::{ParallelConfig, ParallelProcessor};
use pipeline::{create_pipeline_from_config, create_pipeline_builder_from_config, create_input_reader, create_sequential_input_reader};
use unix::{ExitCode, SignalHandler, SafeStdout, SafeStderr, ProcessCleanup, check_termination};

#[derive(Parser)]
#[command(name = "kelora")]
#[command(about = "A command-line log analysis tool with embedded Rhai scripting")]
#[command(long_about = "A command-line log analysis tool with embedded Rhai scripting\n\nMODES:\n  (default)   Sequential processing - best for streaming/interactive use\n  --parallel  Parallel processing - best for high-throughput batch analysis")]
#[command(version = "0.2.0")]
#[command(author = "Dirk Loss <mail@dirk-loss.de>")]
pub struct Cli {
    /// Input files (stdin if not specified)
    pub files: Vec<String>,

    /// Input format
    #[arg(short = 'f', long = "format", value_enum, default_value = "line")]
    pub format: InputFormat,

    /// Output format
    #[arg(
        short = 'F',
        long = "output-format",
        value_enum,
        default_value = "default"
    )]
    pub output_format: OutputFormat,

    /// Run once before processing
    #[arg(long = "begin")]
    pub begin: Option<String>,

    /// Boolean filter expressions (can be repeated)
    #[arg(long = "filter")]
    pub filters: Vec<String>,

    /// Transform/process exec scripts (can be repeated)
    #[arg(short = 'e', long = "exec")]
    pub execs: Vec<String>,

    /// Run once after processing
    #[arg(long = "end")]
    pub end: Option<String>,

    /// Disable field auto-injection
    #[arg(long = "no-inject")]
    pub no_inject_fields: bool,

    /// Prefix for injected variables
    #[arg(long = "inject-prefix")]
    pub inject_prefix: Option<String>,

    /// Error handling strategy
    #[arg(long = "on-error", value_enum, default_value = "print")]
    pub on_error: ErrorStrategy,

    /// Output only specific fields (comma-separated)
    #[arg(long = "keys", value_delimiter = ',')]
    pub keys: Vec<String>,

    /// Exclude specific fields from output (comma-separated, higher priority than --keys)
    #[arg(short = 'K', long = "exclude-keys", value_delimiter = ',')]
    pub exclude_keys: Vec<String>,

    /// Output only core fields (timestamp, level, message) plus any explicitly specified --keys
    #[arg(short = 'm', long = "core")]
    pub core: bool,

    /// Output only field values (no keys), space-separated
    #[arg(short = 'b', long = "brief")]
    pub brief: bool,

    /// Number of worker threads for parallel processing
    #[arg(long = "threads", default_value_t = 0)]
    pub threads: usize,

    /// Batch size for parallel processing (default: 1000)
    #[arg(long = "batch-size")]
    pub batch_size: Option<usize>,

    /// Batch timeout in milliseconds
    #[arg(long = "batch-timeout", default_value_t = 200)]
    pub batch_timeout: u64,

    /// Disable ordered output (faster but may reorder results)
    #[arg(long = "unordered")]
    pub no_preserve_order: bool,

    /// Enable parallel processing for high-throughput analysis (batch-size=1000 by default)
    #[arg(long = "parallel")]
    pub parallel: bool,

    /// File processing order: none (CLI order), name (alphabetical), mtime (modification time, oldest first)
    #[arg(long = "file-order", value_enum, default_value = "none")]
    pub file_order: FileOrder,

    /// Control colored output (auto/always/never)
    #[arg(long = "color", value_enum, default_value = "auto")]
    pub color: ColorMode,

    /// Include only events with these log levels (comma-separated, case-insensitive, e.g. debug,warn,error)
    #[arg(short = 'l', long = "levels", value_delimiter = ',')]
    pub levels: Vec<String>,

    /// Exclude events with these log levels (comma-separated, case-insensitive, higher priority than --levels)
    #[arg(short = 'L', long = "exclude-levels", value_delimiter = ',')]
    pub exclude_levels: Vec<String>,
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

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl Cli {
    /// Extract filter and exec stages in the order they appeared on the command line
    fn get_ordered_script_stages(&self, matches: &ArgMatches) -> Vec<ScriptStageType> {
        let mut stages_with_indices = Vec::new();

        // Get filter stages with their indices
        if let Some(filter_indices) = matches.indices_of("filters") {
            let filter_values: Vec<&String> = matches.get_many::<String>("filters").unwrap().collect();
            for (pos, index) in filter_indices.enumerate() {
                stages_with_indices.push((index, ScriptStageType::Filter(filter_values[pos].clone())));
            }
        }

        // Get exec stages with their indices
        if let Some(exec_indices) = matches.indices_of("execs") {
            let exec_values: Vec<&String> = matches.get_many::<String>("execs").unwrap().collect();
            for (pos, index) in exec_indices.enumerate() {
                stages_with_indices.push((index, ScriptStageType::Exec(exec_values[pos].clone())));
            }
        }

        // Sort by original command line position
        stages_with_indices.sort_by_key(|(index, _)| *index);

        // Extract just the stages
        stages_with_indices
            .into_iter()
            .map(|(_, stage)| stage)
            .collect()
    }
}

fn main() -> Result<()> {
    // Initialize signal handling early
    let _signal_handler = SignalHandler::new().map_err(|e| {
        eprintln!("Failed to initialize signal handling: {}", e);
        ExitCode::GeneralError.exit();
    }).unwrap();

    // Initialize process cleanup
    let _cleanup = ProcessCleanup::new();

    // Initialize safe I/O wrappers
    let mut stdout = SafeStdout::new();
    let mut stderr = SafeStderr::new();

    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| {
        stderr.writeln(&format!("Error: {}", e)).unwrap_or(());
        ExitCode::InvalidUsage.exit();
    });

    // Extract ordered script stages
    let ordered_stages = cli.get_ordered_script_stages(&matches);

    // Validate arguments early
    if let Err(e) = validate_cli_args(&cli) {
        stderr.writeln(&format!("Error: {}", e)).unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    // Create configuration from CLI and set stages
    let mut config = KeloraConfig::from_cli(&cli);
    config.processing.stages = ordered_stages;

    // Determine processing mode using config
    let use_parallel = config.should_use_parallel();

    if use_parallel {
        // Get effective values from config for parallel mode
        let batch_size = config.effective_batch_size();
        run_parallel(&config, batch_size, &mut stdout, &mut stderr);
    } else {
        run_sequential(&config, &mut stdout, &mut stderr);
    }

    // Clean shutdown
    
    ExitCode::Success.exit();
}

/// Run parallel processing mode
/// Note: stdout parameter is currently unused as ParallelProcessor creates its own SafeStdout,
/// but kept for consistency with run_sequential and future flexibility
fn run_parallel(config: &KeloraConfig, batch_size: usize, _stdout: &mut SafeStdout, stderr: &mut SafeStderr) {
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
    let (_pipeline, begin_stage, end_stage, mut ctx) = match pipeline_builder.clone().build(config.processing.stages.clone()) {
        Ok(pipeline_components) => pipeline_components,
        Err(e) => {
            stderr.writeln(&format!("Failed to create pipeline: {}", e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };
    
    // Execute begin stage sequentially if provided
    execute_begin_stage(&begin_stage, &mut ctx, stderr);

    // Get reader using pipeline builder
    let reader = match create_input_reader(config) {
        Ok(r) => r,
        Err(e) => {
            stderr.writeln(&format!("Failed to create input reader: {}", e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };

    // Process stages in parallel
    if let Err(e) = processor.process_with_pipeline(reader, pipeline_builder, config.processing.stages.clone()) {
        stderr.writeln(&format!("Parallel processing error: {}", e)).unwrap_or(());
        ExitCode::GeneralError.exit();
    }

    // Merge the parallel tracked state with our pipeline context
    let parallel_tracked = processor.get_final_tracked_state();
    for (key, dynamic_value) in parallel_tracked {
        ctx.tracker.insert(key, dynamic_value);
    }

    // Execute end stage sequentially with merged state
    execute_end_stage(&end_stage, &ctx, stderr);
}

/// Run sequential processing mode
fn run_sequential(config: &KeloraConfig, stdout: &mut SafeStdout, stderr: &mut SafeStderr) {
    // Sequential processing mode using new pipeline architecture
    let (mut pipeline, begin_stage, end_stage, mut ctx) = match create_pipeline_from_config(config) {
        Ok(pipeline_components) => pipeline_components,
        Err(e) => {
            stderr.writeln(&format!("Failed to create pipeline: {}", e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };

    // Execute begin stage
    execute_begin_stage(&begin_stage, &mut ctx, stderr);

    // Get input reader using pipeline builder
    let reader = match create_sequential_input_reader(config) {
        Ok(r) => r,
        Err(e) => {
            stderr.writeln(&format!("Failed to create input reader: {}", e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };

    // Process lines using pipeline
    let mut line_num = 0;
    for line_result in reader.lines() {
        // Check for termination signal between lines
        check_termination().unwrap_or_else(|_| {
            ExitCode::SignalInt.exit();
        });

        let line = line_result.map_err(|e| {
            stderr.writeln(&format!("Failed to read input line: {}", e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }).unwrap();
        line_num += 1;

        if line.trim().is_empty() {
            continue;
        }

        // Update metadata
        ctx.meta.line_number = Some(line_num);
        
        // Process line through pipeline
        match pipeline.process_line(line, &mut ctx) {
            Ok(results) => {
                // Output all results (usually just one)
                for result in results {
                    stdout.writeln(&result).unwrap_or_else(|e| {
                        stderr.writeln(&format!("Output error: {}", e)).unwrap_or(());
                        ExitCode::GeneralError.exit();
                    });
                }
                stdout.flush().unwrap_or_else(|e| {
                    stderr.writeln(&format!("Flush error: {}", e)).unwrap_or(());
                    ExitCode::GeneralError.exit();
                });
            }
            Err(e) => {
                stderr.writeln(&format!("Pipeline error on line {}: {}", line_num, e)).unwrap_or(());
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
                    stderr.writeln(&format!("Output error: {}", e)).unwrap_or(());
                    ExitCode::GeneralError.exit();
                });
            }
        }
        Err(e) => {
            stderr.writeln(&format!("Pipeline flush error: {}", e)).unwrap_or(());
        }
    }

    // Execute end stage
    execute_end_stage(&end_stage, &ctx, stderr);
}


/// Execute begin stage with shared error handling
fn execute_begin_stage(begin_stage: &pipeline::BeginStage, ctx: &mut pipeline::PipelineContext, stderr: &mut SafeStderr) {
    if let Err(e) = begin_stage.execute(ctx) {
        stderr.writeln(&format!("Begin stage error: {}", e)).unwrap_or(());
        ExitCode::GeneralError.exit();
    }
}

/// Execute end stage with shared error handling
fn execute_end_stage(end_stage: &pipeline::EndStage, ctx: &pipeline::PipelineContext, stderr: &mut SafeStderr) {
    if let Err(e) = end_stage.execute(ctx) {
        stderr.writeln(&format!("End stage error: {}", e)).unwrap_or(());
        ExitCode::GeneralError.exit();
    }
}



/// Validate CLI arguments for early error detection
fn validate_cli_args(cli: &Cli) -> Result<()> {
    // Check if files exist (if specified)
    for file_path in &cli.files {
        if !std::path::Path::new(file_path).exists() {
            return Err(anyhow::anyhow!("File not found: {}", file_path));
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


