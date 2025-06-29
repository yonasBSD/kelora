use anyhow::Result;
use rhai::Dynamic;
use std::collections::HashMap;

use crate::event::Event;
use crate::engine::RhaiEngine;

/// Core pipeline result types
#[derive(Debug, Clone)]
pub enum ScriptResult {
    Skip,
    Emit(Event),
    EmitMultiple(Vec<Event>), // For future emit_each() support
    Error(String),
}

impl ScriptResult {
    /// Unwrap the event from Emit variant, panics if not Emit
    pub fn unwrap_emit(self) -> Event {
        match self {
            ScriptResult::Emit(event) => event,
            _ => panic!("Expected ScriptResult::Emit"),
        }
    }
}

/// Shared context passed between pipeline stages
pub struct PipelineContext {
    pub config: PipelineConfig,
    pub tracker: HashMap<String, Dynamic>,
    pub window: Vec<Event>, // window[0] = current event, rest are previous
    pub rhai: RhaiEngine,
    pub meta: MetaData,
}

/// Pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub on_error: crate::ErrorStrategy,
    pub keys: Vec<String>,
    pub plain: bool,
    pub no_inject_fields: bool,
    pub inject_prefix: Option<String>,
}

/// Metadata about current processing context
#[derive(Debug, Clone, Default)]
pub struct MetaData {
    pub filename: Option<String>,
    pub line_number: Option<usize>,
}

/// Core pipeline traits

/// Parse raw text lines into structured events
pub trait EventParser: Send + Sync {
    fn parse(&self, line: &str) -> Result<Event>;
}

/// Optional line-level filtering before parsing
pub trait LineFilter: Send {
    fn should_keep(&self, line: &str) -> bool;
}

/// Handle multi-line log records (future feature)
pub trait Chunker: Send {
    fn feed_line(&mut self, line: String) -> Option<String>;
    fn flush(&mut self) -> Option<String>;
}

/// Manage sliding window of events (future feature)
pub trait WindowManager: Send {
    fn get_window(&self) -> Vec<Event>; // includes current as window[0]
    fn update(&mut self, current: &Event);
}

/// Core script processing stage (filters, execs, etc.)
pub trait ScriptStage: Send {
    fn apply(&mut self, event: Event, ctx: &mut PipelineContext) -> ScriptResult;
}

/// Optional event limiting (--take N)
pub trait EventLimiter: Send {
    fn allow(&mut self) -> bool;
}

/// Format events for output
pub trait Formatter: Send + Sync {
    fn format(&self, event: &Event) -> String;
}

/// Write formatted output
pub trait OutputWriter: Send {
    fn write(&mut self, line: &str) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()>;
}

/// Main pipeline structure
pub struct Pipeline {
    pub line_filter: Option<Box<dyn LineFilter>>,
    pub chunker: Box<dyn Chunker>,
    pub parser: Box<dyn EventParser>,
    pub script_stages: Vec<Box<dyn ScriptStage>>,
    pub limiter: Option<Box<dyn EventLimiter>>,
    pub formatter: Box<dyn Formatter>,
    pub output: Box<dyn OutputWriter>,
    pub window_manager: Box<dyn WindowManager>,
}

