use super::{PipelineContext, ScriptResult, ScriptStage};
use crate::config::TimestampFilterConfig;
use crate::engine::RhaiEngine;
use crate::event::Event;
use anyhow::Result;

/// Filter stage implementation
pub struct FilterStage {
    compiled_filter: crate::engine::CompiledExpression,
}

impl FilterStage {
    pub fn new(filter: String, engine: &mut RhaiEngine) -> Result<Self> {
        let compiled_filter = engine.compile_filter(&filter)?;
        Ok(Self { compiled_filter })
    }
}

impl ScriptStage for FilterStage {
    fn apply(&mut self, event: Event, ctx: &mut PipelineContext) -> ScriptResult {
        let result = if ctx.window.is_empty() {
            // No window context - use standard method
            ctx.rhai
                .execute_compiled_filter(&self.compiled_filter, &event, &mut ctx.tracker)
        } else {
            // Window context available - use window-aware method
            ctx.rhai.execute_compiled_filter_with_window(
                &self.compiled_filter,
                &event,
                &ctx.window,
                &mut ctx.tracker,
            )
        };

        match result {
            Ok(result) => {
                if result {
                    ScriptResult::Emit(event)
                } else {
                    ScriptResult::Skip
                }
            }
            Err(e) => {
                // Track error for reporting even in resilient mode
                crate::rhai_functions::tracking::track_error(
                    "rhai",
                    ctx.meta.line_number,
                    &format!("Filter error: {}", e),
                    ctx.config.verbose,
                    ctx.config.quiet,
                    Some(&ctx.config),
                );

                // New resiliency model: filter errors evaluate to false (Skip)
                // unless in strict mode, where they still propagate as errors
                if ctx.config.strict {
                    ScriptResult::Error(format!("Filter error: {}", e))
                } else {
                    ScriptResult::Skip
                }
            }
        }
    }
}

/// Exec stage implementation
pub struct ExecStage {
    compiled_exec: crate::engine::CompiledExpression,
}

impl ExecStage {
    pub fn new(exec: String, engine: &mut RhaiEngine) -> Result<Self> {
        let compiled_exec = engine.compile_exec(&exec)?;
        Ok(Self { compiled_exec })
    }
}

impl ScriptStage for ExecStage {
    fn apply(&mut self, event: Event, ctx: &mut PipelineContext) -> ScriptResult {
        // Atomic execution: work on a copy of the event for rollback behavior
        let mut event_copy = event.clone();

        let result = if ctx.window.is_empty() {
            // No window context - use standard method
            ctx.rhai
                .execute_compiled_exec(&self.compiled_exec, &mut event_copy, &mut ctx.tracker)
        } else {
            // Window context available - use window-aware method
            ctx.rhai.execute_compiled_exec_with_window(
                &self.compiled_exec,
                &mut event_copy,
                &ctx.window,
                &mut ctx.tracker,
            )
        };

        match result {
            Ok(()) => {
                // Success: commit the modified event
                ScriptResult::Emit(event_copy)
            }
            Err(e) => {
                // Track error for reporting even in resilient mode
                crate::rhai_functions::tracking::track_error(
                    "rhai",
                    ctx.meta.line_number,
                    &format!("Exec error: {}", e),
                    ctx.config.verbose,
                    ctx.config.quiet,
                    Some(&ctx.config),
                );

                // New resiliency model: atomic rollback - return original event unchanged
                // unless in strict mode, where errors still propagate
                if ctx.config.strict {
                    ScriptResult::Error(format!("Exec error: {}", e))
                } else {
                    // Rollback: return original event unchanged
                    ScriptResult::Emit(event)
                }
            }
        }
    }
}

/// Begin stage for --begin expressions
#[allow(dead_code)] // Used by builders.rs, instantiated in build() method
pub struct BeginStage {
    compiled_begin: Option<crate::engine::CompiledExpression>,
}

impl BeginStage {
    #[allow(dead_code)] // Used by builders.rs in build() method
    pub fn new(begin: Option<String>, engine: &mut RhaiEngine) -> Result<Self> {
        let compiled_begin = if let Some(begin_expr) = begin {
            Some(engine.compile_begin(&begin_expr)?)
        } else {
            None
        };
        Ok(Self { compiled_begin })
    }

    #[allow(dead_code)] // Used by sequential processing pipeline
    pub fn execute(&self, ctx: &mut PipelineContext) -> Result<()> {
        if let Some(ref compiled) = self.compiled_begin {
            ctx.rhai.execute_compiled_begin(compiled, &mut ctx.tracker)
        } else {
            Ok(())
        }
    }
}

/// End stage for --end expressions
#[allow(dead_code)] // Used by builders.rs, instantiated in build() method
pub struct EndStage {
    compiled_end: Option<crate::engine::CompiledExpression>,
}

impl EndStage {
    #[allow(dead_code)] // Used by builders.rs in build() method
    pub fn new(end: Option<String>, engine: &mut RhaiEngine) -> Result<Self> {
        let compiled_end = if let Some(end_expr) = end {
            Some(engine.compile_end(&end_expr)?)
        } else {
            None
        };
        Ok(Self { compiled_end })
    }

