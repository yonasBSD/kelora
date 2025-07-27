use rhai::{Dynamic, Engine, ImmutableString};
use std::cell::Cell;

// Use thread-local storage for exit state to handle parallel processing correctly
thread_local! {
    static EXIT_REQUESTED: Cell<bool> = const { Cell::new(false) };
    static EXIT_CODE: Cell<i32> = const { Cell::new(0) };
}

/// Rhai function: exit(code: INT, msg: STRING = null)
/// Immediately stops all event processing and terminates Kelora with the given exit code.
pub fn exit_process(code: i64, msg: Dynamic) -> Dynamic {
    // Store exit code (clamp to valid range for process exit codes)
    let exit_code = code.clamp(0, 255) as i32;
    EXIT_CODE.with(|ec| ec.set(exit_code));

    // Print message to stderr if provided
    if !msg.is_unit() {
        if let Some(s) = msg.read_lock::<ImmutableString>() {
            eprintln!("{}", s.as_str());
        } else {
            eprintln!("{}", msg);
        }
    }

    // Set exit flag
    EXIT_REQUESTED.with(|er| er.set(true));

    // In testing, don't actually exit - just set the flags
    #[cfg(not(test))]
    {
        std::process::exit(exit_code);
    }

    // Return unit to indicate function completed (only reached in tests)
    #[cfg(test)]
    Dynamic::UNIT
}

/// Check if exit has been requested from Rhai scripts
pub fn is_exit_requested() -> bool {
    EXIT_REQUESTED.with(|er| er.get())
}

/// Get the requested exit code
pub fn get_exit_code() -> i32 {
    EXIT_CODE.with(|ec| ec.get())
}

/// Reset exit state (useful for testing)
#[cfg(test)]
pub fn reset_exit_state() {
    EXIT_REQUESTED.with(|er| er.set(false));
    EXIT_CODE.with(|ec| ec.set(0));
}

/// Rhai function wrapper for single parameter: exit(code)
pub fn exit_process_single(code: i64) -> Dynamic {
    exit_process(code, Dynamic::UNIT)
}

/// Register process control functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("exit", exit_process_single);
    engine.register_fn("exit", exit_process);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::Engine;

    #[test]
    fn test_exit_with_code_only() {
        reset_exit_state();

        let result = exit_process(42, Dynamic::UNIT);

        assert!(result.is_unit());
        assert!(is_exit_requested());
        assert_eq!(get_exit_code(), 42);
    }

    #[test]
    fn test_exit_with_message() {
        reset_exit_state();

        let msg = Dynamic::from("Test error message");
        let result = exit_process(1, msg);

        assert!(result.is_unit());
        assert!(is_exit_requested());
        assert_eq!(get_exit_code(), 1);
    }

    #[test]
    fn test_exit_code_clamping() {
        reset_exit_state();

        // Test negative code
        let _ = exit_process(-5, Dynamic::UNIT);
        assert_eq!(get_exit_code(), 0);

        reset_exit_state();

        // Test code > 255
        let _ = exit_process(300, Dynamic::UNIT);
        assert_eq!(get_exit_code(), 255);
    }

    #[test]
    fn test_rhai_integration() {
        reset_exit_state();

        let mut engine = Engine::new();
        register_functions(&mut engine);

        // Test exit with code only
        let result = engine.eval::<Dynamic>("exit(123)");
        if let Err(e) = &result {
            eprintln!("Rhai error: {}", e);
        }
        assert!(result.is_ok());

        eprintln!("Exit requested: {}", is_exit_requested());
        eprintln!("Exit code: {}", get_exit_code());

        assert!(is_exit_requested());
        assert_eq!(get_exit_code(), 123);

        reset_exit_state();

        // Test exit with message
        let result = engine.eval::<Dynamic>(r#"exit(1, "Error occurred")"#);
        assert!(result.is_ok());
        assert!(is_exit_requested());
        assert_eq!(get_exit_code(), 1);
    }
}
