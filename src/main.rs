use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};

mod engine;
mod event;
mod formatters;
mod parallel;
mod parsers;

use engine::RhaiEngine;
use event::Event;
use formatters::{Formatter, JsonFormatter, TextFormatter};
use parallel::{ParallelConfig, ParallelProcessor, ProcessRequest};
use parsers::{JsonlParser, Parser as LogParser};

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
    #[arg(short = 'f', long = "format", value_enum, default_value = "json")]
    pub format: InputFormat,

    /// Output format
    #[arg(
        short = 'F',
        long = "output-format",
        value_enum,
        default_value = "json"
    )]
    pub output_format: OutputFormat,

    /// Run once before processing
    #[arg(long = "begin")]
    pub begin: Option<String>,

    /// Boolean filter expressions (can be repeated)
    #[arg(long = "filter")]
    pub filters: Vec<String>,

    /// Transform/process expressions (can be repeated)
    #[arg(long = "eval")]
    pub evals: Vec<String>,

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
    Json,
    Line,
    Csv,
    Apache,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Json,
    Text,
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
    let cli = Cli::parse();

    // Create parser based on input format
    let parser = create_parser(&cli.format);

    // Create formatter based on output format
    let formatter = create_formatter(&cli.output_format);

    // Create Rhai engine with custom functions
    let mut engine = RhaiEngine::new();
    
    // Compile all expressions at startup
    engine.compile_expressions(&cli.filters, &cli.evals, cli.begin.as_ref(), cli.end.as_ref())?;

    // Global tracking state
    let mut tracked: HashMap<String, rhai::Dynamic> = HashMap::new();

    // Determine processing mode and smart defaults
    let use_parallel = cli.parallel || cli.threads > 0 || cli.batch_size.is_some();
    
    // Smart defaults based on mode
    let batch_size = cli.batch_size.unwrap_or(1000);

    if use_parallel {
        // Parallel processing mode
        let config = ParallelConfig {
            num_workers: if cli.threads == 0 { num_cpus::get() } else { cli.threads },
            batch_size,
            batch_timeout_ms: cli.batch_timeout,
            preserve_order: !cli.no_preserve_order,
            buffer_size: Some(10000),
        };

        let processor = ParallelProcessor::new(config);
        
        // Execute begin stage sequentially if provided
        engine.execute_begin(&mut tracked)?;

        // Get reader
        let reader: Box<dyn BufRead + Send> = if cli.files.is_empty() {
            // For stdin, we need to read all into memory first since stdin lock isn't Send
            let mut buffer = Vec::new();
            io::stdin().read_to_end(&mut buffer)?;
            Box::new(std::io::Cursor::new(buffer))
        } else {
            let file = std::fs::File::open(&cli.files[0])
                .with_context(|| format!("Failed to open file: {}", cli.files[0]))?;
            Box::new(BufReader::new(file))
        };

        // Process filter and eval stages in parallel
        let request = ProcessRequest {
            input_format: cli.format,
            filters: cli.filters.clone(),
            evals: cli.evals.clone(),
            output_format: cli.output_format,
            on_error: cli.on_error,
            keys: cli.keys,
        };
        
        processor.process(reader, request)?;

        // Merge the parallel tracked state with our sequential tracked state
        let parallel_tracked = processor.get_final_tracked_state();
        for (key, dynamic_value) in parallel_tracked {
            tracked.insert(key, dynamic_value);
        }

        // Execute end stage sequentially with merged state
        engine.execute_end(&tracked)?;
    } else {
        // Original sequential processing mode
        engine.execute_begin(&mut tracked)?;

        let reader: Box<dyn BufRead> = if cli.files.is_empty() {
            Box::new(io::stdin().lock())
        } else {
            let file = std::fs::File::open(&cli.files[0])
                .with_context(|| format!("Failed to open file: {}", cli.files[0]))?;
            Box::new(BufReader::new(file))
        };

        let mut line_num = 0;
        for line_result in reader.lines() {
            let line = line_result?;
            line_num += 1;

            if line.trim().is_empty() {
                continue;
            }

            let mut event = match parser.parse(&line) {
                Ok(event) => event,
                Err(e) => match cli.on_error {
                    ErrorStrategy::Skip => continue,
                    ErrorStrategy::FailFast => return Err(e),
                    ErrorStrategy::EmitErrors => {
                        eprintln!("Parse error on line {}: {}", line_num, e);
                        continue;
                    }
                    ErrorStrategy::DefaultValue => Event::default_with_line(line),
                },
            };

            event.set_metadata(line_num, None);

            let should_output = match engine.execute_filters(&event, &mut tracked) {
                Ok(result) => result,
                Err(e) => match cli.on_error {
                    ErrorStrategy::Skip => false,
                    ErrorStrategy::FailFast => return Err(e),
                    ErrorStrategy::EmitErrors => {
                        eprintln!("Filter error on line {}: {}", line_num, e);
                        false
                    }
                    ErrorStrategy::DefaultValue => true,
                },
            };

            if !should_output {
                continue;
            }

            if let Err(e) = engine.execute_evals(&mut event, &mut tracked) {
                match cli.on_error {
                    ErrorStrategy::Skip => continue,
                    ErrorStrategy::FailFast => return Err(e),
                    ErrorStrategy::EmitErrors => {
                        eprintln!("Eval error on line {}: {}", line_num, e);
                        continue;
                    }
                    ErrorStrategy::DefaultValue => {}
                }
            }

            if !cli.keys.is_empty() {
                event.filter_keys(&cli.keys);
            }

            println!("{}", formatter.format(&event));
            io::stdout().flush().unwrap_or(());
        }

        engine.execute_end(&tracked)?;
    }

    Ok(())
}

fn create_parser(format: &InputFormat) -> Box<dyn LogParser> {
    match format {
        InputFormat::Json => Box::new(JsonlParser::new()),
        InputFormat::Line => todo!("Line parser not implemented yet"),
        InputFormat::Csv => todo!("CSV parser not implemented yet"),
        InputFormat::Apache => todo!("Apache parser not implemented yet"),
    }
}

fn create_formatter(format: &OutputFormat) -> Box<dyn Formatter> {
    match format {
        OutputFormat::Json => Box::new(JsonFormatter::new()),
        OutputFormat::Text => Box::new(TextFormatter::new()),
        OutputFormat::Csv => todo!("CSV formatter not implemented yet"),
    }
}

