use std::sync::{Arc, Mutex};
use rhai::{EvalAltResult, Scope, Position};

#[derive(Debug, Clone)]
pub struct DebugConfig {
    pub verbosity: u8,  // 0-3 for debug levels
    pub show_timing: bool,
    pub trace_events: bool,
}

impl DebugConfig {
    pub fn new(verbose_count: u8) -> Self {
        DebugConfig {
            verbosity: verbose_count,
            show_timing: verbose_count >= 1,
            trace_events: verbose_count >= 2,
        }
    }
    
    pub fn should_trace(&self) -> bool {
        self.verbosity >= 2
    }
    
    pub fn should_show_context(&self) -> bool {
        self.verbosity >= 2
    }
    
    pub fn is_enabled(&self) -> bool {
        self.verbosity > 0
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub position: Option<Position>,
    pub source_snippet: Option<String>,
    pub last_operation: Option<String>,
    pub error_location: Option<String>,
}

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
        if self.config.verbosity >= 1 {
            eprintln!("{}", message);
        }
    }
    
    pub fn update_context(&self, position: Option<Position>, source: Option<&str>) {
        if self.config.is_enabled() {
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
        execution_context: &ExecutionContext
    ) -> String {
        let mut output = String::new();
        
        // Basic error info
        output.push_str(&format!("❌ Stage {} failed\n", stage));
        output.push_str(&format!("   Code: {}\n", script.trim()));
        output.push_str(&format!("   Error: {}\n", error));
        
        // Add execution context if available
        if let Some(pos) = &execution_context.position {
            output.push_str(&format!("   Position: {}\n", pos));
        }
        
        // Show scope information
        if self.debug_config.should_show_context() {
            output.push_str("\n   Variables in scope:\n");
            for (name, _is_const, value) in scope.iter() {
                let preview = self.format_value_preview(&value);
                output.push_str(&format!("   • {}: {} = {}\n", 
                    name, value.type_name(), preview));
            }
            
            // Add suggestions based on error type
            if let Some(suggestions) = self.generate_suggestions(error, scope) {
                output.push_str(&format!("\n   💡 {}\n", suggestions));
            }
            
            // Add stage-specific help
            output.push_str(&self.get_stage_help(stage, error));
        }
        
        output
    }
    
    fn format_value_preview(&self, value: &rhai::Dynamic) -> String {
        let preview = format!("{:?}", value);
        if preview.len() > 50 {
            format!("{}...", &preview[..47])
        } else {
            preview
        }
    }
    
    fn generate_suggestions(&self, error: &EvalAltResult, scope: &Scope) -> Option<String> {
        match error {
            EvalAltResult::ErrorVariableNotFound(var_name, _) => {
                let similar = self.find_similar_variables(var_name, scope);
                if !similar.is_empty() {
                    Some(format!("Did you mean: {}?", similar.join(", ")))
                } else {
                    None
                }
            },
            _ => None
        }
    }
    
    fn find_similar_variables(&self, target: &str, scope: &Scope) -> Vec<String> {
        let mut suggestions = Vec::new();
        let target_lower = target.to_lowercase();
        
        for (name, _is_const, _value) in scope.iter() {
            let name_lower = name.to_lowercase();
            
            // Simple similarity check
            if name_lower.contains(&target_lower) || target_lower.contains(&name_lower) {
                suggestions.push(name.to_string());
            }
        }
        
        suggestions
    }
    
    fn get_stage_help(&self, stage: &str, _error: &EvalAltResult) -> String {
        match stage {
            "filter" => "   🎯 Filter tip: Use 'e.field_name' to access event fields\n".to_string(),
            "exec" => "   🎯 Exec tip: Use 'e.field_name = value' to set fields\n".to_string(),
            _ => String::new(),
        }
    }
}