    #[allow(dead_code)] // Used by sequential processing pipeline
    pub fn execute(&self, ctx: &PipelineContext) -> Result<()> {
        if let Some(ref compiled) = self.compiled_end {
            ctx.rhai.execute_compiled_end(compiled, &ctx.tracker)
        } else {
            Ok(())
        }
    }
}

/// Level filtering stage for --levels and --exclude-levels options
pub struct LevelFilterStage {
    levels: Vec<String>,
    exclude_levels: Vec<String>,
}

impl LevelFilterStage {
    pub fn new(levels: Vec<String>, exclude_levels: Vec<String>) -> Self {
        Self {
            levels,
            exclude_levels,
        }
    }

    /// Check if any filtering is needed
    pub fn is_active(&self) -> bool {
        !self.levels.is_empty() || !self.exclude_levels.is_empty()
    }
}

impl ScriptStage for LevelFilterStage {
    fn apply(&mut self, event: Event, _ctx: &mut PipelineContext) -> ScriptResult {
        if !self.is_active() {
            return ScriptResult::Emit(event);
        }

        // Get the level from the event fields map - check all possible level field names
        let event_level = {
            let mut found_level: Option<String> = None;

            // Check all known level field names in the event's fields map
            for level_field_name in crate::event::LEVEL_FIELD_NAMES {
                if let Some(value) = event.fields.get(*level_field_name) {
                    if let Ok(level_str) = value.clone().into_string() {
                        found_level = Some(level_str);
                        break;
                    }
                }
            }

            match found_level {
                Some(level) => level,
                None => {
                    // If no level field is found, check if we should include or exclude
                    if self.levels.is_empty() {
                        // Only exclude_levels specified, and no level found - include by default
                        return ScriptResult::Emit(event);
                    } else {
                        // levels specified but no level found - exclude
                        return ScriptResult::Skip;
                    }
                }
            }
        };

        // Apply exclude_levels first (higher priority) - case-insensitive
        if !self.exclude_levels.is_empty() {
            for exclude_level in &self.exclude_levels {
                if event_level.eq_ignore_ascii_case(exclude_level) {
                    return ScriptResult::Skip;
                }
            }
        }

        // Apply levels filter - case-insensitive
        if !self.levels.is_empty() {
            for level in &self.levels {
                if event_level.eq_ignore_ascii_case(level) {
                    return ScriptResult::Emit(event);
                }
            }
            // No match found in levels list - exclude
            return ScriptResult::Skip;
        }

        // No levels specified, only exclude_levels - include by default
        ScriptResult::Emit(event)
    }
}

/// Key filtering stage for --keys and --exclude-keys options
pub struct KeyFilterStage {
    keys: Vec<String>,
    exclude_keys: Vec<String>,
}

impl KeyFilterStage {
    pub fn new(keys: Vec<String>, exclude_keys: Vec<String>) -> Self {
        Self { keys, exclude_keys }
    }

    /// Check if any filtering is needed
    pub fn is_active(&self) -> bool {
        !self.keys.is_empty() || !self.exclude_keys.is_empty()
    }
}

impl ScriptStage for KeyFilterStage {
    fn apply(&mut self, mut event: Event, _ctx: &mut PipelineContext) -> ScriptResult {
        if !self.is_active() {
            return ScriptResult::Emit(event);
        }

        // Get available keys from the event
        let available_keys: Vec<String> = event.fields.keys().cloned().collect();

        // Calculate effective keys using the same logic as before
        let effective_keys = {
            let mut result_keys = if self.keys.is_empty() {
                // If no keys specified, start with all available keys
                available_keys
            } else {
                // If keys specified, filter available keys to only include those
                available_keys
                    .iter()
                    .filter(|key| self.keys.contains(key))
                    .cloned()
                    .collect()
            };

            // Apply exclusions (higher priority)
            result_keys.retain(|key| !self.exclude_keys.contains(key));

            result_keys
        };

        // Apply the filtering
        event.filter_keys(&effective_keys);

        // If any key filtering was applied and no fields remain, skip this event
        if self.is_active() && event.fields.is_empty() {
            ScriptResult::Skip
        } else {
            ScriptResult::Emit(event)
        }
    }
}

/// Timestamp filter stage for --since and --until filtering
pub struct TimestampFilterStage {
    config: TimestampFilterConfig,
}

impl TimestampFilterStage {
    pub fn new(config: TimestampFilterConfig) -> Self {
        Self { config }
    }
}

