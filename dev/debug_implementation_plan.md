# Kelora Enhanced Error Reporting & Progressive Debug Implementation Plan

## Overview

This plan implements enhanced error reporting and progressive debugging for Kelora based on the research findings in `dev/findings_report.md`. The approach uses Rhai's debugging interface (`debugging` feature) combined with scope management to provide comprehensive error enhancement.

## Prerequisites

- Enable Rhai debugging feature: `rhai = { version = "1.22", features = ["sync", "debugging"] }`
- All changes should be backward compatible
- Follow kelora's existing code patterns and error handling

## Phase 1: Foundation - Basic Debug Infrastructure (2-3 days)

### Goal
Set up the basic debugging framework and enable Rhai debugging callbacks without changing user-facing behavior.

### Tasks

#### 1.1: Add Debug Configuration Structure
```rust
// In src/config.rs or new src/debug.rs
#[derive(Debug, Clone)]
pub struct DebugConfig {
    pub enabled: bool,
    pub verbosity: u8,  // 0-3 for debug levels
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
    
    pub fn should_trace(&self) -> bool {
        self.enabled && self.verbosity >= 2
    }
    
    pub fn should_show_context(&self) -> bool {
        self.enabled && self.verbosity >= 1
    }
}
```

#### 1.2: Add CLI Debug Flags
```rust
// In src/cli.rs (or wherever CLI args are defined)
#[derive(Parser)]
pub struct Args {
    // ... existing fields ...
    
    /// Enable debug output
    #[arg(long)]
    pub debug: bool,
    
    /// Verbose debug output (can be used multiple times: -v, -vv, -vvv)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    pub verbose: u8,
}
```

#### 1.3: Create Debug Context Tracking
```rust
// In src/debug.rs
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub position: Option<rhai::Position>,
    pub source_snippet: Option<String>,
    pub last_operation: Option<String>,
    pub error_location: Option<String>,
}

pub struct DebugTracker {
    config: DebugConfig,
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
}
```

#### 1.4: Register Rhai Debugging Callbacks
```rust
// In src/script_engine.rs (or wherever Rhai engine is set up)
use rhai::debugger::*;

pub fn setup_debug_engine(config: &DebugConfig) -> rhai::Engine {
    let mut engine = rhai::Engine::new();
    
    if config.enabled {
        let debug_tracker = DebugTracker::new(config.clone());
        
        engine.register_debugger(
            |_engine, debugger| {
                // Initialize debugger - no breakpoints for now
                debugger
            },
            move |_context, event, _node, source, pos| {
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
    
    engine
}
```

### Success Criteria
- `--debug` flag shows basic execution info
- No functional changes to existing behavior
- Rhai debugging callbacks are working
- Foundation ready for enhanced error reporting

### Testing
```bash
# Should show debug info
cargo run -- --debug --filter 'e.level == "ERROR"' test.jsonl

# Should work normally (no debug output)  
cargo run -- --filter 'e.level == "ERROR"' test.jsonl
```

---

## Phase 2: Enhanced Error Reporting (2-3 days)

### Goal
Implement the core enhanced error reporting using scope information and debugging context.

### Tasks

#### 2.1: Create Enhanced Error Display
```rust
// In src/debug.rs
use rhai::{EvalAltResult, Scope};

pub struct ErrorEnhancer {
    debug_config: DebugConfig,
}

impl ErrorEnhancer {
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
                let preview = self.format_value_preview(value);
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
```

#### 2.2: Integrate Enhanced Errors into Pipeline
```rust
// In src/pipeline.rs (or wherever script execution happens)
pub fn execute_stage(
    engine: &rhai::Engine,
    scope: &mut Scope,
    script: &str, 
    stage: &str,
    debug_config: &DebugConfig,
    execution_context: &ExecutionContext
) -> Result<rhai::Dynamic, String> {
    match engine.eval_with_scope::<rhai::Dynamic>(scope, script) {
        Ok(result) => Ok(result),
        Err(error) => {
            if debug_config.should_show_context() {
                let enhancer = ErrorEnhancer::new(debug_config.clone());
                let enhanced_error = enhancer.enhance_error(
                    &error, scope, script, stage, execution_context
                );
                Err(enhanced_error)
            } else {
                Err(format!("Script error: {}", error))
            }
        }
    }
}
```

#### 2.3: Update Debug Callbacks to Capture Context
```rust
// Update the debugging callback from Phase 1
engine.register_debugger(
    |_engine, debugger| debugger,
    move |_context, event, _node, source, pos| {
        // Update execution context
        if let Ok(mut ctx) = debug_tracker.context.lock() {
            ctx.position = pos;
            if let Some(src) = source {
                ctx.source_snippet = Some(src.to_string());
            }
        }
        
        // Log events based on verbosity
        match event {
            DebuggerEvent::Start => {
                debug_tracker.log_basic("Script execution started");
            },
            DebuggerEvent::End => {
                debug_tracker.log_basic("Script execution completed");
            },
            _ => {}
        }
        
        Ok(DebuggerCommand::Continue)
    }
);
```

