use anyhow::{Context, Result};
use rhai::{Dynamic, Engine, Scope, AST};
use std::collections::HashMap;
use std::cell::RefCell;

// Thread-local storage for tracking state
thread_local! {
    static THREAD_TRACKING_STATE: RefCell<HashMap<String, Dynamic>> = RefCell::new(HashMap::new());
}

use crate::event::Event;

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
        Self::register_functions(&mut engine);
        
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
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.clear();
            for (k, v) in tracked {
                state.insert(k.clone(), v.clone());
            }
        });
    }

    pub fn get_thread_tracking_state() -> HashMap<String, Dynamic> {
        THREAD_TRACKING_STATE.with(|state| {
            state.borrow().clone()
        })
    }

    pub fn clear_thread_tracking_state() {
        THREAD_TRACKING_STATE.with(|state| {
            state.borrow_mut().clear();
        });
    }
    pub fn new() -> Self {
        let mut engine = Engine::new();
        
        // Enable print statements (they output to stderr by default in Rhai)
        engine.set_optimization_level(rhai::OptimizationLevel::Simple);
        
        // Register custom functions for log analysis
        Self::register_functions(&mut engine);
        
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

    pub fn compile_expressions(&mut self, 
        filters: &[String], 
        execs: &[String], 
        begin: Option<&String>, 
        end: Option<&String>
    ) -> Result<()> {
        for filter in filters {
            let ast = self.engine.compile(filter)
                .with_context(|| format!("Failed to compile filter expression: {}", filter))?;
            self.compiled_filters.push(CompiledExpression {
                ast,
                expr: filter.clone(),
            });
        }

        for exec in execs {
            let ast = self.engine.compile(exec)
                .with_context(|| format!("Failed to compile exec script: {}", exec))?;
            self.compiled_execs.push(CompiledExpression {
                ast,
                expr: exec.clone(),
            });
        }

        if let Some(begin_expr) = begin {
            let ast = self.engine.compile(begin_expr)
                .with_context(|| format!("Failed to compile begin expression: {}", begin_expr))?;
            self.compiled_begin = Some(CompiledExpression {
                ast,
                expr: begin_expr.clone(),
            });
        }

        if let Some(end_expr) = end {
            let ast = self.engine.compile(end_expr)
                .with_context(|| format!("Failed to compile end expression: {}", end_expr))?;
            self.compiled_end = Some(CompiledExpression {
                ast,
                expr: end_expr.clone(),
            });
        }

        Ok(())
    }


    fn register_functions(engine: &mut Engine) {
        // Track functions using thread-local storage - clean user API
        // Store operation metadata for proper parallel merging
        engine.register_fn("track_count", |key: &str| {
            THREAD_TRACKING_STATE.with(|state| {
                let mut state = state.borrow_mut();
                let count = state.get(key).cloned().unwrap_or(Dynamic::from(0i64));
                let new_count = count.as_int().unwrap_or(0) + 1;
                state.insert(key.to_string(), Dynamic::from(new_count));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("count"));
            });
        });

        engine.register_fn("track_count", |key: &str, delta: i64| {
            THREAD_TRACKING_STATE.with(|state| {
                let mut state = state.borrow_mut();
                let count = state.get(key).cloned().unwrap_or(Dynamic::from(0i64));
                let new_count = count.as_int().unwrap_or(0) + delta;
                state.insert(key.to_string(), Dynamic::from(new_count));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("count"));
            });
        });

        engine.register_fn("track_min", |key: &str, value: i64| {
            THREAD_TRACKING_STATE.with(|state| {
                let mut state = state.borrow_mut();
                let current = state.get(key).cloned().unwrap_or(Dynamic::from(i64::MAX));
                let current_val = current.as_int().unwrap_or(i64::MAX);
                if value < current_val {
                    state.insert(key.to_string(), Dynamic::from(value));
                    // Store operation type metadata for parallel merging
                    state.insert(format!("__op_{}", key), Dynamic::from("min"));
                }
            });
        });

        engine.register_fn("track_max", |key: &str, value: i64| {
            THREAD_TRACKING_STATE.with(|state| {
                let mut state = state.borrow_mut();
                let current = state.get(key).cloned().unwrap_or(Dynamic::from(i64::MIN));
                let current_val = current.as_int().unwrap_or(i64::MIN);
                if value > current_val {
                    state.insert(key.to_string(), Dynamic::from(value));
                    // Store operation type metadata for parallel merging
                    state.insert(format!("__op_{}", key), Dynamic::from("max"));
                }
            });
        });

        engine.register_fn("track_unique", |key: &str, value: &str| {
            THREAD_TRACKING_STATE.with(|state| {
                let mut state = state.borrow_mut();
                // Get existing set or create new one
                let current = state.get(key).cloned().unwrap_or_else(|| {
                    // Create a new array to store unique values
                    Dynamic::from(rhai::Array::new())
                });
                
                if let Ok(mut arr) = current.into_array() {
                    let value_dynamic = Dynamic::from(value.to_string());
                    // Check if value already exists in array
                    if !arr.iter().any(|v| v.clone().into_string().unwrap_or_default() == value) {
                        arr.push(value_dynamic);
                    }
                    state.insert(key.to_string(), Dynamic::from(arr));
                    // Store operation type metadata for parallel merging
                    state.insert(format!("__op_{}", key), Dynamic::from("unique"));
                }
            });
        });

        engine.register_fn("track_bucket", |key: &str, bucket: &str| {
            THREAD_TRACKING_STATE.with(|state| {
                let mut state = state.borrow_mut();
                // Get existing map or create new one
                let current = state.get(key).cloned().unwrap_or_else(|| {
                    Dynamic::from(rhai::Map::new())
                });
                
                if let Some(mut map) = current.try_cast::<rhai::Map>() {
                    let count = map.get(bucket).cloned().unwrap_or(Dynamic::from(0i64));
                    let new_count = count.as_int().unwrap_or(0) + 1;
                    map.insert(bucket.into(), Dynamic::from(new_count));
                    state.insert(key.to_string(), Dynamic::from(map));
                    // Store operation type metadata for parallel merging
                    state.insert(format!("__op_{}", key), Dynamic::from("bucket"));
                }
            });
        });

        // String analysis functions
        engine.register_fn("contains", |text: &str, pattern: &str| {
            text.contains(pattern)
        });

        engine.register_fn("matches", |text: &str, pattern: &str| {
            regex::Regex::new(pattern)
                .map(|re| re.is_match(text))
                .unwrap_or(false)
        });

        engine.register_fn("to_int", |text: &str| -> rhai::Dynamic {
            text.parse::<i64>().map(Dynamic::from).unwrap_or(Dynamic::UNIT)
        });

        engine.register_fn("to_float", |text: &str| -> rhai::Dynamic {
            text.parse::<f64>().map(Dynamic::from).unwrap_or(Dynamic::UNIT)
        });

        // Log analysis functions
        engine.register_fn("status_class", |status: i64| -> String {
            match status {
                100..=199 => "1xx".to_string(),
                200..=299 => "2xx".to_string(),
                300..=399 => "3xx".to_string(),
                400..=499 => "4xx".to_string(),
                500..=599 => "5xx".to_string(),
                _ => "unknown".to_string(),
            }
        });
    }

    fn register_variable_resolver(_engine: &mut Engine) {
        // For now, keep this empty - we'll implement proper function-based approach
        // Variable resolver is not the right tool for function calls
    }

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

    pub fn execute_filters(&mut self, event: &Event, tracked: &mut HashMap<String, Dynamic>) -> Result<bool> {
        if self.compiled_filters.is_empty() {
            return Ok(true);
        }

        // Set thread-local state (filters don't usually modify it, but just in case)
        Self::set_thread_tracking_state(tracked);
        
        let mut scope = self.create_scope_for_event(event);
        
        for compiled_filter in &self.compiled_filters {
            let result = self.engine.eval_ast_with_scope::<bool>(&mut scope, &compiled_filter.ast)
                .map_err(|e| anyhow::anyhow!("Failed to execute filter expression '{}': {}", compiled_filter.expr, e))?;
            
            if !result {
                return Ok(false);
            }
        }

        // Update tracked from thread-local state (in case filter modified it)
        *tracked = Self::get_thread_tracking_state();

        Ok(true)
    }

    pub fn execute_execs(&mut self, event: &mut Event, tracked: &mut HashMap<String, Dynamic>) -> Result<()> {
        if self.compiled_execs.is_empty() {
            return Ok(());
        }

        // Set thread-local state for tracking functions
        Self::set_thread_tracking_state(tracked);
        
        let mut scope = self.create_scope_for_event(event);
        
        for compiled_exec in &self.compiled_execs {
            let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &compiled_exec.ast)
                .map_err(|e| anyhow::anyhow!("Failed to execute exec script '{}': {}", compiled_exec.expr, e))?;
        }

        // Update event fields from scope
        self.update_event_from_scope(event, &scope);

        // Update tracked state from thread-local storage
        *tracked = Self::get_thread_tracking_state();

        Ok(())
    }

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