impl Pipeline {
    /// Process a single line through the entire pipeline
    /// This is the core method used by both sequential and parallel processing
    pub fn process_line(&mut self, line: String, ctx: &mut PipelineContext) -> Result<Vec<String>> {
        let mut results = Vec::new();
        
        // Line filter stage
        if let Some(filter) = &self.line_filter {
            if !filter.should_keep(&line) {
                return Ok(results);
            }
        }

        // Chunker stage (for multi-line records)
        if let Some(chunk) = self.chunker.feed_line(line) {
            // Parse stage
            let event = match self.parser.parse(&chunk) {
                Ok(e) => e,
                Err(err) => {
                    return match ctx.config.on_error {
                        crate::ErrorStrategy::Skip => Ok(results),
                        crate::ErrorStrategy::FailFast => Err(err),
                        crate::ErrorStrategy::EmitErrors => {
                            eprintln!("Parse error: {}", err);
                            Ok(results)
                        }
                        crate::ErrorStrategy::DefaultValue => {
                            Ok(vec![self.formatter.format(&Event::default_with_line(chunk))])
                        }
                    };
                }
            };

            // Update window manager
            self.window_manager.update(&event);
            ctx.window = self.window_manager.get_window();

            // Apply script stages (filters, execs, etc.)
            let mut result = ScriptResult::Emit(event);
            
            for stage in &mut self.script_stages {
                result = match result {
                    ScriptResult::Emit(event) => stage.apply(event, ctx),
                    ScriptResult::EmitMultiple(events) => {
                        // Process each event through remaining stages
                        let mut multi_results = Vec::new();
                        for event in events {
                            match stage.apply(event, ctx) {
                                ScriptResult::Emit(e) => multi_results.push(e),
                                ScriptResult::EmitMultiple(mut es) => multi_results.append(&mut es),
                                ScriptResult::Skip => {}
                                ScriptResult::Error(msg) => {
                                    return match ctx.config.on_error {
                                        crate::ErrorStrategy::Skip => Ok(results),
                                        crate::ErrorStrategy::FailFast => Err(anyhow::anyhow!(msg)),
                                        crate::ErrorStrategy::EmitErrors => {
                                            eprintln!("Script error: {}", msg);
                                            Ok(results)
                                        }
                                        crate::ErrorStrategy::DefaultValue => Ok(results),
                                    };
                                }
                            }
                        }
                        ScriptResult::EmitMultiple(multi_results)
                    }
                    other => other, // Skip or Error, stop processing
                };

                match &result {
                    ScriptResult::Skip | ScriptResult::Error(_) => break,
                    _ => {}
                }
            }

            // Handle final result
            match result {
                ScriptResult::Emit(mut event) => {
                    if self.limiter.as_mut().map_or(true, |l| l.allow()) {
                        self.apply_field_filtering(&mut event, ctx);
                        results.push(self.formatter.format(&event));
                    }
                }
                ScriptResult::EmitMultiple(events) => {
                    for mut event in events {
                        if self.limiter.as_mut().map_or(true, |l| l.allow()) {
                            self.apply_field_filtering(&mut event, ctx);
                            results.push(self.formatter.format(&event));
                        }
                    }
                }
                ScriptResult::Skip => {}
                ScriptResult::Error(msg) => {
                    return match ctx.config.on_error {
                        crate::ErrorStrategy::Skip => Ok(results),
                        crate::ErrorStrategy::FailFast => Err(anyhow::anyhow!(msg)),
                        crate::ErrorStrategy::EmitErrors => {
                            eprintln!("Script error: {}", msg);
                            Ok(results)
                        }
                        crate::ErrorStrategy::DefaultValue => Ok(results),
                    };
                }
            }
        }

        Ok(results)
    }

    /// Apply field filtering based on --keys option
    fn apply_field_filtering(&self, event: &mut Event, ctx: &PipelineContext) {
        if !ctx.config.keys.is_empty() {
            event.filter_keys(&ctx.config.keys);
        }
    }

    /// Flush any remaining chunks from the chunker
    pub fn flush(&mut self, ctx: &mut PipelineContext) -> Result<Vec<String>> {
        if let Some(chunk) = self.chunker.flush() {
            self.process_line(chunk, ctx)
        } else {
            Ok(Vec::new())
        }
    }

    /// Clone pipeline for parallel processing (each worker gets its own instance)
    pub fn clone_for_worker(&self) -> Result<Pipeline> {
        // This will be implemented when we refactor the existing components
        todo!("Pipeline cloning for parallel processing")
    }
}

/// Default implementations for pipeline stages

/// Simple pass-through chunker (no multi-line support)
pub struct SimpleChunker;

impl Chunker for SimpleChunker {
    fn feed_line(&mut self, line: String) -> Option<String> {
        Some(line)
    }

    fn flush(&mut self) -> Option<String> {
        None
    }
}

/// Simple window manager (no windowing support)
pub struct SimpleWindowManager {
    current: Option<Event>,
}

impl SimpleWindowManager {
    pub fn new() -> Self {
        Self { current: None }
    }
}

impl WindowManager for SimpleWindowManager {
    fn get_window(&self) -> Vec<Event> {
        if let Some(ref event) = self.current {
            vec![event.clone()]
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, current: &Event) {
        self.current = Some(current.clone());
    }
}

/// Standard output writer
pub struct StdoutWriter;

impl OutputWriter for StdoutWriter {
    fn write(&mut self, line: &str) -> std::io::Result<()> {
        println!("{}", line);
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        use std::io::Write;
        std::io::stdout().flush()
    }
}

/// Filter stage implementation
pub struct FilterStage {
    compiled_filters: Vec<crate::engine::CompiledExpression>,
}

impl FilterStage {
    pub fn new(filters: Vec<String>, engine: &mut RhaiEngine) -> Result<Self> {
        let mut compiled_filters = Vec::new();
        for filter in filters {
            let compiled = engine.compile_filter(&filter)?;
            compiled_filters.push(compiled);
        }
        Ok(Self { compiled_filters })
    }
}

impl ScriptStage for FilterStage {
    fn apply(&mut self, event: Event, ctx: &mut PipelineContext) -> ScriptResult {
        if self.compiled_filters.is_empty() {
            return ScriptResult::Emit(event);
        }

        // Execute all filters - if any returns false, skip the event
        for compiled_filter in &self.compiled_filters {
            match ctx.rhai.execute_compiled_filter(compiled_filter, &event, &mut ctx.tracker) {
                Ok(result) => {
                    if !result {
                        return ScriptResult::Skip;
                    }
                }
                Err(e) => {
                    return ScriptResult::Error(format!("Filter error: {}", e));
                }
            }
        }

        ScriptResult::Emit(event)
    }
}

/// Exec stage implementation
pub struct ExecStage {
    compiled_execs: Vec<crate::engine::CompiledExpression>,
}

impl ExecStage {
    pub fn new(execs: Vec<String>, engine: &mut RhaiEngine) -> Result<Self> {
        let mut compiled_execs = Vec::new();
        for exec in execs {
            let compiled = engine.compile_exec(&exec)?;
            compiled_execs.push(compiled);
        }
        Ok(Self { compiled_execs })
    }
}

impl ScriptStage for ExecStage {
    fn apply(&mut self, mut event: Event, ctx: &mut PipelineContext) -> ScriptResult {
        if self.compiled_execs.is_empty() {
            return ScriptResult::Emit(event);
        }

        // Execute all exec scripts in sequence
        for compiled_exec in &self.compiled_execs {
            match ctx.rhai.execute_compiled_exec(compiled_exec, &mut event, &mut ctx.tracker) {
                Ok(()) => {}
                Err(e) => {
                    return ScriptResult::Error(format!("Exec error: {}", e));
                }
            }
        }

        ScriptResult::Emit(event)
    }
}

/// Begin stage for --begin expressions
pub struct BeginStage {
    compiled_begin: Option<crate::engine::CompiledExpression>,
}

impl BeginStage {
    pub fn new(begin: Option<String>, engine: &mut RhaiEngine) -> Result<Self> {
        let compiled_begin = if let Some(begin_expr) = begin {
            Some(engine.compile_begin(&begin_expr)?)
        } else {
            None
        };
        Ok(Self { compiled_begin })
    }

