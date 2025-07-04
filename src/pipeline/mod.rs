use anyhow::Result;
use rhai::Dynamic;
use std::collections::HashMap;

use crate::engine::RhaiEngine;
use crate::event::Event;

// Re-export submodules
pub mod builders;
pub mod defaults;
pub mod multiline;
pub mod stages;

// Re-export main types for convenience
pub use builders::*;
pub use defaults::*;
pub use multiline::*;
pub use stages::*;

/// Core pipeline result types
#[derive(Debug, Clone)]
pub enum ScriptResult {
    Skip,
    Emit(Event),
    #[allow(dead_code)]
    EmitMultiple(Vec<Event>), // For future emit_each() support
    Error(String),
}

impl ScriptResult {
    /// Unwrap the event from Emit variant, panics if not Emit
    #[allow(dead_code)]
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
    pub brief: bool,
    #[allow(dead_code)]
    pub no_inject_fields: bool,
    #[allow(dead_code)]
    pub inject_prefix: Option<String>,
    pub color_mode: crate::config::ColorMode,
}

/// Metadata about current processing context
#[derive(Debug, Clone, Default)]
pub struct MetaData {
    #[allow(dead_code)]
    pub filename: Option<String>,
    pub line_number: Option<usize>,
}

/// Core pipeline traits
///
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
#[allow(dead_code)]
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
    #[allow(dead_code)]
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
                Ok(e) => {
                    // Event was successfully created from chunk
                    crate::stats::stats_add_event_created();

                    // Also track in Rhai context for parallel processing
                    if !ctx.tracker.is_empty() {
                        ctx.tracker
                            .entry("__kelora_stats_events_created".to_string())
                            .and_modify(|v| *v = rhai::Dynamic::from(v.as_int().unwrap_or(0) + 1))
                            .or_insert(rhai::Dynamic::from(1i64));
                        ctx.tracker.insert(
                            "__op___kelora_stats_events_created".to_string(),
                            rhai::Dynamic::from("count"),
                        );
                    }

                    e
                }
                Err(err) => {
                    return match ctx.config.on_error {
                        crate::ErrorStrategy::Skip => Ok(results),
                        crate::ErrorStrategy::Abort => Err(err),
                        crate::ErrorStrategy::Print => {
                            eprintln!(
                                "{}",
                                crate::config::format_error_message_auto(&format!(
                                    "Parse error: {}",
                                    err
                                ))
                            );
                            Ok(results)
                        }
                        crate::ErrorStrategy::Stub => Ok(vec![self
                            .formatter
                            .format(&Event::default_with_line(chunk))]),
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
                                        crate::ErrorStrategy::Abort => Err(anyhow::anyhow!(msg)),
                                        crate::ErrorStrategy::Print => {
                                            eprintln!(
                                                "{}",
                                                crate::config::format_error_message_auto(&format!(
                                                    "Script error: {}",
                                                    msg
                                                ))
                                            );
                                            Ok(results)
                                        }
                                        crate::ErrorStrategy::Stub => Ok(results),
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
                ScriptResult::Emit(event) => {
                    if self.limiter.as_mut().map_or(true, |l| l.allow()) {
                        crate::stats::stats_add_event_output();

                        // Also track in Rhai context for parallel processing
                        if !ctx.tracker.is_empty() {
                            ctx.tracker
                                .entry("__kelora_stats_events_output".to_string())
                                .and_modify(|v| {
                                    *v = rhai::Dynamic::from(v.as_int().unwrap_or(0) + 1)
                                })
                                .or_insert(rhai::Dynamic::from(1i64));
                            ctx.tracker.insert(
                                "__op___kelora_stats_events_output".to_string(),
                                rhai::Dynamic::from("count"),
                            );
                        }

                        results.push(self.formatter.format(&event));
                    } else {
                        crate::stats::stats_add_event_filtered();

                        // Also track in Rhai context for parallel processing
                        if !ctx.tracker.is_empty() {
                            ctx.tracker
                                .entry("__kelora_stats_events_filtered".to_string())
                                .and_modify(|v| {
                                    *v = rhai::Dynamic::from(v.as_int().unwrap_or(0) + 1)
                                })
                                .or_insert(rhai::Dynamic::from(1i64));
                            ctx.tracker.insert(
                                "__op___kelora_stats_events_filtered".to_string(),
                                rhai::Dynamic::from("count"),
                            );
                        }
                    }
                }
                ScriptResult::EmitMultiple(events) => {
                    for event in events {
                        if self.limiter.as_mut().map_or(true, |l| l.allow()) {
                            crate::stats::stats_add_event_output();

                            // Also track in Rhai context for parallel processing
                            if !ctx.tracker.is_empty() {
                                ctx.tracker
                                    .entry("__kelora_stats_events_output".to_string())
                                    .and_modify(|v| {
                                        *v = rhai::Dynamic::from(v.as_int().unwrap_or(0) + 1)
                                    })
                                    .or_insert(rhai::Dynamic::from(1i64));
                                ctx.tracker.insert(
                                    "__op___kelora_stats_events_output".to_string(),
                                    rhai::Dynamic::from("count"),
                                );
                            }

                            results.push(self.formatter.format(&event));
                        } else {
                            crate::stats::stats_add_event_filtered();

                            // Also track in Rhai context for parallel processing
                            if !ctx.tracker.is_empty() {
                                ctx.tracker
                                    .entry("__kelora_stats_events_filtered".to_string())
                                    .and_modify(|v| {
                                        *v = rhai::Dynamic::from(v.as_int().unwrap_or(0) + 1)
                                    })
                                    .or_insert(rhai::Dynamic::from(1i64));
                                ctx.tracker.insert(
                                    "__op___kelora_stats_events_filtered".to_string(),
                                    rhai::Dynamic::from("count"),
                                );
                            }
                        }
                    }
                }
                ScriptResult::Skip => {
                    crate::stats::stats_add_event_filtered();

                    // Also track in Rhai context for parallel processing
                    if !ctx.tracker.is_empty() {
                        ctx.tracker
                            .entry("__kelora_stats_events_filtered".to_string())
                            .and_modify(|v| *v = rhai::Dynamic::from(v.as_int().unwrap_or(0) + 1))
                            .or_insert(rhai::Dynamic::from(1i64));
                        ctx.tracker.insert(
                            "__op___kelora_stats_events_filtered".to_string(),
                            rhai::Dynamic::from("count"),
                        );
                    }
                }
                ScriptResult::Error(msg) => {
                    return match ctx.config.on_error {
                        crate::ErrorStrategy::Skip => Ok(results),
                        crate::ErrorStrategy::Abort => Err(anyhow::anyhow!(msg)),
                        crate::ErrorStrategy::Print => {
                            eprintln!(
                                "{}",
                                crate::config::format_error_message_auto(&format!(
                                    "Script error: {}",
                                    msg
                                ))
                            );
                            Ok(results)
                        }
                        crate::ErrorStrategy::Stub => Ok(results),
                    };
                }
            }
        }

        Ok(results)
    }

    /// Flush any remaining chunks from the chunker
    pub fn flush(&mut self, ctx: &mut PipelineContext) -> Result<Vec<String>> {
        if let Some(chunk) = self.chunker.flush() {
            self.process_line(chunk, ctx)
        } else {
            Ok(Vec::new())
        }
    }
}
