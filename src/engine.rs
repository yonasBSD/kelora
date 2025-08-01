#![allow(dead_code)]
use anyhow::{Context, Result};
use rhai::{Dynamic, Engine, EvalAltResult, Scope, AST};
use std::collections::HashMap;

use rhai::debugger::{DebuggerCommand, DebuggerEvent};

use crate::event::Event;
use crate::rhai_functions;

use rhai::Map;

// Temporary debug types until module structure is fixed
#[derive(Debug, Clone)]
pub struct DebugConfig {
    pub enabled: bool,
    pub verbosity: u8,
    pub show_timing: bool,
    pub trace_events: bool,
}

impl DebugConfig {
    pub fn new(debug: bool, verbose_count: u8) -> Self {
        DebugConfig {
            enabled: debug,
            verbosity: if debug { verbose_count } else { 0 },
            show_timing: debug,
            trace_events: verbose_count >= 2,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub position: Option<rhai::Position>,
    pub source_snippet: Option<String>,
    pub last_operation: Option<String>,
    pub error_location: Option<String>,
}

use std::sync::{Arc, Mutex};
pub struct DebugTracker {
    pub config: DebugConfig,
    context: Arc<Mutex<ExecutionContext>>,
    event_count: Arc<Mutex<u64>>,
    error_count: Arc<Mutex<u64>>,
}

impl DebugTracker {
    pub fn new(config: DebugConfig) -> Self {
        DebugTracker {
            config,
            context: Arc::new(Mutex::new(ExecutionContext::default())),
            event_count: Arc::new(Mutex::new(0)),
            error_count: Arc::new(Mutex::new(0)),
        }
    }
    
    pub fn log_basic(&self, message: &str) {
        if self.config.enabled {
            eprintln!("Debug: {}", message);
        }
    }
    
    pub fn update_context(&self, position: Option<rhai::Position>, source: Option<&str>) {
        if self.config.enabled {
            if let Ok(mut ctx) = self.context.lock() {
                ctx.position = position;
                ctx.source_snippet = source.map(|s| s.to_string());
            }
        }
    }
    
    pub fn get_context(&self) -> ExecutionContext {
        if let Ok(ctx) = self.context.lock() {
            ctx.clone()
        } else {
            ExecutionContext::default()
        }
    }
}

impl Clone for DebugTracker {
    fn clone(&self) -> Self {
        DebugTracker {
            config: self.config.clone(),
            context: Arc::clone(&self.context),
            event_count: Arc::clone(&self.event_count),
            error_count: Arc::clone(&self.error_count),
        }
    }
}

pub struct ErrorEnhancer {
    debug_config: DebugConfig,
}

impl ErrorEnhancer {
    pub fn new(debug_config: DebugConfig) -> Self {
        ErrorEnhancer { debug_config }
    }
    
    pub fn enhance_error(&self, 
        error: &EvalAltResult, 
        scope: &Scope, 
        script: &str,
        stage: &str,
        _execution_context: &ExecutionContext
    ) -> String {
        let mut output = String::new();
        
        // Basic error info
        output.push_str(&format!("❌ Stage {} failed\n", stage));
        output.push_str(&format!("   Code: {}\n", script.trim()));
        output.push_str(&format!("   Error: {}\n", error));
        
        // Show scope information if debug enabled
        if self.debug_config.enabled {
            output.push_str("\n   Variables in scope:\n");
            for (name, _is_const, value) in scope.iter() {
                let preview = format!("{:?}", value);
                let preview = if preview.len() > 50 {
                    format!("{}...", &preview[..47])
                } else {
                    preview
                };
                output.push_str(&format!("   • {}: {} = {}\n", 
                    name, value.type_name(), preview));
            }
        }
        
        output
    }
}

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
    init_map: Option<rhai::Map>,
    debug_tracker: Option<DebugTracker>,
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
            init_map: self.init_map.clone(),
            debug_tracker: self.debug_tracker.clone(),
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
                // Use both old capture system (for compatibility) and new ordered system
                crate::rhai_functions::strings::capture_print(text.to_string());
                crate::rhai_functions::strings::capture_stdout(text.to_string());
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
        scope_template.push("e", rhai::Map::new());
        scope_template.push("meta", rhai::Map::new());
        scope_template.push("init", rhai::Map::new());

        Self {
            engine,
            compiled_filters: Vec::new(),
            compiled_execs: Vec::new(),
            compiled_begin: None,
            compiled_end: None,
            scope_template,
            suppress_side_effects: false,
            init_map: None,
            debug_tracker: None,
        }
    }

