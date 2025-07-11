use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};

/// Wrapper parser that applies timestamp configuration after parsing
struct TimestampConfiguredParser {
    inner: Box<dyn EventParser>,
    ts_config: crate::timestamp::TsConfig,
}

impl TimestampConfiguredParser {
    fn new(inner: Box<dyn EventParser>, ts_field: Option<String>, ts_format: Option<String>, default_timezone: Option<String>) -> Self {
        Self {
            inner,
            ts_config: crate::timestamp::TsConfig {
                custom_field: ts_field,
                custom_format: ts_format,
                default_timezone,
                auto_parse: true,
            },
        }
    }
}

impl EventParser for TimestampConfiguredParser {
    fn parse(&self, line: &str) -> Result<crate::event::Event> {
        let mut event = self.inner.parse(line)?;
        // Apply timestamp configuration
        event.extract_timestamp_with_config(None, &self.ts_config);
        Ok(event)
    }
}

use super::{
    create_multiline_chunker, BeginStage, EndStage, EventLimiter, EventParser, ExecStage,
    FilterStage, Formatter, KeyFilterStage, LevelFilterStage, MetaData, Pipeline, PipelineConfig,
    PipelineContext, ScriptStage, SimpleChunker, SimpleWindowManager, SlidingWindowManager,
    StdoutWriter, TakeNLimiter, TimestampFilterStage,
};
use crate::decompression::DecompressionReader;
use crate::engine::RhaiEngine;
use crate::readers::{ChannelStdinReader, MultiFileReader};

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
    multiline: Option<crate::config::MultilineConfig>,
    window_size: usize,
    csv_headers: Option<Vec<String>>, // Pre-processed CSV headers for parallel mode
    timestamp_filter: Option<crate::config::TimestampFilterConfig>,
    ts_field: Option<String>,
    ts_format: Option<String>,
    default_timezone: Option<String>,
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
            multiline: None,
            window_size: 0,
            csv_headers: None,
            timestamp_filter: None,
            ts_field: None,
            ts_format: None,
            default_timezone: None,
        }
    }

    pub fn with_config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    /// Build pipeline with stages
    pub fn build(
        self,
        stages: Vec<crate::config::ScriptStageType>,
    ) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
        let mut rhai_engine = RhaiEngine::new();

        // Create parser
        let base_parser: Box<dyn EventParser> = match self.input_format {
            crate::InputFormat::Jsonl => Box::new(crate::parsers::JsonlParser::new()),
            crate::InputFormat::Line => Box::new(crate::parsers::LineParser::new()),
            crate::InputFormat::Logfmt => Box::new(crate::parsers::LogfmtParser::new()),
            crate::InputFormat::Syslog => Box::new(crate::parsers::SyslogParser::new()?),
            crate::InputFormat::Cef => Box::new(crate::parsers::CefParser::new()),
            crate::InputFormat::Csv => {
                if let Some(ref headers) = self.csv_headers {
                    Box::new(crate::parsers::CsvParser::new_csv_with_headers(
                        headers.clone(),
                    ))
                } else {
                    Box::new(crate::parsers::CsvParser::new_csv())
                }
            }
            crate::InputFormat::Tsv => {
                if let Some(ref headers) = self.csv_headers {
                    Box::new(crate::parsers::CsvParser::new_tsv_with_headers(
                        headers.clone(),
                    ))
                } else {
                    Box::new(crate::parsers::CsvParser::new_tsv())
                }
            }
            crate::InputFormat::Csvnh => {
                if let Some(ref headers) = self.csv_headers {
                    Box::new(crate::parsers::CsvParser::new_csv_no_headers_with_columns(
                        headers.clone(),
                    ))
                } else {
                    Box::new(crate::parsers::CsvParser::new_csv_no_headers())
                }
            }
            crate::InputFormat::Tsvnh => {
                if let Some(ref headers) = self.csv_headers {
                    Box::new(crate::parsers::CsvParser::new_tsv_no_headers_with_columns(
                        headers.clone(),
                    ))
                } else {
                    Box::new(crate::parsers::CsvParser::new_tsv_no_headers())
                }
            }
            crate::InputFormat::Apache => Box::new(crate::parsers::ApacheParser::new()?),
            crate::InputFormat::Nginx => Box::new(crate::parsers::NginxParser::new()?),
            crate::InputFormat::Cols => Box::new(crate::parsers::ColsParser::new()),
        };

        // Wrap parser with timestamp configuration if needed
        let parser: Box<dyn EventParser> = if self.ts_field.is_some() || self.ts_format.is_some() || self.default_timezone.is_some() {
            Box::new(TimestampConfiguredParser::new(
                base_parser,
                self.ts_field.clone(),
                self.ts_format.clone(),
                self.default_timezone.clone(),
            ))
        } else {
            base_parser
        };

        // Create formatter
        let formatter: Box<dyn Formatter> = match self.output_format {
            crate::OutputFormat::Jsonl => Box::new(crate::formatters::JsonFormatter::new()),
            crate::OutputFormat::Default => {
                let use_colors = crate::tty::should_use_colors_with_mode(&self.config.color_mode);
                Box::new(crate::formatters::DefaultFormatter::new(
                    use_colors,
                    self.config.brief,
                ))
            }
            crate::OutputFormat::Logfmt => Box::new(crate::formatters::LogfmtFormatter::new()),
            crate::OutputFormat::Csv => {
                if self.keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "CSV output format requires --keys to specify field order"
                    ));
                }
                Box::new(crate::formatters::CsvFormatter::new(self.keys.clone()))
            }
            crate::OutputFormat::Tsv => {
                if self.keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "TSV output format requires --keys to specify field order"
                    ));
                }
                Box::new(crate::formatters::CsvFormatter::new_tsv(self.keys.clone()))
            }
            crate::OutputFormat::Csvnh => {
                if self.keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "CSVNH output format requires --keys to specify field order"
                    ));
                }
                Box::new(crate::formatters::CsvFormatter::new_csv_no_header(
                    self.keys.clone(),
                ))
            }
            crate::OutputFormat::Tsvnh => {
                if self.keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "TSVNH output format requires --keys to specify field order"
                    ));
                }
                Box::new(crate::formatters::CsvFormatter::new_tsv_no_header(
                    self.keys.clone(),
                ))
            }
            crate::OutputFormat::Hide => Box::new(crate::formatters::HideFormatter::new()),
            crate::OutputFormat::Null => {
                // Null format: suppress side effects in addition to hiding output
                rhai_engine.set_suppress_side_effects(true);
                Box::new(crate::formatters::HideFormatter::new())
            }
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

        // Add timestamp filtering stage (runs after script stages, before level filtering)
        if let Some(timestamp_filter_config) = self.timestamp_filter {
            let timestamp_filter_stage = TimestampFilterStage::new(timestamp_filter_config);
            script_stages.push(Box::new(timestamp_filter_stage));
        }

        // Add level filtering stage (runs after timestamp filtering, before key filtering)
        let level_filter_stage =
            LevelFilterStage::new(self.levels.clone(), self.exclude_levels.clone());
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

        // Create chunker based on multiline configuration
        let chunker = if let Some(ref multiline_config) = self.multiline {
            create_multiline_chunker(multiline_config)
                .map_err(|e| anyhow::anyhow!("Failed to create multiline chunker: {}", e))?
        } else {
            Box::new(SimpleChunker) as Box<dyn super::Chunker>
        };

        // Create window manager based on window_size configuration
        let window_manager: Box<dyn super::WindowManager> = if self.window_size > 0 {
            Box::new(SlidingWindowManager::new(self.window_size))
        } else {
            Box::new(SimpleWindowManager::new())
        };

        // Create pipeline
        let pipeline = Pipeline {
            line_filter: None, // No line filter implementation yet
            chunker,
            parser,
            script_stages,
            limiter,
            formatter,
            output: Box::new(StdoutWriter),
            window_manager,
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
    pub fn build_worker(
        self,
        stages: Vec<crate::config::ScriptStageType>,
    ) -> Result<(Pipeline, PipelineContext)> {
        let mut rhai_engine = RhaiEngine::new();

        // Create parser (with pre-processed CSV headers if available)
        let base_parser: Box<dyn EventParser> = match self.input_format {
            crate::InputFormat::Jsonl => Box::new(crate::parsers::JsonlParser::new()),
            crate::InputFormat::Line => Box::new(crate::parsers::LineParser::new()),
            crate::InputFormat::Logfmt => Box::new(crate::parsers::LogfmtParser::new()),
            crate::InputFormat::Syslog => Box::new(crate::parsers::SyslogParser::new()?),
            crate::InputFormat::Cef => Box::new(crate::parsers::CefParser::new()),
            crate::InputFormat::Csv => {
                if let Some(ref headers) = self.csv_headers {
                    Box::new(crate::parsers::CsvParser::new_csv_with_headers(
                        headers.clone(),
                    ))
                } else {
                    Box::new(crate::parsers::CsvParser::new_csv())
                }
            }
            crate::InputFormat::Tsv => {
                if let Some(ref headers) = self.csv_headers {
                    Box::new(crate::parsers::CsvParser::new_tsv_with_headers(
                        headers.clone(),
                    ))
                } else {
                    Box::new(crate::parsers::CsvParser::new_tsv())
                }
            }
            crate::InputFormat::Csvnh => {
                if let Some(ref headers) = self.csv_headers {
                    Box::new(crate::parsers::CsvParser::new_csv_no_headers_with_columns(
                        headers.clone(),
                    ))
                } else {
                    Box::new(crate::parsers::CsvParser::new_csv_no_headers())
                }
            }
            crate::InputFormat::Tsvnh => {
                if let Some(ref headers) = self.csv_headers {
                    Box::new(crate::parsers::CsvParser::new_tsv_no_headers_with_columns(
                        headers.clone(),
                    ))
                } else {
                    Box::new(crate::parsers::CsvParser::new_tsv_no_headers())
                }
            }
            crate::InputFormat::Apache => Box::new(crate::parsers::ApacheParser::new()?),
            crate::InputFormat::Nginx => Box::new(crate::parsers::NginxParser::new()?),
            crate::InputFormat::Cols => Box::new(crate::parsers::ColsParser::new()),
        };

        // Wrap parser with timestamp configuration if needed
        let parser: Box<dyn EventParser> = if self.ts_field.is_some() || self.ts_format.is_some() || self.default_timezone.is_some() {
            Box::new(TimestampConfiguredParser::new(
                base_parser,
                self.ts_field.clone(),
                self.ts_format.clone(),
                self.default_timezone.clone(),
            ))
        } else {
            base_parser
        };

        // Create formatter (workers still need formatters for output)
        let formatter: Box<dyn Formatter> = match self.output_format {
            crate::OutputFormat::Jsonl => Box::new(crate::formatters::JsonFormatter::new()),
            crate::OutputFormat::Default => {
                let use_colors = crate::tty::should_use_colors_with_mode(&self.config.color_mode);
                Box::new(crate::formatters::DefaultFormatter::new(
                    use_colors,
                    self.config.brief,
                ))
            }
            crate::OutputFormat::Logfmt => Box::new(crate::formatters::LogfmtFormatter::new()),
            crate::OutputFormat::Csv => {
                if self.keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "CSV output format requires --keys to specify field order"
                    ));
                }
                Box::new(crate::formatters::CsvFormatter::new_worker(
                    self.keys.clone(),
                ))
            }
            crate::OutputFormat::Tsv => {
                if self.keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "TSV output format requires --keys to specify field order"
                    ));
                }
                Box::new(crate::formatters::CsvFormatter::new_tsv_worker(
                    self.keys.clone(),
                ))
            }
            crate::OutputFormat::Csvnh => {
                if self.keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "CSVNH output format requires --keys to specify field order"
                    ));
                }
                Box::new(crate::formatters::CsvFormatter::new_csv_no_header_worker(
                    self.keys.clone(),
                ))
            }
            crate::OutputFormat::Tsvnh => {
                if self.keys.is_empty() {
                    return Err(anyhow::anyhow!(
                        "TSVNH output format requires --keys to specify field order"
                    ));
                }
                Box::new(crate::formatters::CsvFormatter::new_tsv_no_header_worker(
                    self.keys.clone(),
                ))
            }
            crate::OutputFormat::Hide => Box::new(crate::formatters::HideFormatter::new()),
            crate::OutputFormat::Null => {
                // Null format: suppress side effects in addition to hiding output
                rhai_engine.set_suppress_side_effects(true);
                Box::new(crate::formatters::HideFormatter::new())
            }
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

        // Add timestamp filtering stage (runs after script stages, before level filtering)
        if let Some(timestamp_filter_config) = self.timestamp_filter {
            let timestamp_filter_stage = TimestampFilterStage::new(timestamp_filter_config);
            script_stages.push(Box::new(timestamp_filter_stage));
        }

        // Add level filtering stage (runs after timestamp filtering, before key filtering)
        let level_filter_stage =
            LevelFilterStage::new(self.levels.clone(), self.exclude_levels.clone());
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

        // Create chunker based on multiline configuration
        let chunker = if let Some(ref multiline_config) = self.multiline {
            create_multiline_chunker(multiline_config)
                .map_err(|e| anyhow::anyhow!("Failed to create multiline chunker: {}", e))?
        } else {
            Box::new(SimpleChunker) as Box<dyn super::Chunker>
        };

        // Create window manager based on window_size configuration
        let window_manager: Box<dyn super::WindowManager> = if self.window_size > 0 {
            Box::new(SlidingWindowManager::new(self.window_size))
        } else {
            Box::new(SimpleWindowManager::new())
        };

        // Create worker pipeline (no output writer - results are collected by the processor)
        let pipeline = Pipeline {
            line_filter: None,
            chunker,
            parser,
            script_stages,
            limiter,
            formatter,
            output: Box::new(StdoutWriter), // This won't actually be used in parallel mode
            window_manager,
        };

        Ok((pipeline, ctx))
    }

    pub fn with_csv_headers(mut self, headers: Vec<String>) -> Self {
        self.csv_headers = Some(headers);
        self
    }

    #[allow(dead_code)]
    pub fn with_timestamp_filter(
        mut self,
        timestamp_filter: Option<crate::config::TimestampFilterConfig>,
    ) -> Self {
        self.timestamp_filter = timestamp_filter;
        self
    }

    pub fn with_ts_field(mut self, ts_field: Option<String>) -> Self {
        self.ts_field = ts_field;
        self
    }

    pub fn with_ts_format(mut self, ts_format: Option<String>) -> Self {
        self.ts_format = ts_format;
        self
    }

    pub fn with_default_timezone(mut self, default_timezone: Option<String>) -> Self {
        self.default_timezone = default_timezone;
        self
    }
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a pipeline from configuration
pub fn create_pipeline_from_config(
    config: &crate::config::KeloraConfig,
) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
    let builder = create_pipeline_builder_from_config(config);
    builder.build(config.processing.stages.clone())
}

