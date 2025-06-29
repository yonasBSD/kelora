use anyhow::Result;
use clap::Parser;
use std::io::{self, BufRead, BufReader, Read};

mod colors;
mod config;
mod engine;
mod event;
mod formatters;
mod parallel;
mod parsers;
mod pipeline;
mod tty;
mod unix;

use config::KeloraConfig;
use parallel::{ParallelConfig, ParallelProcessor};
use pipeline::{create_pipeline_from_config, create_pipeline_builder_from_config};
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
    #[arg(long = "on-error", value_enum, default_value = "emit-errors")]
    pub on_error: ErrorStrategy,

    /// Output only specific fields (comma-separated)
    #[arg(long = "keys", value_delimiter = ',')]
    pub keys: Vec<String>,

    /// Output only field values (no keys), space-separated
    #[arg(long = "plain")]
    pub plain: bool,

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
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InputFormat {
    Jsonl,
    Line,
    Logfmt,
    Csv,
    Apache,
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
    FailFast,
    EmitErrors,
    DefaultValue,
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

    let cli = Cli::parse();

    // Validate arguments early
    if let Err(e) = validate_cli_args(&cli) {
        stderr.writeln(&format!("Error: {}", e)).unwrap_or(());
        ExitCode::InvalidUsage.exit();
    }

    // Create configuration from CLI
    let config = KeloraConfig::from_cli(&cli);

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
    let (_pipeline, begin_stage, end_stage, mut ctx) = match pipeline_builder.clone().build() {
        Ok(pipeline_components) => pipeline_components,
        Err(e) => {
            stderr.writeln(&format!("Failed to create pipeline: {}", e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }
    };
    
    // Execute begin stage sequentially if provided
    execute_begin_stage(&begin_stage, &mut ctx, stderr);

    // Get reader
    let reader = create_parallel_reader(config, stderr);

    // Process filter and exec stages in parallel using new pipeline architecture
    if let Err(e) = processor.process_with_pipeline(reader, pipeline_builder) {
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

    // Get input reader
    let reader = create_sequential_reader(config, stderr);

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
                    config::ErrorStrategy::FailFast => ExitCode::GeneralError.exit(),
                    _ => continue, // Skip, EmitErrors, and DefaultValue all continue processing
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

/// Create a reader for parallel processing (needs to be Send)
fn create_parallel_reader(config: &KeloraConfig, stderr: &mut SafeStderr) -> Box<dyn BufRead + Send> {
    if config.input.files.is_empty() {
        // For stdin, we need to read all into memory first since stdin lock isn't Send
        let mut buffer = Vec::new();
        if let Err(e) = io::stdin().read_to_end(&mut buffer) {
            stderr.writeln(&format!("Failed to read from stdin: {}", e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }
        Box::new(std::io::Cursor::new(buffer))
    } else {
        let file = std::fs::File::open(&config.input.files[0]).map_err(|e| {
            stderr.writeln(&format!("Failed to open file '{}': {}", config.input.files[0], e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }).unwrap();
        Box::new(BufReader::new(file))
    }
}

/// Create a reader for sequential processing
fn create_sequential_reader(config: &KeloraConfig, stderr: &mut SafeStderr) -> Box<dyn BufRead> {
    if config.input.files.is_empty() {
        Box::new(io::stdin().lock())
    } else {
        let file = std::fs::File::open(&config.input.files[0]).map_err(|e| {
            stderr.writeln(&format!("Failed to open file '{}': {}", config.input.files[0], e)).unwrap_or(());
            ExitCode::GeneralError.exit();
        }).unwrap();
        Box::new(BufReader::new(file))
    }
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


