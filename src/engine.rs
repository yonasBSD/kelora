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

        engine.register_fn("slice", |s: &str, spec: &str| -> String {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as i32;
            
            if len == 0 {
                return String::new();
            }

            let parts: Vec<&str> = spec.split(':').collect();
            
            // Parse step first
            let step = if parts.len() > 2 && !parts[2].trim().is_empty() {
                parts[2].trim().parse::<i32>().unwrap_or(1)
            } else {
                1
            };
            
            if step == 0 {
                return String::new();
            }
            
            // Determine defaults based on step direction
            let (default_start, default_end) = if step > 0 {
                (0, len)
            } else {
                (len - 1, -1)
            };
            
            // Parse start
            let start = if !parts.is_empty() && !parts[0].trim().is_empty() {
                let mut s = parts[0].trim().parse::<i32>().unwrap_or(default_start);
                if s < 0 { s += len; }
                if step > 0 {
                    s.clamp(0, len)
                } else {
                    s.clamp(0, len - 1)
                }
            } else {
                default_start
            };
            
            // Parse end
            let end = if parts.len() > 1 && !parts[1].trim().is_empty() {
                let mut e = parts[1].trim().parse::<i32>().unwrap_or(default_end);
                if e < 0 { e += len; }
                if step > 0 {
                    e.clamp(0, len)
                } else {
                    e.clamp(-1, len - 1)
                }
            } else {
                default_end
            };
            
            let mut result = String::new();
            let mut i = start;
            
            if step > 0 {
                while i < end {
                    if i >= 0 && i < len {
                        result.push(chars[i as usize]);
                    }
                    i += step;
                }
            } else {
                while i > end {
                    if i >= 0 && i < len {
                        result.push(chars[i as usize]);
                    }
                    i += step;
                }
            }
            
            result
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to test slice functionality
    fn test_slice(input: &str, spec: &str) -> String {
        let mut engine = Engine::new();
        engine.register_fn("slice", |s: &str, spec: &str| -> String {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as i32;
            
            if len == 0 {
                return String::new();
            }

            let parts: Vec<&str> = spec.split(':').collect();
            
            // Parse step first
            let step = if parts.len() > 2 && !parts[2].trim().is_empty() {
                parts[2].trim().parse::<i32>().unwrap_or(1)
            } else {
                1
            };
            
            if step == 0 {
                return String::new();
            }
            
            // Determine defaults based on step direction
            let (default_start, default_end) = if step > 0 {
                (0, len)
            } else {
                (len - 1, -1)
            };
            
            // Parse start
            let start = if !parts.is_empty() && !parts[0].trim().is_empty() {
                let mut s = parts[0].trim().parse::<i32>().unwrap_or(default_start);
                if s < 0 { s += len; }
                if step > 0 {
                    s.clamp(0, len)
                } else {
                    s.clamp(0, len - 1)
                }
            } else {
                default_start
            };
            
            // Parse end
            let end = if parts.len() > 1 && !parts[1].trim().is_empty() {
                let mut e = parts[1].trim().parse::<i32>().unwrap_or(default_end);
                if e < 0 { e += len; }
                if step > 0 {
                    e.clamp(0, len)
                } else {
                    e.clamp(-1, len - 1)
                }
            } else {
                default_end
            };
            
            let mut result = String::new();
            let mut i = start;
            
            if step > 0 {
                while i < end {
                    if i >= 0 && i < len {
                        result.push(chars[i as usize]);
                    }
                    i += step;
                }
            } else {
                while i > end {
                    if i >= 0 && i < len {
                        result.push(chars[i as usize]);
                    }
                    i += step;
                }
            }
            
            result
        });

        let mut scope = rhai::Scope::new();
        scope.push("text", input.to_string());
        
        engine.eval_with_scope::<String>(&mut scope, &format!("text.slice(\"{}\")", spec)).unwrap()
    }

    #[test]
    fn test_slice_basic_forward() {
        assert_eq!(test_slice("hello", "0:3"), "hel");
        assert_eq!(test_slice("hello", "1:4"), "ell");
        assert_eq!(test_slice("hello", "2:5"), "llo");
    }

    #[test]
    fn test_slice_from_start() {
        assert_eq!(test_slice("hello", ":3"), "hel");
        assert_eq!(test_slice("hello", ":0"), "");
        assert_eq!(test_slice("hello", ":5"), "hello");
    }

    #[test]
    fn test_slice_to_end() {
        assert_eq!(test_slice("hello", "2:"), "llo");
        assert_eq!(test_slice("hello", "0:"), "hello");
        assert_eq!(test_slice("hello", "5:"), "");
    }

    #[test]
    fn test_slice_negative_indices() {
        assert_eq!(test_slice("hello", "-3:"), "llo");
        assert_eq!(test_slice("hello", ":-2"), "hel");
        assert_eq!(test_slice("hello", "-3:-1"), "ll");
        assert_eq!(test_slice("hello", "-1:"), "o");
    }

    #[test]
    fn test_slice_step() {
        assert_eq!(test_slice("hello", "::2"), "hlo");
        assert_eq!(test_slice("hello", "1::2"), "el");
        assert_eq!(test_slice("hello", "::3"), "hl");
        assert_eq!(test_slice("abcdefg", "1:6:2"), "bdf");
    }

    #[test]
    fn test_slice_reverse() {
        assert_eq!(test_slice("hello", "::-1"), "olleh");
        assert_eq!(test_slice("abc", "::-1"), "cba");
        assert_eq!(test_slice("a", "::-1"), "a");
    }

    #[test]
    fn test_slice_reverse_with_bounds() {
        assert_eq!(test_slice("hello", "4:1:-1"), "oll");
        assert_eq!(test_slice("hello", "3::-1"), "lleh");
        assert_eq!(test_slice("hello", ":2:-1"), "ol");
    }

    #[test]
    fn test_slice_edge_cases() {
        // Empty string
        assert_eq!(test_slice("", "0:1"), "");
        assert_eq!(test_slice("", "::-1"), "");
        
        // Single character
        assert_eq!(test_slice("a", "0:1"), "a");
        assert_eq!(test_slice("a", "::-1"), "a");
        
        // Out of bounds indices
        assert_eq!(test_slice("hello", "10:20"), "");
        assert_eq!(test_slice("hello", "-10:2"), "he");
        assert_eq!(test_slice("hello", "2:10"), "llo");
        
        // Same start and end
        assert_eq!(test_slice("hello", "2:2"), "");
        
        // Start greater than end (forward step)
        assert_eq!(test_slice("hello", "4:2"), "");
    }

    #[test]
    fn test_slice_unicode() {
        // Test with Unicode characters
        assert_eq!(test_slice("héllo", "0:2"), "hé");
        assert_eq!(test_slice("héllo", "::-1"), "olléh");
        assert_eq!(test_slice("🌟⭐✨", "1:3"), "⭐✨");
        assert_eq!(test_slice("🌟⭐✨", "::-1"), "✨⭐🌟");
    }

    #[test]
    fn test_slice_zero_step() {
        // Zero step should return empty string
        assert_eq!(test_slice("hello", "::0"), "");
    }

    #[test]
    fn test_slice_large_step() {
        assert_eq!(test_slice("hello", "::10"), "h");
        assert_eq!(test_slice("hello", "1::10"), "e");
    }

    #[test]
    fn test_slice_negative_step_edge_cases() {
        assert_eq!(test_slice("hello", "1::-2"), "e");
        assert_eq!(test_slice("hello", ":1:-2"), "ol");
        assert_eq!(test_slice("hello", "3:0:-2"), "le");
    }
}