    /// Set up debugging with the provided configuration
    pub fn setup_debugging(&mut self, debug_config: DebugConfig) {
        if !debug_config.enabled {
            return;
        }

        self.debug_tracker = Some(DebugTracker::new(debug_config.clone()));

        let debug_tracker = self.debug_tracker.as_ref().unwrap().clone();
        self.engine.register_debugger(
            |_engine, debugger| {
                // Initialize debugger - no breakpoints for now in Phase 1
                debugger
            },
            move |_context, event, _node, source, pos| {
                // Update execution context
                debug_tracker.update_context(Some(pos), source);
                
                // Basic event logging for Phase 1
                match event {
                    DebuggerEvent::Start => {
                        debug_tracker.log_basic("Script execution started");
                    },
                    DebuggerEvent::End => {
                        debug_tracker.log_basic("Script execution completed");
                    },
                    _ => {} // Handle more events in later phases
                }
                
                Ok(DebuggerCommand::Continue)
            }
        );
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
                let detailed_msg = if let Some(ref debug_tracker) = self.debug_tracker {
                    let enhancer = ErrorEnhancer::new(debug_tracker.config.clone());
                    let context = debug_tracker.get_context();
                    enhancer.enhance_error(&e, &scope, &compiled.expr, "filter", &context)
                } else {
                    Self::format_rhai_error(e, "filter expression", &compiled.expr)
                };
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
                let detailed_msg = if let Some(ref debug_tracker) = self.debug_tracker {
                    let enhancer = ErrorEnhancer::new(debug_tracker.config.clone());
                    let context = debug_tracker.get_context();
                    enhancer.enhance_error(&e, &scope, &compiled.expr, "exec", &context)
                } else {
                    Self::format_rhai_error(e, "exec script", &compiled.expr)
                };
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
    ) -> Result<rhai::Map> {
        Self::set_thread_tracking_state(tracked);

        // Set begin phase flag to allow read_file/read_lines
        crate::rhai_functions::init::set_begin_phase(true);

        let mut scope = self.scope_template.clone();

        let _ = self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &compiled.ast)
            .map_err(|e| {
                let detailed_msg = Self::format_rhai_error(e, "begin expression", &compiled.expr);
                anyhow::anyhow!("{}", detailed_msg)
            })?;

        // Reset begin phase flag
        crate::rhai_functions::init::set_begin_phase(false);

        *tracked = Self::get_thread_tracking_state();

        // Extract the init map from scope and store it
        let mut init_map = scope.get_value::<rhai::Map>("init").unwrap_or_default();

        // Deep freeze the init map to make it read-only
        crate::rhai_functions::init::deep_freeze_map(&mut init_map);

        // Store the frozen init map
        self.init_map = Some(init_map.clone());

        Ok(init_map)
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

        // Update built-in variables
        scope.set_value("line", event.original_line.clone());

        // Update event map for fields with invalid identifiers
        let mut event_map = rhai::Map::new();
        for (k, v) in &event.fields {
            event_map.insert(k.clone().into(), v.clone());
        }
        scope.set_value("e", event_map);

        // Update metadata
        let mut meta_map = rhai::Map::new();
        if let Some(line_num) = event.line_number {
            meta_map.insert("line_number".into(), Dynamic::from(line_num as i64));
        }
        if let Some(filename) = &event.filename {
            meta_map.insert("filename".into(), Dynamic::from(filename.clone()));
        }

        // Add raw line for quarantine mode
        meta_map.insert("line".into(), Dynamic::from(event.original_line.clone()));

        // Check for quarantine metadata
        if let Some(parse_error) = event.fields.get("__kelora_quarantine_parse_error") {
            meta_map.insert("parse_error".into(), parse_error.clone());
        }
        if let Some(decode_error) = event.fields.get("__kelora_quarantine_decode_error") {
            meta_map.insert("decode_error".into(), decode_error.clone());
        }

        scope.set_value("meta", meta_map);

        // Set the frozen init map
        if let Some(ref init_map) = self.init_map {
            scope.set_value("init", init_map.clone());
        }

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
        // Check if entire event 'e' was set to unit () - clear all fields
        if scope.get_value::<()>("e").is_some() {
            event.fields.clear();
            return;
        }

        // Capture mutations made directly to the `e` event map
        if let Some(obj) = scope.get_value::<Map>("e") {
            for (k, v) in obj {
                // Remove fields that are set to unit () - allows easy field removal
                if v.is::<()>() {
                    event.fields.shift_remove(&k.to_string());
                } else {
                    event.fields.insert(k.to_string(), v.clone());
                }
            }
        }
    }
}
