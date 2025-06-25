use anyhow::{Context, Result};
use rhai::{Dynamic, Engine, Scope, AST};
use std::collections::HashMap;

use crate::event::Event;

pub struct RhaiEngine {
    engine: Engine,
    begin_ast: Option<AST>,
    filter_asts: Vec<AST>,
    eval_asts: Vec<AST>,
    end_ast: Option<AST>,
}

impl RhaiEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        
        // Enable print statements (they output to stderr by default in Rhai)
        engine.set_optimization_level(rhai::OptimizationLevel::Simple);
        
        // Register custom functions for log analysis
        Self::register_functions(&mut engine);
        
        Self {
            engine,
            begin_ast: None,
            filter_asts: Vec::new(),
            eval_asts: Vec::new(),
            end_ast: None,
        }
    }

    fn register_functions(engine: &mut Engine) {
        // Track functions for global state
        engine.register_fn("track_count", |tracked: &mut rhai::Map, key: &str| {
            let count = tracked.get(key).cloned().unwrap_or(Dynamic::from(0i64));
            let new_count = count.as_int().unwrap_or(0) + 1;
            tracked.insert(key.into(), Dynamic::from(new_count));
        });

        engine.register_fn("track_min", |tracked: &mut rhai::Map, key: &str, value: i64| {
            let current = tracked.get(key).cloned().unwrap_or(Dynamic::from(i64::MAX));
            let current_val = current.as_int().unwrap_or(i64::MAX);
            if value < current_val {
                tracked.insert(key.into(), Dynamic::from(value));
            }
        });

        engine.register_fn("track_max", |tracked: &mut rhai::Map, key: &str, value: i64| {
            let current = tracked.get(key).cloned().unwrap_or(Dynamic::from(i64::MIN));
            let current_val = current.as_int().unwrap_or(i64::MIN);
            if value > current_val {
                tracked.insert(key.into(), Dynamic::from(value));
            }
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

    pub fn execute_begin(&mut self, expr: &str, tracked: &mut HashMap<String, Dynamic>) -> Result<()> {
        let ast = self.engine.compile(expr)
            .with_context(|| format!("Failed to compile begin expression: {}", expr))?;
        
        let mut scope = Scope::new();
        let mut tracked_map = rhai::Map::new();
        
        // Convert HashMap to Rhai Map
        for (k, v) in tracked.iter() {
            tracked_map.insert(k.clone().into(), v.clone());
        }
        scope.push("tracked", tracked_map);
        
        let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute begin expression '{}': {}", expr, e))?;

        // Convert back to HashMap
        if let Some(tracked_result) = scope.get_value::<rhai::Map>("tracked") {
            tracked.clear();
            for (k, v) in tracked_result {
                tracked.insert(k.to_string(), v);
            }
        }

        Ok(())
    }

    pub fn execute_filter(&mut self, expr: &str, event: &Event, tracked: &mut HashMap<String, Dynamic>) -> Result<bool> {
        let ast = self.engine.compile(expr)
            .with_context(|| format!("Failed to compile filter expression: {}", expr))?;
        
        let mut scope = self.create_scope_for_event(event, tracked);
        
        let result = self.engine.eval_ast_with_scope::<bool>(&mut scope, &ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute filter expression '{}': {}", expr, e))?;

        Ok(result)
    }

    pub fn execute_eval(&mut self, expr: &str, event: &mut Event, tracked: &mut HashMap<String, Dynamic>) -> Result<()> {
        let ast = self.engine.compile(expr)
            .with_context(|| format!("Failed to compile eval expression: {}", expr))?;
        
        let mut scope = self.create_scope_for_event(event, tracked);
        
        let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute eval expression '{}': {}", expr, e))?;

        // Update event fields from scope
        self.update_event_from_scope(event, &scope);

        // Update tracked state
        if let Some(tracked_result) = scope.get_value::<rhai::Map>("tracked") {
            tracked.clear();
            for (k, v) in tracked_result {
                tracked.insert(k.to_string(), v);
            }
        }

        Ok(())
    }

    pub fn execute_end(&mut self, expr: &str, tracked: &HashMap<String, Dynamic>) -> Result<()> {
        let ast = self.engine.compile(expr)
            .with_context(|| format!("Failed to compile end expression: {}", expr))?;
        
        let mut scope = Scope::new();
        let mut tracked_map = rhai::Map::new();
        
        // Convert HashMap to Rhai Map (read-only)
        for (k, v) in tracked.iter() {
            tracked_map.insert(k.clone().into(), v.clone());
        }
        scope.push("tracked", tracked_map);
        
        let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute end expression '{}': {}", expr, e))?;

        Ok(())
    }

    fn create_scope_for_event(&self, event: &Event, tracked: &HashMap<String, Dynamic>) -> Scope {
        let mut scope = Scope::new();
        
        // Inject event fields as variables
        for (key, value) in &event.fields {
            if self.is_valid_identifier(key) {
                scope.push(key, self.convert_value_to_dynamic(value));
            }
        }
        
        // Add built-in variables
        scope.push("line", event.original_line.clone());
        
        // Add event map for fields with invalid identifiers
        let mut event_map = rhai::Map::new();
        for (k, v) in &event.fields {
            event_map.insert(k.clone().into(), self.convert_value_to_dynamic(v));
        }
        scope.push("event", event_map);
        
        // Add metadata
        let mut meta_map = rhai::Map::new();
        if let Some(line_num) = event.line_number {
            meta_map.insert("linenum".into(), Dynamic::from(line_num as i64));
        }
        if let Some(filename) = &event.filename {
            meta_map.insert("filename".into(), Dynamic::from(filename.clone()));
        }
        scope.push("meta", meta_map);
        
        // Add tracked state
        let mut tracked_map = rhai::Map::new();
        for (k, v) in tracked.iter() {
            tracked_map.insert(k.clone().into(), v.clone());
        }
        scope.push("tracked", tracked_map);
        
        scope
    }

    fn update_event_from_scope(&self, event: &mut Event, scope: &Scope) {
        for (name, _constant, value) in scope.iter() {
            if name != "line" && name != "event" && name != "meta" && name != "tracked" {
                if let Some(json_value) = self.convert_dynamic_to_json_value(&value) {
                    event.fields.insert(name.to_string(), json_value);
                }
            }
        }
    }

    fn is_valid_identifier(&self, name: &str) -> bool {
        name.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') &&
        name.chars().all(|c| c.is_alphanumeric() || c == '_')
    }

    fn convert_value_to_dynamic(&self, value: &serde_json::Value) -> Dynamic {
        match value {
            serde_json::Value::Null => Dynamic::UNIT,
            serde_json::Value::Bool(b) => Dynamic::from(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Dynamic::from(i)
                } else if let Some(f) = n.as_f64() {
                    Dynamic::from(f)
                } else {
                    Dynamic::from(n.to_string())
                }
            }
            serde_json::Value::String(s) => Dynamic::from(s.clone()),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                Dynamic::from(value.to_string())
            }
        }
    }

    fn convert_dynamic_to_json_value(&self, value: &Dynamic) -> Option<serde_json::Value> {
        if value.is_unit() {
            Some(serde_json::Value::Null)
        } else if value.is_bool() {
            Some(serde_json::Value::Bool(value.as_bool().unwrap_or(false)))
        } else if value.is_int() {
            Some(serde_json::Value::Number(serde_json::Number::from(value.as_int().unwrap_or(0))))
        } else if value.is_float() {
            serde_json::Number::from_f64(value.as_float().unwrap_or(0.0)).map(serde_json::Value::Number)
        } else if value.is_string() {
            Some(serde_json::Value::String(value.clone().into_string().unwrap_or_default()))
        } else {
            Some(serde_json::Value::String(value.to_string()))
        }
    }
}