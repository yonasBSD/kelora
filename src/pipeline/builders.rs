use anyhow::Result;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::fs;

use crate::engine::RhaiEngine;
use crate::decompression::DecompressionReader;
use super::{
    EventParser, Formatter, ScriptStage, EventLimiter, 
    Pipeline, PipelineContext, PipelineConfig, MetaData,
    FilterStage, ExecStage, BeginStage, EndStage, KeyFilterStage,
    SimpleChunker, SimpleWindowManager, StdoutWriter, TakeNLimiter
};

/// Pipeline builder for easy construction from CLI arguments
#[derive(Clone)]
pub struct PipelineBuilder {
    config: PipelineConfig,
    filters: Vec<String>,
    execs: Vec<String>,
    begin: Option<String>,
    end: Option<String>,
    input_format: crate::InputFormat,
    output_format: crate::OutputFormat,
    take_limit: Option<usize>,
    keys: Vec<String>,
    exclude_keys: Vec<String>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Self {
            config: PipelineConfig {
                on_error: crate::ErrorStrategy::Print,
                plain: false,
                no_inject_fields: false,
                inject_prefix: None,
            },
            filters: Vec::new(),
            execs: Vec::new(),
            begin: None,
            end: None,
            input_format: crate::InputFormat::Jsonl,
            output_format: crate::OutputFormat::Default,
            take_limit: None,
            keys: Vec::new(),
            exclude_keys: Vec::new(),
        }
    }

    pub fn with_config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_filters(mut self, filters: Vec<String>) -> Self {
        self.filters = filters;
        self
    }

    pub fn with_execs(mut self, execs: Vec<String>) -> Self {
        self.execs = execs;
        self
    }

    pub fn with_begin(mut self, begin: Option<String>) -> Self {
        self.begin = begin;
        self
    }

    pub fn with_end(mut self, end: Option<String>) -> Self {
        self.end = end;
        self
    }

    pub fn with_input_format(mut self, format: crate::InputFormat) -> Self {
        self.input_format = format;
        self
    }

    pub fn with_output_format(mut self, format: crate::OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    #[allow(dead_code)]
    pub fn with_take_limit(mut self, limit: Option<usize>) -> Self {
        self.take_limit = limit;
        self
    }

    /// Build a complete pipeline with begin/end stages for sequential processing
    pub fn build(self) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
        let mut rhai_engine = RhaiEngine::new();

        // Create parser
        let parser: Box<dyn EventParser> = match self.input_format {
            crate::InputFormat::Jsonl => Box::new(crate::parsers::JsonlParser::new()),
            crate::InputFormat::Line => Box::new(crate::parsers::LineParser::new()),
            crate::InputFormat::Logfmt => Box::new(crate::parsers::LogfmtParser::new()),
            crate::InputFormat::Syslog => Box::new(crate::parsers::SyslogParser::new()?),
            crate::InputFormat::Csv => return Err(anyhow::anyhow!("CSV parser not implemented yet")),
            crate::InputFormat::Apache => Box::new(crate::parsers::ApacheParser::new()?),
            crate::InputFormat::Nginx => Box::new(crate::parsers::NginxParser::new()?),
        };

        // Create formatter
        let formatter: Box<dyn Formatter> = match self.output_format {
            crate::OutputFormat::Jsonl => Box::new(crate::formatters::JsonFormatter::new()),
            crate::OutputFormat::Default => {
                let use_colors = crate::tty::should_use_colors();
                Box::new(crate::formatters::DefaultFormatter::new(use_colors, self.config.plain))
            },
            crate::OutputFormat::Logfmt => Box::new(crate::formatters::LogfmtFormatter::new()),
            crate::OutputFormat::Csv => return Err(anyhow::anyhow!("CSV formatter not implemented yet")),
        };

        // Create script stages
        let mut script_stages: Vec<Box<dyn ScriptStage>> = Vec::new();
        
        if !self.filters.is_empty() {
            let filter_stage = FilterStage::new(self.filters, &mut rhai_engine)?;
            script_stages.push(Box::new(filter_stage));
        }

        if !self.execs.is_empty() {
            let exec_stage = ExecStage::new(self.execs, &mut rhai_engine)?;
            script_stages.push(Box::new(exec_stage));
        }

        // Add key filtering stage (runs after filters and execs, before formatting)
        let key_filter_stage = KeyFilterStage::new(self.keys.clone(), self.exclude_keys.clone());
        if key_filter_stage.is_active() {
            script_stages.push(Box::new(key_filter_stage));
        }

        // Create limiter if specified
        let limiter: Option<Box<dyn EventLimiter>> = if let Some(limit) = self.take_limit {
            Some(Box::new(TakeNLimiter::new(limit)))
        } else {
            None
        };

        // Create begin and end stages
        let begin_stage = BeginStage::new(self.begin, &mut rhai_engine)?;
        let end_stage = EndStage::new(self.end, &mut rhai_engine)?;

        // Create pipeline context
        let ctx = PipelineContext {
            config: self.config,
            tracker: HashMap::new(),
            window: Vec::new(),
            rhai: rhai_engine.clone(),
            meta: MetaData::default(),
        };

        // Create pipeline
        let pipeline = Pipeline {
            line_filter: None, // No line filter implementation yet
            chunker: Box::new(SimpleChunker),
            parser,
            script_stages,
            limiter,
            formatter,
            output: Box::new(StdoutWriter),
            window_manager: Box::new(SimpleWindowManager::new()),
        };

        Ok((pipeline, begin_stage, end_stage, ctx))
    }

    /// Build a worker pipeline for parallel processing (no begin/end stages, no output writer)
    #[allow(dead_code)]
    pub fn build_worker(self) -> Result<(Pipeline, PipelineContext)> {
        let mut rhai_engine = RhaiEngine::new();

        // Create parser
        let parser: Box<dyn EventParser> = match self.input_format {
            crate::InputFormat::Jsonl => Box::new(crate::parsers::JsonlParser::new()),
            crate::InputFormat::Line => Box::new(crate::parsers::LineParser::new()),
            crate::InputFormat::Logfmt => Box::new(crate::parsers::LogfmtParser::new()),
            crate::InputFormat::Syslog => Box::new(crate::parsers::SyslogParser::new()?),
            crate::InputFormat::Csv => return Err(anyhow::anyhow!("CSV parser not implemented yet")),
            crate::InputFormat::Apache => Box::new(crate::parsers::ApacheParser::new()?),
            crate::InputFormat::Nginx => Box::new(crate::parsers::NginxParser::new()?),
        };

        // Create formatter (workers still need formatters for output)
        let formatter: Box<dyn Formatter> = match self.output_format {
            crate::OutputFormat::Jsonl => Box::new(crate::formatters::JsonFormatter::new()),
            crate::OutputFormat::Default => {
                let use_colors = crate::tty::should_use_colors();
                Box::new(crate::formatters::DefaultFormatter::new(use_colors, self.config.plain))
            },
            crate::OutputFormat::Logfmt => Box::new(crate::formatters::LogfmtFormatter::new()),
            crate::OutputFormat::Csv => return Err(anyhow::anyhow!("CSV formatter not implemented yet")),
        };

        // Create script stages
        let mut script_stages: Vec<Box<dyn ScriptStage>> = Vec::new();
        
        if !self.filters.is_empty() {
            let filter_stage = FilterStage::new(self.filters, &mut rhai_engine)?;
            script_stages.push(Box::new(filter_stage));
        }

        if !self.execs.is_empty() {
            let exec_stage = ExecStage::new(self.execs, &mut rhai_engine)?;
            script_stages.push(Box::new(exec_stage));
        }

        // Add key filtering stage (runs after filters and execs, before formatting)
        let key_filter_stage = KeyFilterStage::new(self.keys.clone(), self.exclude_keys.clone());
        if key_filter_stage.is_active() {
            script_stages.push(Box::new(key_filter_stage));
        }

        // No limiter for parallel workers (limiting happens at the result sink level)
        let limiter: Option<Box<dyn EventLimiter>> = None;

        // Create pipeline context
        let ctx = PipelineContext {
            config: self.config,
            tracker: HashMap::new(),
            window: Vec::new(),
            rhai: rhai_engine.clone(),
            meta: MetaData::default(),
        };

        // Create worker pipeline (no output writer - results are collected by the processor)
        let pipeline = Pipeline {
            line_filter: None,
            chunker: Box::new(SimpleChunker),
            parser,
            script_stages,
            limiter,
            formatter,
            output: Box::new(StdoutWriter), // This won't actually be used in parallel mode
            window_manager: Box::new(SimpleWindowManager::new()),
        };

        Ok((pipeline, ctx))
    }
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a pipeline from CLI arguments
/// Deprecated: Use create_pipeline_from_config instead
#[allow(dead_code)]
pub fn create_pipeline_from_cli(cli: &crate::Cli) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
    let builder = create_pipeline_builder_from_cli(cli);
    builder.build()
}

