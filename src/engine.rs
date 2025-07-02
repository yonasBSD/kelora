use anyhow::{Context, Result};
use rhai::{Dynamic, Engine, Scope, AST};
use std::collections::HashMap;

use crate::event::Event;
use crate::rhai_functions;

#[derive(Clone)]
pub struct CompiledExpression {
    ast: AST,
    expr: String,
}

pub struct RhaiEngine {
    engine: Engine,
    compiled_filters: Vec<CompiledExpression>,
    compiled_execs: Vec<CompiledExpression>,
    compiled_begin: Option<CompiledExpression>,
    compiled_end: Option<CompiledExpression>,
    scope_template: Scope<'static>,
}

impl Clone for RhaiEngine {
    fn clone(&self) -> Self {
        let mut engine = Engine::new();
        engine.set_optimization_level(rhai::OptimizationLevel::Simple);
        rhai_functions::register_all_functions(&mut engine);
        
        Self {
            engine,
            compiled_filters: self.compiled_filters.clone(),
            compiled_execs: self.compiled_execs.clone(),
            compiled_begin: self.compiled_begin.clone(),
            compiled_end: self.compiled_end.clone(),
            scope_template: self.scope_template.clone(),
        }
    }
}

impl RhaiEngine {
    // Thread-local state management functions
    pub fn set_thread_tracking_state(tracked: &HashMap<String, Dynamic>) {
        rhai_functions::tracking::set_thread_tracking_state(tracked);
    }

    pub fn get_thread_tracking_state() -> HashMap<String, Dynamic> {
        rhai_functions::tracking::get_thread_tracking_state()
    }

    pub fn new() -> Self {
        let mut engine = Engine::new();
        
        // Enable print statements (they output to stderr by default in Rhai)
        engine.set_optimization_level(rhai::OptimizationLevel::Simple);
        
        // Register custom functions for log analysis
        rhai_functions::register_all_functions(&mut engine);
        
        // Register variable access callback for tracking functions
        Self::register_variable_resolver(&mut engine);
        
        let mut scope_template = Scope::new();
        scope_template.push("line", "");
        scope_template.push("event", rhai::Map::new());
        scope_template.push("meta", rhai::Map::new());
        
        Self {
            engine,
            compiled_filters: Vec::new(),
            compiled_execs: Vec::new(),
            compiled_begin: None,
            compiled_end: None,
            scope_template,
        }
    }


    // Individual compilation methods for pipeline stages
    pub fn compile_filter(&mut self, filter: &str) -> Result<CompiledExpression> {
        let ast = self.engine.compile(filter)
            .with_context(|| format!("Failed to compile filter expression: {}", filter))?;
        Ok(CompiledExpression {
            ast,
            expr: filter.to_string(),
        })
    }

    pub fn compile_exec(&mut self, exec: &str) -> Result<CompiledExpression> {
        let ast = self.engine.compile(exec)
            .with_context(|| format!("Failed to compile exec script: {}", exec))?;
        Ok(CompiledExpression {
            ast,
            expr: exec.to_string(),
        })
    }

    pub fn compile_begin(&mut self, begin: &str) -> Result<CompiledExpression> {
        let ast = self.engine.compile(begin)
            .with_context(|| format!("Failed to compile begin expression: {}", begin))?;
        Ok(CompiledExpression {
            ast,
            expr: begin.to_string(),
        })
    }

    pub fn compile_end(&mut self, end: &str) -> Result<CompiledExpression> {
        let ast = self.engine.compile(end)
            .with_context(|| format!("Failed to compile end expression: {}", end))?;
        Ok(CompiledExpression {
            ast,
            expr: end.to_string(),
        })
    }