/// Create a pipeline builder from configuration (useful for parallel processing)
pub fn create_pipeline_builder_from_config(
    config: &crate::config::KeloraConfig,
) -> PipelineBuilder {
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
    builder.multiline = config.input.multiline.clone();
    builder.window_size = config.processing.window_size;
    builder.timestamp_filter = config.processing.timestamp_filter.clone();
    builder.ts_field = config.input.ts_field.clone();
    builder.ts_format = config.input.ts_format.clone();
    builder.default_timezone = config.input.default_timezone.clone();
    builder
}

/// Create concatenated content from multiple files for parallel processing
/// DEPRECATED: Use streaming readers instead
#[allow(dead_code)]
fn read_all_files_to_memory(
    files: &[String],
    _config: &crate::config::KeloraConfig,
) -> Result<Vec<u8>> {
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
pub fn create_input_reader(
    config: &crate::config::KeloraConfig,
) -> Result<Box<dyn BufRead + Send>> {
    if config.input.files.is_empty() {
        // Use channel-based stdin reader for Send compatibility
        Ok(Box::new(ChannelStdinReader::new()?))
    } else {
        let sorted_files = sort_files(&config.input.files, &config.input.file_order)?;
        Ok(Box::new(MultiFileReader::new(sorted_files)?))
    }
}

/// Create file-aware input reader for parallel processing with filename tracking
pub fn create_file_aware_input_reader(
    config: &crate::config::KeloraConfig,
) -> Result<Box<dyn crate::readers::FileAwareRead>> {
    if config.input.files.is_empty() {
        // For stdin, we don't have filename information
        // We'll need to create a wrapper that implements FileAwareRead
        Err(anyhow::anyhow!("File-aware reader not supported for stdin"))
    } else {
        let sorted_files = sort_files(&config.input.files, &config.input.file_order)?;
        Ok(Box::new(crate::readers::FileAwareMultiFileReader::new(
            sorted_files,
        )?))
    }
}

/// Sort files according to the specified file order
pub fn sort_files(files: &[String], order: &crate::config::FileOrder) -> Result<Vec<String>> {
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
