use anyhow::{Context, Result};
use rhai::{Dynamic, Engine, Scope, AST, EvalAltResult};
use std::collections::HashMap;

use crate::event::Event;
use crate::rhai_functions;

use rhai::Map;

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

        // Apply the same on_print override as in new()
        engine.on_print(|text| {
            if crate::rhai_functions::strings::is_parallel_mode() {
                crate::rhai_functions::strings::capture_print(text.to_string());
            } else {
                println!("{}", text);
            }
        });

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

    fn format_rhai_error(err: Box<EvalAltResult>, script_name: &str, _script_text: &str) -> String {
        match *err {
            EvalAltResult::ErrorParsing(parse_err, pos) => {
                format!("Syntax error in {} at {}: {}", script_name, pos, parse_err)
            }
            EvalAltResult::ErrorRuntime(runtime_err, pos) => {
                format!("Runtime error in {} at {}: {}", script_name, pos, runtime_err)
            }
            EvalAltResult::ErrorVariableNotFound(var, pos) => {
                format!("Variable '{}' not found in {} at {}", var, script_name, pos)
            }
            EvalAltResult::ErrorFunctionNotFound(func, pos) => {
                format!("Function '{}' not found in {} at {}", func, script_name, pos)
            }
            EvalAltResult::ErrorPropertyNotFound(prop, pos) => {
                format!("Property '{}' not found in {} at {}", prop, script_name, pos)
            }
            EvalAltResult::ErrorIndexNotFound(index, pos) => {
                format!("Index '{}' not found in {} at {}", index, script_name, pos)
            }
            EvalAltResult::ErrorDotExpr(msg, pos) => {
                format!("Property access error in {} at {}: {}", script_name, pos, msg)
            }
            EvalAltResult::ErrorArithmetic(msg, pos) => {
                format!("Arithmetic error in {} at {}: {}", script_name, pos, msg)
            }
            EvalAltResult::ErrorTooManyOperations(pos) => {
                format!("Too many operations in {} at {}", script_name, pos)
            }
            EvalAltResult::ErrorStackOverflow(pos) => {
                format!("Stack overflow in {} at {}", script_name, pos)
            }
            EvalAltResult::ErrorDataTooLarge(msg, pos) => {
                format!("Data too large in {} at {}: {}", script_name, pos, msg)
            }
            EvalAltResult::ErrorTerminated(val, pos) => {
                format!("Script terminated in {} at {}: {}", script_name, pos, val)
            }
            _ => format!("Error in {}: {}", script_name, err)
        }
    }

    pub fn new() -> Self {
        let mut engine = Engine::new();

        engine.set_optimization_level(rhai::OptimizationLevel::Simple);

        // Override the built-in print function to support capture in parallel mode
        engine.on_print(|text| {
            if crate::rhai_functions::strings::is_parallel_mode() {
                crate::rhai_functions::strings::capture_print(text.to_string());
            } else {
                println!("{}", text);
            }
        });

        // Register custom functions for log analysis (includes eprint() for stderr output)
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
        let ast = self
            .engine
            .compile_expression(filter)
            .with_context(|| format!("Failed to compile filter expression: {}", filter))?;
        Ok(CompiledExpression {
            ast,
            expr: filter.to_string(),
        })
    }

    pub fn compile_exec(&mut self, exec: &str) -> Result<CompiledExpression> {
        let ast = self
            .engine
            .compile(exec)
            .with_context(|| format!("Failed to compile exec script: {}", exec))?;
        Ok(CompiledExpression {
            ast,
            expr: exec.to_string(),
        })
    }

    pub fn compile_begin(&mut self, begin: &str) -> Result<CompiledExpression> {
        let ast = self
            .engine
            .compile(begin)
            .with_context(|| format!("Failed to compile begin expression: {}", begin))?;
        Ok(CompiledExpression {
            ast,
            expr: begin.to_string(),
        })
    }

    pub fn compile_end(&mut self, end: &str) -> Result<CompiledExpression> {
        let ast = self
            .engine
            .compile(end)
            .with_context(|| format!("Failed to compile end expression: {}", end))?;
        Ok(CompiledExpression {
            ast,
            expr: end.to_string(),
        })
    }

    // Individual execution methods for pipeline stages
    pub fn execute_compiled_filter(
        &mut self,
        compiled: &CompiledExpression,
        event: &Event,
        tracked: &mut HashMap<String, Dynamic>,
    ) -> Result<bool> {
        Self::set_thread_tracking_state(tracked);
        let mut scope = self.create_scope_for_event(event);

        let result = self
            .engine
            .eval_expression_with_scope::<bool>(&mut scope, &compiled.expr)
            .map_err(|e| {
                let detailed_msg = Self::format_rhai_error(e, "filter expression", &compiled.expr);
                anyhow::anyhow!("{}", detailed_msg)
            })?;

        *tracked = Self::get_thread_tracking_state();
        Ok(result)
    }

    pub fn execute_compiled_exec(
        &mut self,
        compiled: &CompiledExpression,
        event: &mut Event,
        tracked: &mut HashMap<String, Dynamic>,
    ) -> Result<()> {
        Self::set_thread_tracking_state(tracked);
        let mut scope = self.create_scope_for_event(event);

        let _ = self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
            .map_err(|e| {
                let detailed_msg = Self::format_rhai_error(e, "exec script", &compiled.expr);
                anyhow::anyhow!("{}", detailed_msg)
            })?;

        self.update_event_from_scope(event, &scope);
        *tracked = Self::get_thread_tracking_state();
        Ok(())
    }

    pub fn execute_compiled_begin(
        &mut self,
        compiled: &CompiledExpression,
        tracked: &mut HashMap<String, Dynamic>,
    ) -> Result<()> {
        Self::set_thread_tracking_state(tracked);
        let mut scope = self.scope_template.clone();

        let _ = self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
            .map_err(|e| {
                let detailed_msg = Self::format_rhai_error(e, "begin expression", &compiled.expr);
                anyhow::anyhow!("{}", detailed_msg)
            })?;

        *tracked = Self::get_thread_tracking_state();
        Ok(())
    }

    pub fn execute_compiled_end(
        &self,
        compiled: &CompiledExpression,
        tracked: &HashMap<String, Dynamic>,
    ) -> Result<()> {
        let mut scope = self.scope_template.clone();
        let mut tracked_map = rhai::Map::new();

        // Convert HashMap to Rhai Map (read-only)
        for (k, v) in tracked.iter() {
            tracked_map.insert(k.clone().into(), v.clone());
        }
        scope.set_value("tracked", tracked_map);

        let _ = self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
            .map_err(|e| {
                let detailed_msg = Self::format_rhai_error(e, "end expression", &compiled.expr);
                anyhow::anyhow!("{}", detailed_msg)
            })?;

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

            let _ = self
                .engine
                .eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
                .map_err(|e| {
                    let detailed_msg = Self::format_rhai_error(e, "begin expression", &compiled.expr);
                    anyhow::anyhow!("{}", detailed_msg)
                })?;

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

            let _ = self
                .engine
                .eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
                .map_err(|e| {
                    let detailed_msg = Self::format_rhai_error(e, "end expression", &compiled.expr);
                    anyhow::anyhow!("{}", detailed_msg)
                })?;
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
        // Capture mutations made directly to the `event` map
        if let Some(obj) = scope.get_value::<Map>("event") {
            for (k, v) in obj {
                event.fields.insert(k.to_string(), v.clone());
            }
        }

        // Also include top-level vars (e.g. `let x = ...`)
        for (name, _constant, value) in scope.iter() {
            if name != "line" && name != "event" && name != "meta" {
                event.fields.insert(name.to_string(), value.clone());
            }
        }
    }

    fn is_valid_identifier(&self, name: &str) -> bool {
        name.chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_alphanumeric() || c == '_')
    }
}
