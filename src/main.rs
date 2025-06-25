use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader};

mod engine;
mod event;
mod formatters;
mod parsers;

use engine::RhaiEngine;
use event::Event;
use formatters::{Formatter, JsonFormatter, TextFormatter};
use parsers::{JsonlParser, Parser as LogParser};

#[derive(Parser)]
#[command(name = "kelora")]
#[command(about = "A command-line log analysis tool with embedded Rhai scripting")]
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
    #[arg(long = "no-inject-fields")]
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

    // Execute begin stage if provided
    engine.execute_begin(&mut tracked)?;

    // Process input
    let reader: Box<dyn BufRead> = if cli.files.is_empty() {
        Box::new(io::stdin().lock())
    } else {
        // For MVP, just handle first file
        let file = std::fs::File::open(&cli.files[0])
            .with_context(|| format!("Failed to open file: {}", cli.files[0]))?;
        Box::new(BufReader::new(file))
    };

    let mut line_num = 0;
    for line_result in reader.lines() {
        let line = line_result?;
        line_num += 1;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Parse the line into an event
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

        // Set metadata
        event.set_metadata(line_num, None);

        // Apply filters
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

        // Apply eval expressions
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

        // Filter keys if specified
        if !cli.keys.is_empty() {
            event.filter_keys(&cli.keys);
        }

        // Output the event
        println!("{}", formatter.format(&event));
    }

    // Execute end stage if provided
    engine.execute_end(&tracked)?;

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
