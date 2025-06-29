use anyhow::Result;
use crate::event::Event;
use crate::engine::RhaiEngine;
use super::{ScriptStage, ScriptResult, PipelineContext};

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
                available_keys.iter()
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
        
        ScriptResult::Emit(event)
    }
}