/// Create a pipeline builder from CLI arguments (useful for parallel processing)
/// Deprecated: Use create_pipeline_builder_from_config instead
#[allow(dead_code)]
pub fn create_pipeline_builder_from_cli(cli: &crate::Cli) -> PipelineBuilder {
    let config = PipelineConfig {
        on_error: cli.on_error.clone(),
        plain: cli.plain,
        no_inject_fields: cli.no_inject_fields,
        inject_prefix: cli.inject_prefix.clone(),
    };

    let mut builder = PipelineBuilder::new()
        .with_config(config)
        .with_filters(cli.filters.clone())
        .with_execs(cli.execs.clone())
        .with_begin(cli.begin.clone())
        .with_end(cli.end.clone())
        .with_input_format(cli.format.clone())
        .with_output_format(cli.output_format.clone());
    builder.keys = cli.keys.clone();
    builder.exclude_keys = cli.exclude_keys.clone();
    builder
}

/// Create a pipeline from configuration
pub fn create_pipeline_from_config(config: &crate::config::KeloraConfig) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
    let builder = create_pipeline_builder_from_config(config);
    builder.build()
}

/// Create a pipeline builder from configuration (useful for parallel processing)
pub fn create_pipeline_builder_from_config(config: &crate::config::KeloraConfig) -> PipelineBuilder {
    let pipeline_config = PipelineConfig {
        on_error: config.processing.on_error.clone().into(),
        plain: config.output.plain,
        no_inject_fields: config.processing.no_inject_fields,
        inject_prefix: config.processing.inject_prefix.clone(),
    };

    let mut builder = PipelineBuilder::new()
        .with_config(pipeline_config)
        .with_filters(config.processing.filters.clone())
        .with_execs(config.processing.execs.clone())
        .with_begin(config.processing.begin.clone())
        .with_end(config.processing.end.clone())
        .with_input_format(config.input.format.clone().into())
        .with_output_format(config.output.format.clone().into());
    builder.keys = config.output.keys.clone();
    builder.exclude_keys = config.output.exclude_keys.clone();
    builder
}