    pub fn execute(&self, ctx: &mut PipelineContext) -> Result<()> {
        if let Some(ref compiled) = self.compiled_begin {
            ctx.rhai.execute_compiled_begin(compiled, &mut ctx.tracker)
        } else {
            Ok(())
        }
    }
}

/// End stage for --end expressions
pub struct EndStage {
    compiled_end: Option<crate::engine::CompiledExpression>,
}

impl EndStage {
    pub fn new(end: Option<String>, engine: &mut RhaiEngine) -> Result<Self> {
        let compiled_end = if let Some(end_expr) = end {
            Some(engine.compile_end(&end_expr)?)
        } else {
            None
        };
        Ok(Self { compiled_end })
    }

    pub fn execute(&self, ctx: &PipelineContext) -> Result<()> {
        if let Some(ref compiled) = self.compiled_end {
            ctx.rhai.execute_compiled_end(compiled, &ctx.tracker)
        } else {
            Ok(())
        }
    }
}

/// Simple event limiter for --take N
pub struct TakeNLimiter {
    remaining: usize,
}

impl TakeNLimiter {
    pub fn new(limit: usize) -> Self {
        Self { remaining: limit }
    }
}

impl EventLimiter for TakeNLimiter {
    fn allow(&mut self) -> bool {
        if self.remaining > 0 {
            self.remaining -= 1;
            true
        } else {
            false
        }
    }
}

/// Pipeline builder for easy construction from CLI arguments
pub struct PipelineBuilder {
    config: PipelineConfig,
    filters: Vec<String>,
    execs: Vec<String>,
    begin: Option<String>,
    end: Option<String>,
    input_format: crate::InputFormat,
    output_format: crate::OutputFormat,
    take_limit: Option<usize>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Self {
            config: PipelineConfig {
                on_error: crate::ErrorStrategy::EmitErrors,
                keys: Vec::new(),
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

    pub fn with_take_limit(mut self, limit: Option<usize>) -> Self {
        self.take_limit = limit;
        self
    }

    pub fn build(self) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
        let mut rhai_engine = RhaiEngine::new();

        // Create parser
        let parser: Box<dyn EventParser> = match self.input_format {
            crate::InputFormat::Jsonl => Box::new(crate::parsers::JsonlParser::new()),
            crate::InputFormat::Line => Box::new(crate::parsers::LineParser::new()),
            crate::InputFormat::Logfmt => Box::new(crate::parsers::LogfmtParser::new()),
            crate::InputFormat::Csv => return Err(anyhow::anyhow!("CSV parser not implemented yet")),
            crate::InputFormat::Apache => return Err(anyhow::anyhow!("Apache parser not implemented yet")),
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
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a pipeline from CLI arguments
pub fn create_pipeline_from_cli(cli: &crate::Cli) -> Result<(Pipeline, BeginStage, EndStage, PipelineContext)> {
    let config = PipelineConfig {
        on_error: cli.on_error.clone(),
        keys: cli.keys.clone(),
        plain: cli.plain,
        no_inject_fields: cli.no_inject_fields,
        inject_prefix: cli.inject_prefix.clone(),
    };

    PipelineBuilder::new()
        .with_config(config)
        .with_filters(cli.filters.clone())
        .with_execs(cli.execs.clone())
        .with_begin(cli.begin.clone())
        .with_end(cli.end.clone())
        .with_input_format(cli.format.clone())
        .with_output_format(cli.output_format.clone())
        .build()
}