### Success Criteria
- `--debug -v` shows enhanced error messages with scope information
- Variable suggestions work for common typos
- Stage-specific help appears in error messages
- Execution context is captured from debugging callbacks

### Testing
```bash
# Test enhanced error reporting
cargo run -- --debug -v --filter 'user_data.active == true' test.jsonl
# Should show variables in scope and suggestions

# Test normal error (should be unchanged)
cargo run -- --filter 'user_data.active == true' test.jsonl
```

---

## Phase 3: Execution Tracing (2-3 days)

### Goal
Add step-by-step execution tracing for `-vv` verbosity level.

### Tasks

#### 3.1: Implement Expression Tracing
```rust
// In src/debug.rs
pub struct ExecutionTracer {
    config: DebugConfig,
    current_event: Arc<Mutex<u64>>,
}

impl ExecutionTracer {
    pub fn trace_step(&self, 
        event_num: u64,
        step_info: &str, 
        result: &str
    ) {
        if self.config.verbosity >= 2 {
            eprintln!("  → {} → {}", step_info, result);
        }
    }
    
    pub fn trace_event_start(&self, event_num: u64, event_data: &str) {
        if self.config.verbosity >= 2 {
            eprintln!("Debug: Filter execution trace for event {}:", event_num);
            eprintln!("  Event: {}", self.truncate_for_display(event_data, 100));
        }
    }
    
    pub fn trace_event_result(&self, result: bool, action: &str) {
        if self.config.verbosity >= 2 {
            eprintln!("  Result: {} ({})", result, action);
        }
    }
    
    fn truncate_for_display(&self, text: &str, max_len: usize) -> String {
        if text.len() > max_len {
            format!("{}...", &text[..max_len-3])
        } else {
            text.to_string()
        }
    }
}
```

#### 3.2: Add Breakpoint-Based Tracing
```rust
// Update debugging callback to support tracing
engine.register_debugger(
    move |_engine, mut debugger| {
        if config.trace_events {
            // Set breakpoints at key positions for tracing
            // This would require parsing the script to find interesting positions
            // For now, we'll trace at the step level
        }
        debugger
    },
    move |_context, event, _node, source, pos| {
        match event {
            DebuggerEvent::Step => {
                if debug_tracker.config.verbosity >= 2 {
                    if let Some(src) = source {
                        debug_tracker.log_step(src, pos);
                    }
                }
            },
            DebuggerEvent::Start => {
                debug_tracker.log_event_start();
            },
            DebuggerEvent::End => {
                debug_tracker.log_event_end();
            },
            _ => {}
        }
        
        Ok(DebuggerCommand::Continue)
    }
);
```

#### 3.3: Integrate Tracing with Pipeline
```rust
// Add event-level tracing
pub fn process_event_with_tracing(
    event: &Event,
    script: &str,
    tracer: &ExecutionTracer,
    event_num: u64
) -> Result<bool, String> {
    tracer.trace_event_start(event_num, &format!("{:?}", event));
    
    let result = execute_filter(event, script)?;
    
    let action = if result { "passed" } else { "filtered out" };
    tracer.trace_event_result(result, action);
    
    Ok(result)
}
```

### Success Criteria
- `--debug -vv` shows step-by-step execution tracing
- Users can see how complex expressions are evaluated
- Performance impact is acceptable (tracing only when requested)

### Testing
```bash
# Test execution tracing
cargo run -- --debug -vv --filter 'e.level == "ERROR" && e.user_id > 1000' test.jsonl
# Should show step-by-step evaluation
```

---

## Phase 4: Advanced Debug Features (2-3 days)

### Goal
Add the most detailed debugging level and optional interactive features.

### Tasks

#### 4.1: Implement Detailed Step-by-Step Tracing
```rust
// Enhanced tracing for -vvv level
pub struct DetailedTracer {
    config: DebugConfig,
    step_counter: Arc<Mutex<u32>>,
}

impl DetailedTracer {
    pub fn trace_detailed_step(&self, 
        context: &str,
        operation: &str, 
        input: &str,
        output: &str,
        step_type: &str
    ) {
        if self.config.verbosity >= 3 {
            let step_num = {
                let mut counter = self.step_counter.lock().unwrap();
                *counter += 1;
                *counter
            };
            
            eprintln!("  [Step {}:{}] {}: {} → {}", 
                step_num, context, operation, input, output);
        }
    }
}
```

