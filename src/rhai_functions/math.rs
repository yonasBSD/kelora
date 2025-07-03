use rhai::{Engine, Dynamic};

pub fn register_functions(engine: &mut Engine) {
    // Register modulo function since % operator seems to be missing
    engine.register_fn("mod", |a: i64, b: i64| -> i64 {
        if b == 0 {
            0  // Avoid division by zero
        } else {
            a % b
        }
    });
    
    // Also register it as % for completeness
    engine.register_fn("%", |a: i64, b: i64| -> i64 {
        if b == 0 {
            0  // Avoid division by zero
        } else {
            a % b
        }
    });
}