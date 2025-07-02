use anyhow::Result;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::fs;

use crate::engine::RhaiEngine;
use crate::decompression::DecompressionReader;
use crate::readers::{ChannelStdinReader, MultiFileReader};
use super::{
    EventParser, Formatter, ScriptStage, EventLimiter, 
    Pipeline, PipelineContext, PipelineConfig, MetaData,
    FilterStage, ExecStage, BeginStage, EndStage, KeyFilterStage, LevelFilterStage,
    SimpleChunker, SimpleWindowManager, StdoutWriter, TakeNLimiter
};

/// Pipeline builder for easy construction from CLI arguments
#[derive(Clone)]
pub struct PipelineBuilder {
    config: PipelineConfig,
    begin: Option<String>,
    end: Option<String>,
    input_format: crate::InputFormat,
    output_format: crate::OutputFormat,
    take_limit: Option<usize>,
    keys: Vec<String>,
    exclude_keys: Vec<String>,
    levels: Vec<String>,
    exclude_levels: Vec<String>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Self {
            config: PipelineConfig {
                on_error: crate::ErrorStrategy::Print,
                brief: false,
                no_inject_fields: false,
                inject_prefix: None,
                color_mode: crate::config::ColorMode::Auto,
            },
            begin: None,
            end: None,
            input_format: crate::InputFormat::Jsonl,
            output_format: crate::OutputFormat::Default,
            take_limit: None,
            keys: Vec::new(),
            exclude_keys: Vec::new(),
            levels: Vec::new(),
            exclude_levels: Vec::new(),
        }
    }

    pub fn with_config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    /// Build pipeline with stages
    pub fn build(self, stages: Vec<crate::config::ScriptStageType>) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
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
                let use_colors = crate::tty::should_use_colors_with_mode(&self.config.color_mode);
                Box::new(crate::formatters::DefaultFormatter::new(use_colors, self.config.brief))
            },
            crate::OutputFormat::Logfmt => Box::new(crate::formatters::LogfmtFormatter::new()),
            crate::OutputFormat::Csv => return Err(anyhow::anyhow!("CSV formatter not implemented yet")),
        };

        // Create script stages
        let mut script_stages: Vec<Box<dyn ScriptStage>> = Vec::new();
        
        for stage in stages {
            match stage {
                crate::config::ScriptStageType::Filter(filter) => {
                    let filter_stage = FilterStage::new(filter, &mut rhai_engine)?;
                    script_stages.push(Box::new(filter_stage));
                }
                crate::config::ScriptStageType::Exec(exec) => {
                    let exec_stage = ExecStage::new(exec, &mut rhai_engine)?;
                    script_stages.push(Box::new(exec_stage));
                }
            }
        }

        // Add level filtering stage (runs after all script stages, before key filtering)
        let level_filter_stage = LevelFilterStage::new(self.levels.clone(), self.exclude_levels.clone());
        if level_filter_stage.is_active() {
            script_stages.push(Box::new(level_filter_stage));
        }

        // Add key filtering stage (runs after level filtering, before formatting)
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



    /// Build a worker pipeline for parallel processing
    pub fn build_worker(self, stages: Vec<crate::config::ScriptStageType>) -> Result<(Pipeline, PipelineContext)> {
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
                let use_colors = crate::tty::should_use_colors_with_mode(&self.config.color_mode);
                Box::new(crate::formatters::DefaultFormatter::new(use_colors, self.config.brief))
            },
            crate::OutputFormat::Logfmt => Box::new(crate::formatters::LogfmtFormatter::new()),
            crate::OutputFormat::Csv => return Err(anyhow::anyhow!("CSV formatter not implemented yet")),
        };

        // Create script stages
        let mut script_stages: Vec<Box<dyn ScriptStage>> = Vec::new();
        
        for stage in stages {
            match stage {
                crate::config::ScriptStageType::Filter(filter) => {
                    let filter_stage = FilterStage::new(filter, &mut rhai_engine)?;
                    script_stages.push(Box::new(filter_stage));
                }
                crate::config::ScriptStageType::Exec(exec) => {
                    let exec_stage = ExecStage::new(exec, &mut rhai_engine)?;
                    script_stages.push(Box::new(exec_stage));
                }
            }
        }

        // Add level filtering stage (runs after all script stages, before key filtering)
        let level_filter_stage = LevelFilterStage::new(self.levels.clone(), self.exclude_levels.clone());
        if level_filter_stage.is_active() {
            script_stages.push(Box::new(level_filter_stage));
        }

        // Add key filtering stage (runs after level filtering, before formatting)
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


/// Create a pipeline from configuration
pub fn create_pipeline_from_config(config: &crate::config::KeloraConfig) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
    let builder = create_pipeline_builder_from_config(config);
    builder.build(config.processing.stages.clone())
}

/// Create a pipeline builder from configuration (useful for parallel processing)
pub fn create_pipeline_builder_from_config(config: &crate::config::KeloraConfig) -> PipelineBuilder {
    let pipeline_config = PipelineConfig {
        on_error: config.processing.on_error.clone().into(),
        brief: config.output.brief,
        no_inject_fields: config.processing.no_inject_fields,
        inject_prefix: config.processing.inject_prefix.clone(),
        color_mode: config.output.color.clone(),
    };

    let mut builder = PipelineBuilder::new()
        .with_config(pipeline_config)
        .with_begin(config.processing.begin.clone())
        .with_end(config.processing.end.clone())
        .with_input_format(config.input.format.clone().into())
        .with_output_format(config.output.format.clone().into());
    builder.keys = config.output.get_effective_keys();
    builder.exclude_keys = config.output.exclude_keys.clone();
    builder.levels = config.processing.levels.clone();
    builder.exclude_levels = config.processing.exclude_levels.clone();
    builder
}


/// Create concatenated content from multiple files for parallel processing
/// DEPRECATED: Use streaming readers instead
#[allow(dead_code)]
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

/// Create input reader with optional decompression for parallel processing
pub fn create_input_reader(config: &crate::config::KeloraConfig) -> Result<Box<dyn BufRead + Send>> {
    if config.input.files.is_empty() {
        // Use channel-based stdin reader for Send compatibility
        Ok(Box::new(ChannelStdinReader::new()?))
    } else {
        let sorted_files = sort_files(&config.input.files, &config.input.file_order)?;
        Ok(Box::new(MultiFileReader::new(sorted_files)?))
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
        // Use stdin lock directly for sequential processing (most efficient)
        Ok(Box::new(io::stdin().lock()))
    } else {
        // Use streaming multi-file reader for sequential processing too
        let sorted_files = sort_files(&config.input.files, &config.input.file_order)?;
        Ok(Box::new(MultiFileReader::new(sorted_files)?))
    }
}