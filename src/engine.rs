use anyhow::{Context, Result};
use rhai::{Dynamic, Engine, EvalAltResult, Scope, AST};
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
    suppress_side_effects: bool,
}

impl Clone for RhaiEngine {
    fn clone(&self) -> Self {
        let mut engine = Engine::new();
        engine.set_optimization_level(rhai::OptimizationLevel::Simple);

        // Apply the same on_print override as in new(), respecting suppress_side_effects
        let suppress_side_effects = self.suppress_side_effects;
        engine.on_print(move |text| {
            if suppress_side_effects {
                // Suppress all print output
                return;
            }

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
            suppress_side_effects,
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
                format!(
                    "Runtime error in {} at {}: {}",
                    script_name, pos, runtime_err
                )
            }
            EvalAltResult::ErrorVariableNotFound(var, pos) => {
                format!("Variable '{}' not found in {} at {}", var, script_name, pos)
            }
            EvalAltResult::ErrorFunctionNotFound(func, pos) => {
                Self::format_function_not_found_error(func, script_name, pos)
            }
            EvalAltResult::ErrorMismatchDataType(expected, actual, pos) => {
                format!("Type mismatch in {} at {}: expected {}, got {} (this often indicates a function was called with incorrect argument types)", 
                        script_name, pos, expected, actual)
            }
            EvalAltResult::ErrorInFunctionCall(func, _source, inner_err, pos) => {
                let inner_msg = Self::format_rhai_error(inner_err, "function", "");
                format!(
                    "Error in function '{}' in {} at {}: {}",
                    func, script_name, pos, inner_msg
                )
            }
            EvalAltResult::ErrorPropertyNotFound(prop, pos) => {
                format!(
                    "Property '{}' not found in {} at {}",
                    prop, script_name, pos
                )
            }
            EvalAltResult::ErrorIndexNotFound(index, pos) => {
                format!("Index '{}' not found in {} at {}", index, script_name, pos)
            }
            EvalAltResult::ErrorDotExpr(msg, pos) => {
                format!(
                    "Property access error in {} at {}: {}",
                    script_name, pos, msg
                )
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
            _ => format!("Error in {}: {}", script_name, err),
        }
    }

    fn format_function_not_found_error(
        func_signature: String,
        script_name: &str,
        pos: rhai::Position,
    ) -> String {
        // Extract function name from signature (before the first '(' or space)
        let func_name = if let Some(paren_pos) = func_signature.find('(') {
            &func_signature[..paren_pos]
        } else if let Some(space_pos) = func_signature.find(' ') {
            &func_signature[..space_pos]
        } else {
            &func_signature
        }
        .trim();

        // Check if this looks like a type mismatch rather than a missing function
        if Self::is_likely_type_mismatch(&func_signature, func_name) {
            let expected_types = Self::get_expected_function_signature(func_name);
            if !expected_types.is_empty() {
                let called_types = Self::extract_called_types(&func_signature);
                return format!(
                    "Wrong argument types for '{}' in {} at {}: got {}, expected {}. Note: x.{}() = {}(x)",
                    func_name,
                    script_name,
                    pos,
                    called_types,
                    expected_types,
                    func_name,
                    func_name
                );
            }
        }

        // Fall back to "function not found" with suggestions
        let base_msg = format!(
            "Function '{}' not found in {} at {}",
            func_signature, script_name, pos
        );
        let suggestions = Self::get_function_suggestions(func_name);

        if suggestions.is_empty() {
            base_msg
        } else {
            format!("{}. Did you mean: {}", base_msg, suggestions.join(", "))
        }
    }

    fn is_likely_type_mismatch(func_signature: &str, func_name: &str) -> bool {
        // Check if the function name is one we know exists
        let known_functions = vec![
            "extract_re",
            "extract_all_re",
            "split_re",
            "replace_re",
            "count",
            "strip",
            "before",
            "after",
            "between",
            "starting_with",
            "ending_with",
            "is_digit",
            "join",
            "extract_ip",
            "extract_ips",
            "mask_ip",
            "is_private_ip",
            "extract_url",
            "extract_domain",
            "parse_json",
            "parse_kv",
            "get_path",
            "col",
            "cols",
            "status_class",
            "track_count",
            "track_sum",
            "track_min",
            "track_max",
            "track_avg",
            "track_unique",
            "track_bucket",
            // Common Rhai built-ins that work on strings
            "len",
            "contains",
            "starts_with",
            "ends_with",
            "split",
            "replace",
            "trim",
        ];

        known_functions.contains(&func_name) && func_signature.contains('(')
    }

    fn extract_called_types(func_signature: &str) -> String {
        if let Some(start) = func_signature.find('(') {
            if let Some(end) = func_signature.find(')') {
                return func_signature[start + 1..end].to_string();
            }
        }
        "unknown types".to_string()
    }

    fn get_expected_function_signature(func_name: &str) -> String {
        match func_name {
            "extract_re" => "string, regex_pattern, optional_group_index".to_string(),
            "extract_all_re" => "string, regex_pattern, optional_group_index".to_string(),
            "split_re" => "string, regex_pattern".to_string(),
            "replace_re" => "string, regex_pattern, replacement".to_string(),
            "before" | "after" => "string, delimiter".to_string(),
            "between" => "string, start_delimiter, end_delimiter".to_string(),
            "starting_with" | "ending_with" => "string, prefix_or_suffix".to_string(),
            "strip" => "string, optional_characters_to_strip".to_string(),
            "join" => "string_separator, array OR array, string_separator".to_string(),
            "extract_ip" | "extract_ips" | "extract_url" | "extract_domain" => "string".to_string(),
            "mask_ip" => "string, optional_octets_to_mask".to_string(),
            "is_private_ip" | "is_digit" => "string".to_string(),
            "parse_json" => "json_string".to_string(),
            "parse_kv" => "string, optional_separator, optional_kv_separator".to_string(),
            "get_path" => "map_or_json_string, path, optional_default".to_string(),
            "col" => "string, column_selector".to_string(),
            "cols" => "string, column_selectors...".to_string(),
            "status_class" => "status_code_number".to_string(),
            "track_count" | "track_sum" | "track_min" | "track_max" | "track_avg" => {
                "key, optional_value".to_string()
            }
            "track_unique" => "key, value".to_string(),
            "track_bucket" => "key, value, bucket_size".to_string(),
            "count" => "string, substring".to_string(),
            // Common string functions that expect strings
            "len" | "trim" => "string".to_string(),
            "contains" | "starts_with" | "ends_with" => "string, substring".to_string(),
            "split" | "replace" => "string, delimiter_or_pattern, optional_replacement".to_string(),
            _ => "".to_string(),
        }
    }

    fn get_function_suggestions(func_name: &str) -> Vec<String> {
        // List of common Rhai built-in functions and our custom functions
        let available_functions = vec![
            // String functions
            "lower",
            "upper",
            "trim",
            "len",
            "contains",
            "starts_with",
            "ends_with",
            "split",
            "replace",
            "substring",
            "to_string",
            "parse",
            // Our custom string functions
            "extract_re",
            "extract_all_re",
            "split_re",
            "replace_re",
            "count",
            "strip",
            "before",
            "after",
            "between",
            "starting_with",
            "ending_with",
            "is_digit",
            "join",
            "extract_ip",
            "extract_ips",
            "mask_ip",
            "is_private_ip",
            "extract_url",
            "extract_domain",
            // Math functions
            "abs",
            "floor",
            "ceil",
            "round",
            "min",
            "max",
            "pow",
            "sqrt",
            // Array functions
            "push",
            "pop",
            "shift",
            "unshift",
            "reverse",
            "sort",
            "clear",
            // Map functions
            "keys",
            "values",
            "remove",
            "contains",
            // Our custom functions
            "parse_json",
            "parse_kv",
            "get_path",
            "col",
            "cols",
            "status_class",
            "track_count",
            "track_sum",
            "track_min",
            "track_max",
            "track_avg",
            "track_unique",
            "track_bucket",
            // Utility functions
            "print",
            "debug",
            "type_of",
            "is_def_fn",
        ];

        // Find functions that are similar to the requested one
        let suggestions: Vec<String> = available_functions
            .iter()
            .filter(|&f| {
                // Check for starts with or contains
                f.starts_with(func_name) || (func_name.len() > 1 && f.contains(func_name))
            })
            .take(3) // Limit to 3 suggestions
            .map(|s| s.to_string())
            .collect();

        // For debugging: always include at least one suggestion if the function name contains common patterns
        if suggestions.is_empty() && func_name.len() > 2 {
            if func_name.contains("extract") {
                return vec![
                    "extract_re".to_string(),
                    "extract_ip".to_string(),
                    "extract_url".to_string(),
                ];
            } else if func_name.contains("track") {
                return vec![
                    "track_count".to_string(),
                    "track_sum".to_string(),
                    "track_unique".to_string(),
                ];
            } else if func_name.contains("pars") {
                return vec!["parse_json".to_string(), "parse_kv".to_string()];
            }
        }

        suggestions
    }

    pub fn new() -> Self {
        let mut engine = Engine::new();

        engine.set_optimization_level(rhai::OptimizationLevel::Simple);

        // Override the built-in print function to support capture in parallel mode
        // Note: suppress_side_effects is false by default in new()
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
            suppress_side_effects: false,
        }
    }

