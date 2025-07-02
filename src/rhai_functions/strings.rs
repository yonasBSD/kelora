use rhai::{Engine, Dynamic};
use std::cell::RefCell;

thread_local! {
    static CAPTURED_PRINTS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static CAPTURED_EPRINTS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static PARALLEL_MODE: RefCell<bool> = RefCell::new(false);
}

/// Capture a print statement in thread-local storage for parallel processing
pub fn capture_print(message: String) {
    CAPTURED_PRINTS.with(|prints| {
        prints.borrow_mut().push(message);
    });
}

/// Capture an eprint statement in thread-local storage for parallel processing
pub fn capture_eprint(message: String) {
    CAPTURED_EPRINTS.with(|eprints| {
        eprints.borrow_mut().push(message);
    });
}

/// Get all captured prints and clear the buffer
pub fn take_captured_prints() -> Vec<String> {
    CAPTURED_PRINTS.with(|prints| {
        std::mem::take(&mut *prints.borrow_mut())
    })
}

/// Get all captured eprints and clear the buffer
pub fn take_captured_eprints() -> Vec<String> {
    CAPTURED_EPRINTS.with(|eprints| {
        std::mem::take(&mut *eprints.borrow_mut())
    })
}

/// Clear captured prints without returning them
pub fn clear_captured_prints() {
    CAPTURED_PRINTS.with(|prints| {
        prints.borrow_mut().clear();
    });
}

/// Clear captured eprints without returning them
pub fn clear_captured_eprints() {
    CAPTURED_EPRINTS.with(|eprints| {
        eprints.borrow_mut().clear();
    });
}

/// Set whether we're in parallel processing mode
pub fn set_parallel_mode(enabled: bool) {
    PARALLEL_MODE.with(|mode| {
        *mode.borrow_mut() = enabled;
    });
}

/// Check if we're in parallel processing mode
pub fn is_parallel_mode() -> bool {
    PARALLEL_MODE.with(|mode| *mode.borrow())
}

pub fn register_functions(engine: &mut Engine) {
    // Note: print() function is now handled via engine.on_print() in engine.rs
    
    // Custom eprint function that captures output in parallel mode
    engine.register_fn("eprint", |message: rhai::Dynamic| {
        let msg = message.to_string();
        if is_parallel_mode() {
            capture_eprint(msg);
        } else {
            eprintln!("{}", msg);
        }
    });

    // Existing string functions from engine.rs
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

    // String processing functions (literal string matching, not regex)
    engine.register_fn("after", |text: &str, substring: &str| -> String {
        if let Some(pos) = text.find(substring) {
            text[pos + substring.len()..].to_string()
        } else {
            String::new()
        }
    });

    engine.register_fn("before", |text: &str, substring: &str| -> String {
        if let Some(pos) = text.find(substring) {
            text[..pos].to_string()
        } else {
            String::new()
        }
    });

    engine.register_fn("between", |text: &str, start_substring: &str, end_substring: &str| -> String {
        if let Some(start_pos) = text.find(start_substring) {
            let start_idx = start_pos + start_substring.len();
            let remainder = &text[start_idx..];
            
            if end_substring.is_empty() {
                // Empty end substring means "to end of string"
                remainder.to_string()
            } else if let Some(end_pos) = remainder.find(end_substring) {
                remainder[..end_pos].to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    });

    engine.register_fn("starting_with", |text: &str, prefix: &str| -> String {
        if text.starts_with(prefix) {
            text.to_string()
        } else {
            String::new()
        }
    });

    engine.register_fn("ending_with", |text: &str, suffix: &str| -> String {
        if text.ends_with(suffix) {
            text.to_string()
        } else {
            String::new()
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::Scope;

    #[test]
    fn test_after_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);
        
        let mut scope = Scope::new();
        scope.push("text", "hello world test");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.after("world")"#).unwrap();
        assert_eq!(result, " test");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.after("missing")"#).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_before_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);
        
        let mut scope = Scope::new();
        scope.push("text", "hello world test");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.before("world")"#).unwrap();
        assert_eq!(result, "hello ");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.before("missing")"#).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_between_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);
        
        let mut scope = Scope::new();
        scope.push("text", "start[content]end");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.between("[", "]")"#).unwrap();
        assert_eq!(result, "content");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.between("missing", "]")"#).unwrap();
        assert_eq!(result, "");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.between("[", "missing")"#).unwrap();
        assert_eq!(result, "");
        
        // Test empty end substring - should return everything after start
        let result: String = engine.eval_with_scope(&mut scope, r#"text.between("[", "")"#).unwrap();
        assert_eq!(result, "content]end");
        
        scope.push("log", "ERROR: connection failed");
        let result: String = engine.eval_with_scope(&mut scope, r#"log.between("ERROR: ", "")"#).unwrap();
        assert_eq!(result, "connection failed");
    }

    #[test]
    fn test_starting_with_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);
        
        let mut scope = Scope::new();
        scope.push("text", "hello world");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.starting_with("hello")"#).unwrap();
        assert_eq!(result, "hello world");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.starting_with("world")"#).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_ending_with_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);
        
        let mut scope = Scope::new();
        scope.push("text", "hello world");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.ending_with("world")"#).unwrap();
        assert_eq!(result, "hello world");
        
        let result: String = engine.eval_with_scope(&mut scope, r#"text.ending_with("hello")"#).unwrap();
        assert_eq!(result, "");
    }
}