#### 4.2: Add Interactive Debug Option
```rust
// Interactive debugging (opt-in via environment variable)
pub struct InteractiveDebugger {
    config: DebugConfig,
    interactive_enabled: bool,
}

impl InteractiveDebugger {
    pub fn new(config: DebugConfig) -> Self {
        InteractiveDebugger {
            config,
            interactive_enabled: std::env::var("KELORA_DEBUG_INTERACTIVE").is_ok(),
        }
    }
    
    pub fn maybe_interactive_break(&self, 
        context: &ExecutionContext,
        scope: &Scope,
        error: Option<&EvalAltResult>
    ) -> DebuggerCommand {
        if self.interactive_enabled && self.config.verbosity >= 3 {
            if error.is_some() || self.should_break_for_inspection() {
                return self.interactive_session(context, scope);
            }
        }
        DebuggerCommand::Continue
    }
    
    fn interactive_session(&self, 
        _context: &ExecutionContext, 
        scope: &Scope
    ) -> DebuggerCommand {
        use std::io::{self, Write};
        
        println!("\n🔍 Interactive Debug Session");
        println!("Variables in scope:");
        for (name, _is_const, value) in scope.iter() {
            println!("  {}: {:?}", name, value);
        }
        
        loop {
            print!("Debug> (s)tep, (c)ontinue, (i)nspect <var>, (q)uit? ");
            io::stdout().flush().unwrap();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            
            match input.trim().to_lowercase().as_str() {
                "s" | "step" => return DebuggerCommand::StepInto,
                "c" | "continue" => return DebuggerCommand::Continue,
                "q" | "quit" => std::process::exit(0),
                cmd if cmd.starts_with("i ") => {
                    let var_name = &cmd[2..];
                    self.inspect_variable(var_name, scope);
                },
                _ => println!("Unknown command. Use (s)tep, (c)ontinue, (i)nspect <var>, (q)uit"),
            }
        }
    }
    
    fn inspect_variable(&self, var_name: &str, scope: &Scope) {
        for (name, _is_const, value) in scope.iter() {
            if name == var_name {
                println!("Variable '{}': {:?} (type: {})", name, value, value.type_name());
                return;
            }
        }
        println!("Variable '{}' not found", var_name);
    }
    
    fn should_break_for_inspection(&self) -> bool {
        // Could implement smart breakpoint logic here
        false
    }
}
```

#### 4.3: Add Performance and Statistics Tracking
```rust
// In src/debug.rs
pub struct DebugStats {
    start_time: std::time::Instant,
    events_processed: u64,
    events_passed: u64,
    errors_encountered: u64,
    script_executions: u64,
}

impl DebugStats {
    pub fn report(&self, config: &DebugConfig) {
        if config.enabled {
            let duration = self.start_time.elapsed();
            eprintln!("Debug: Processing completed in {:?}", duration);
            eprintln!("Debug: {} events processed, {} passed filter", 
                self.events_processed, self.events_passed);
            if self.errors_encountered > 0 {
                eprintln!("Debug: {} errors encountered", self.errors_encountered);
            }
            if config.show_timing {
                eprintln!("Debug: {:.2} events/sec", 
                    self.events_processed as f64 / duration.as_secs_f64());
            }
        }
    }
}
```

### Success Criteria
- `--debug -vvv` shows maximum detail tracing
- `KELORA_DEBUG_INTERACTIVE=1` enables interactive debugging
- Performance statistics are shown
- All debug levels work together harmoniously

### Testing
```bash
# Test maximum verbosity
cargo run -- --debug -vvv --filter 'complex_function(e)' test.jsonl

# Test interactive mode
KELORA_DEBUG_INTERACTIVE=1 cargo run -- --debug -vvv --filter 'e.level == "ERROR"' test.jsonl
```

---

## Phase 5: Polish and Integration (1-2 days)

### Goal
Clean up the implementation, add documentation, and ensure everything works together.

### Tasks

#### 5.1: Integration Testing
- Test all debug levels work correctly
- Verify backward compatibility
- Performance testing with debug enabled/disabled
- Edge case testing (empty files, malformed JSON, etc.)

#### 5.2: Documentation Updates
- Update CLI help text
- Add examples to README
- Document environment variables
- Create debugging guide

#### 5.3: Code Cleanup
- Remove any TODO comments
- Ensure consistent error message formatting
- Add proper error handling for debug code
- Optimize performance-critical paths

### Success Criteria
- All features work as designed
- Documentation is complete
- No performance regressions
- Code is production-ready

---

## Implementation Notes

### Error Handling Strategy
- Debug code should never crash the main application
- All debug operations should be wrapped in proper error handling
- Failed debug operations should log to stderr but not stop processing

### Performance Considerations
- Debug code should have minimal impact when disabled
- Use lazy evaluation for expensive debug operations
- Consider making some debug features compile-time optional

### Testing Strategy
- Unit tests for debug functionality
- Integration tests for CLI flags
- Performance benchmarks with debug enabled/disabled
- Test with various log formats and edge cases

### Future Extensions
- Add debug output to file option (`--debug-output=file.log`)
- Web-based debug interface for complex analysis
- Integration with external debugging tools
- Debug script validation and syntax checking

This plan provides a clear, incremental path to implementing comprehensive debugging features while maintaining kelora's reliability and performance.