/// Create concatenated content from multiple files for parallel processing
fn read_all_files_to_memory(files: &[String], _config: &crate::config::KeloraConfig) -> Result<Vec<u8>> {
    let mut all_content = Vec::new();
    
    for file_path in files {
        let mut reader = DecompressionReader::new(file_path)?;
        io::Read::read_to_end(&mut reader, &mut all_content)?;
        
        // Add a newline between files if the last file doesn't end with one
        if !all_content.is_empty() && all_content[all_content.len() - 1] != b'\n' {
            all_content.push(b'\n');
        }
    }
    
    Ok(all_content)
}

/// Create input reader with optional decompression
pub fn create_input_reader(config: &crate::config::KeloraConfig) -> Result<Box<dyn BufRead + Send>> {
    if config.input.files.is_empty() {
        // For stdin, read all into memory since stdin lock isn't Send
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut io::stdin(), &mut buffer)?;
        Ok(Box::new(std::io::Cursor::new(buffer)))
    } else {
        let sorted_files = sort_files(&config.input.files, &config.input.file_order)?;
        let all_content = read_all_files_to_memory(&sorted_files, config)?;
        Ok(Box::new(std::io::Cursor::new(all_content)))
    }
}

/// Sort files according to the specified file order
fn sort_files(files: &[String], order: &crate::config::FileOrder) -> Result<Vec<String>> {
    let mut sorted_files = files.to_vec();
    
    match order {
        crate::config::FileOrder::None => {
            // Keep CLI order - no sorting needed
        }
        crate::config::FileOrder::Name => {
            sorted_files.sort();
        }
        crate::config::FileOrder::Mtime => {
            // Sort by modification time (oldest first)
            sorted_files.sort_by(|a, b| {
                let mtime_a = fs::metadata(a)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let mtime_b = fs::metadata(b)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                mtime_a.cmp(&mtime_b)
            });
        }
    }
    
    Ok(sorted_files)
}


/// Create input reader for sequential processing (doesn't need to be Send)
pub fn create_sequential_input_reader(config: &crate::config::KeloraConfig) -> Result<Box<dyn BufRead>> {
    if config.input.files.is_empty() {
        Ok(Box::new(io::stdin().lock()))
    } else {
        let sorted_files = sort_files(&config.input.files, &config.input.file_order)?;
        let all_content = read_all_files_to_memory(&sorted_files, config)?;
        Ok(Box::new(std::io::Cursor::new(all_content)))
    }
}