impl ScriptStage for TimestampFilterStage {
    fn apply(&mut self, event: Event, ctx: &mut PipelineContext) -> ScriptResult {
        // Get the parsed timestamp from the event
        let event_timestamp = match event.parsed_ts {
            Some(ts) => ts,
            None => {
                // No timestamp available - use new resiliency model
                if ctx.config.strict {
                    // Stop processing on missing timestamp in strict mode
                    return ScriptResult::Error(
                        "Event has no valid timestamp for --since/--until filtering".to_string(),
                    );
                } else {
                    // Filter out events without valid timestamps (resilient mode)
                    return ScriptResult::Skip;
                }
            }
        };

        // Check since filter (event must be >= since)
        if let Some(since) = self.config.since {
            if event_timestamp < since {
                return ScriptResult::Skip;
            }
        }

        // Check until filter (event must be <= until)
        if let Some(until) = self.config.until {
            if event_timestamp > until {
                return ScriptResult::Skip;
            }
        }

        // Event is within the time range
        ScriptResult::Emit(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TimestampFilterConfig;
    use crate::pipeline::{MetaData, PipelineConfig};
    use chrono::{Duration, Utc};

    #[test]
    fn test_timestamp_filter_stage_since() {
        let since = Utc::now() - Duration::hours(1);
        let config = TimestampFilterConfig {
            since: Some(since),
            until: None,
        };
        let mut stage = TimestampFilterStage::new(config);

        // Create dummy context
        let mut ctx = PipelineContext {
            config: PipelineConfig {
                error_report: crate::config::ErrorReportConfig {
                    style: crate::config::ErrorReportStyle::Summary,
                    file: None,
                },
                brief: false,
                color_mode: crate::config::ColorMode::Auto,
                timestamp_formatting: crate::config::TimestampFormatConfig::default(),
                strict: false,
                verbose: false,
                quiet: false,
                no_emoji: false,
            },
            tracker: std::collections::HashMap::new(),
            window: Vec::new(),
            rhai: crate::engine::RhaiEngine::new(),
            meta: MetaData::default(),
        };

        // Test event before since time (should be skipped)
        let old_event = crate::event::Event {
            parsed_ts: Some(since - Duration::minutes(30)),
            ..Default::default()
        };

        let result = stage.apply(old_event, &mut ctx);
        matches!(result, ScriptResult::Skip);

        // Test event after since time (should be emitted)
        let new_event = crate::event::Event {
            parsed_ts: Some(since + Duration::minutes(30)),
            ..Default::default()
        };

        let result = stage.apply(new_event, &mut ctx);
        matches!(result, ScriptResult::Emit(_));
    }

    #[test]
    fn test_timestamp_filter_stage_until() {
        let until = Utc::now() - Duration::hours(1);
        let config = TimestampFilterConfig {
            since: None,
            until: Some(until),
        };
        let mut stage = TimestampFilterStage::new(config);

        // Create dummy context
        let mut ctx = PipelineContext {
            config: PipelineConfig {
                error_report: crate::config::ErrorReportConfig {
                    style: crate::config::ErrorReportStyle::Summary,
                    file: None,
                },
                brief: false,
                color_mode: crate::config::ColorMode::Auto,
                timestamp_formatting: crate::config::TimestampFormatConfig::default(),
                strict: false,
                verbose: false,
                quiet: false,
                no_emoji: false,
            },
            tracker: std::collections::HashMap::new(),
            window: Vec::new(),
            rhai: crate::engine::RhaiEngine::new(),
            meta: MetaData::default(),
        };

        // Test event before until time (should be emitted)
        let old_event = crate::event::Event {
            parsed_ts: Some(until - Duration::minutes(30)),
            ..Default::default()
        };

        let result = stage.apply(old_event, &mut ctx);
        matches!(result, ScriptResult::Emit(_));

        // Test event after until time (should be skipped)
        let new_event = crate::event::Event {
            parsed_ts: Some(until + Duration::minutes(30)),
            ..Default::default()
        };

        let result = stage.apply(new_event, &mut ctx);
        matches!(result, ScriptResult::Skip);
    }

    #[test]
    fn test_timestamp_filter_stage_no_timestamp() {
        let config = TimestampFilterConfig {
            since: Some(Utc::now() - Duration::hours(1)),
            until: Some(Utc::now() + Duration::hours(1)),
        };
        let mut stage = TimestampFilterStage::new(config);

        // Create dummy context
        let mut ctx = PipelineContext {
            config: PipelineConfig {
                error_report: crate::config::ErrorReportConfig {
                    style: crate::config::ErrorReportStyle::Summary,
                    file: None,
                },
                brief: false,
                color_mode: crate::config::ColorMode::Auto,
                timestamp_formatting: crate::config::TimestampFormatConfig::default(),
                strict: false,
                verbose: false,
                quiet: false,
                no_emoji: false,
            },
            tracker: std::collections::HashMap::new(),
            window: Vec::new(),
            rhai: crate::engine::RhaiEngine::new(),
            meta: MetaData::default(),
        };

        // Test event without timestamp (should be emitted - pass through behavior)
        let event_no_ts = crate::event::Event {
            parsed_ts: None,
            ..Default::default()
        };

        let result = stage.apply(event_no_ts, &mut ctx);
        matches!(result, ScriptResult::Emit(_));
    }
}