    // Individual execution methods for pipeline stages
    pub fn execute_compiled_filter(&mut self, compiled: &CompiledExpression, event: &Event, tracked: &mut HashMap<String, Dynamic>) -> Result<bool> {
        Self::set_thread_tracking_state(tracked);
        let mut scope = self.create_scope_for_event(event);
        
        let result = self.engine.eval_ast_with_scope::<bool>(&mut scope, &compiled.ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute filter expression '{}': {}", compiled.expr, e))?;
        
        *tracked = Self::get_thread_tracking_state();
        Ok(result)
    }

    pub fn execute_compiled_exec(&mut self, compiled: &CompiledExpression, event: &mut Event, tracked: &mut HashMap<String, Dynamic>) -> Result<()> {
        Self::set_thread_tracking_state(tracked);
        let mut scope = self.create_scope_for_event(event);
        
        let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute exec script '{}': {}", compiled.expr, e))?;

        self.update_event_from_scope(event, &scope);
        *tracked = Self::get_thread_tracking_state();
        Ok(())
    }

    pub fn execute_compiled_begin(&mut self, compiled: &CompiledExpression, tracked: &mut HashMap<String, Dynamic>) -> Result<()> {
        Self::set_thread_tracking_state(tracked);
        let mut scope = self.scope_template.clone();
        
        let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute begin expression '{}': {}", compiled.expr, e))?;

        *tracked = Self::get_thread_tracking_state();
        Ok(())
    }

    pub fn execute_compiled_end(&self, compiled: &CompiledExpression, tracked: &HashMap<String, Dynamic>) -> Result<()> {
        let mut scope = self.scope_template.clone();
        let mut tracked_map = rhai::Map::new();
        
        // Convert HashMap to Rhai Map (read-only)
        for (k, v) in tracked.iter() {
            tracked_map.insert(k.clone().into(), v.clone());
        }
        scope.set_value("tracked", tracked_map);
        
        let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute end expression '{}': {}", compiled.expr, e))?;

        Ok(())
    }


    fn register_variable_resolver(_engine: &mut Engine) {
        // For now, keep this empty - we'll implement proper function-based approach
        // Variable resolver is not the right tool for function calls
    }

    #[allow(dead_code)]
    pub fn execute_begin(&mut self, tracked: &mut HashMap<String, Dynamic>) -> Result<()> {
        if let Some(compiled) = &self.compiled_begin {
            // Set thread-local state from tracked
            Self::set_thread_tracking_state(tracked);
            
            let mut scope = self.scope_template.clone();
            
            let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
                .map_err(|e| anyhow::anyhow!("Failed to execute begin expression '{}': {}", compiled.expr, e))?;

            // Update tracked from thread-local state
            *tracked = Self::get_thread_tracking_state();
        }

        Ok(())
    }



    #[allow(dead_code)]
    pub fn execute_end(&mut self, tracked: &HashMap<String, Dynamic>) -> Result<()> {
        if let Some(compiled) = &self.compiled_end {
            let mut scope = self.scope_template.clone();
            let mut tracked_map = rhai::Map::new();
            
            // Convert HashMap to Rhai Map (read-only)
            for (k, v) in tracked.iter() {
                tracked_map.insert(k.clone().into(), v.clone());
            }
            scope.set_value("tracked", tracked_map);
            
            let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
                .map_err(|e| anyhow::anyhow!("Failed to execute end expression '{}': {}", compiled.expr, e))?;
        }

        Ok(())
    }

    fn create_scope_for_event(&self, event: &Event) -> Scope {
        let mut scope = self.scope_template.clone();
        
        // Inject event fields as variables
        for (key, value) in &event.fields {
            if self.is_valid_identifier(key) {
                scope.push(key, value.clone());
            }
        }
        
        // Update built-in variables
        scope.set_value("line", event.original_line.clone());
        
        // Update event map for fields with invalid identifiers
        let mut event_map = rhai::Map::new();
        for (k, v) in &event.fields {
            event_map.insert(k.clone().into(), v.clone());
        }
        scope.set_value("event", event_map);
        
        // Update metadata
        let mut meta_map = rhai::Map::new();
        if let Some(line_num) = event.line_number {
            meta_map.insert("linenum".into(), Dynamic::from(line_num as i64));
        }
        if let Some(filename) = &event.filename {
            meta_map.insert("filename".into(), Dynamic::from(filename.clone()));
        }
        scope.set_value("meta", meta_map);
        
        scope
    }

    fn update_event_from_scope(&self, event: &mut Event, scope: &Scope) {
        for (name, _constant, value) in scope.iter() {
            if name != "line" && name != "event" && name != "meta" {
                event.fields.insert(name.to_string(), value.clone());
            }
        }
    }

    fn is_valid_identifier(&self, name: &str) -> bool {
        name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') &&
        name.chars().all(|c| c.is_alphanumeric() || c == '_')
    }


}