    /// Set whether to suppress side effects (print, eprint, etc.)
    pub fn set_suppress_side_effects(&mut self, suppress: bool) {
        self.suppress_side_effects = suppress;

        // Set the thread-local flag for eprint and other functions
        crate::rhai_functions::strings::set_suppress_side_effects(suppress);

        // Re-register the print handler with the new suppression setting
        let suppress_copy = suppress;
        self.engine.on_print(move |text| {
            if suppress_copy {
                // Suppress all print output
                return;
            }

            if crate::rhai_functions::strings::is_parallel_mode() {
                crate::rhai_functions::strings::capture_print(text.to_string());
            } else {
                println!("{}", text);
            }
        });
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
                    let detailed_msg =
                        Self::format_rhai_error(e, "begin expression", &compiled.expr);
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

    // Window-aware execution methods
    pub fn execute_compiled_filter_with_window(
        &mut self,
        compiled: &CompiledExpression,
        event: &Event,
        window: &[Event],
        tracked: &mut HashMap<String, Dynamic>,
    ) -> Result<bool> {
        Self::set_thread_tracking_state(tracked);
        let mut scope = self.create_scope_for_event_with_window(event, window);

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

    pub fn execute_compiled_exec_with_window(
        &mut self,
        compiled: &CompiledExpression,
        event: &mut Event,
        window: &[Event],
        tracked: &mut HashMap<String, Dynamic>,
    ) -> Result<()> {
        Self::set_thread_tracking_state(tracked);
        let mut scope = self.create_scope_for_event_with_window(event, window);

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

    fn create_scope_for_event_with_window(&self, event: &Event, window: &[Event]) -> Scope {
        let mut scope = self.create_scope_for_event(event);

        // Add window array to scope
        let window_array: rhai::Array = window
            .iter()
            .map(|event| {
                let mut event_map = rhai::Map::new();
                // Add all event fields to the map
                for (k, v) in &event.fields {
                    event_map.insert(k.clone().into(), v.clone());
                }
                // Add built-in fields
                event_map.insert("line".into(), Dynamic::from(event.original_line.clone()));
                if let Some(line_num) = event.line_number {
                    event_map.insert("line_number".into(), Dynamic::from(line_num as i64));
                }
                if let Some(filename) = &event.filename {
                    event_map.insert("filename".into(), Dynamic::from(filename.clone()));
                }
                Dynamic::from(event_map)
            })
            .collect();

        scope.set_value("window", window_array);
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
            if name != "line" && name != "event" && name != "meta" && name != "window" {
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
