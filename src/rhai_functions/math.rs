use rhai::Engine;

pub fn register_functions(engine: &mut Engine) {
    // Register modulo function since % operator seems to be missing
    engine.register_fn("mod", |a: i64, b: i64| -> i64 {
        if b == 0 {
            0 // Avoid division by zero
        } else {
            a % b
        }
    });

    // Also register it as % for completeness
    engine.register_fn("%", |a: i64, b: i64| -> i64 {
        if b == 0 {
            0 // Avoid division by zero
        } else {
            a % b
        }
    });

    // Register clamp function for integers
    engine.register_fn("clamp", clamp_i64);

    // Register clamp function for floats
    engine.register_fn("clamp", clamp_f64);
}

/// Clamp an integer value between a minimum and maximum
///
/// Constrains a value to be within a specified range. If the value is less than
/// the minimum, returns the minimum. If the value is greater than the maximum,
/// returns the maximum. Otherwise returns the value unchanged.
///
/// # Arguments
/// * `value` - The value to clamp
/// * `min` - The minimum allowed value
/// * `max` - The maximum allowed value
///
/// # Returns
/// The clamped value
///
/// # Examples
/// ```rhai
/// clamp(5, 0, 10)    // 5 (within range)
/// clamp(-5, 0, 10)   // 0 (below minimum)
/// clamp(15, 0, 10)   // 10 (above maximum)
/// clamp(50, 0, 100)  // 50 (within range)
///
/// // Useful for normalizing values
/// e.normalized_score = clamp(e.score, 0, 100);
/// e.safe_port = clamp(e.port, 1024, 65535);
/// e.cpu_pct = clamp(e.cpu_usage, 0, 100);
/// ```
///
/// # Panics
/// Panics if `min > max`.
fn clamp_i64(value: i64, min: i64, max: i64) -> i64 {
    value.clamp(min, max)
}

/// Clamp a floating-point value between a minimum and maximum
///
/// Constrains a value to be within a specified range. If the value is less than
/// the minimum, returns the minimum. If the value is greater than the maximum,
/// returns the maximum. Otherwise returns the value unchanged.
///
/// # Arguments
/// * `value` - The value to clamp
/// * `min` - The minimum allowed value
/// * `max` - The maximum allowed value
///
/// # Returns
/// The clamped value
///
/// # Examples
/// ```rhai
/// clamp(3.14, 0.0, 5.0)   // 3.14 (within range)
/// clamp(-1.5, 0.0, 5.0)   // 0.0 (below minimum)
/// clamp(7.8, 0.0, 5.0)    // 5.0 (above maximum)
///
/// // Useful for normalizing metrics
/// e.response_time_sec = clamp(e.response_time_ms / 1000.0, 0.0, 30.0);
/// e.normalized_latency = clamp(e.latency / e.baseline, 0.0, 10.0);
/// ```
///
/// # Panics
/// Panics if `min > max` or if either value is NaN.
fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_i64_within_range() {
        assert_eq!(clamp_i64(5, 0, 10), 5);
        assert_eq!(clamp_i64(50, 0, 100), 50);
        assert_eq!(clamp_i64(0, 0, 10), 0);
        assert_eq!(clamp_i64(10, 0, 10), 10);
    }

    #[test]
    fn test_clamp_i64_below_min() {
        assert_eq!(clamp_i64(-5, 0, 10), 0);
        assert_eq!(clamp_i64(-100, 0, 100), 0);
        assert_eq!(clamp_i64(-1, 0, 10), 0);
    }

    #[test]
    fn test_clamp_i64_above_max() {
        assert_eq!(clamp_i64(15, 0, 10), 10);
        assert_eq!(clamp_i64(200, 0, 100), 100);
        assert_eq!(clamp_i64(11, 0, 10), 10);
    }

    #[test]
    fn test_clamp_i64_negative_range() {
        assert_eq!(clamp_i64(-5, -10, -1), -5);
        assert_eq!(clamp_i64(-15, -10, -1), -10);
        assert_eq!(clamp_i64(0, -10, -1), -1);
    }

    #[test]
    #[should_panic(expected = "assertion failed: min <= max")]
    fn test_clamp_i64_inverted_range() {
        // When min > max, clamp panics (Rust's behavior)
        clamp_i64(5, 10, 0);
    }

    #[test]
    fn test_clamp_f64_within_range() {
        assert_eq!(clamp_f64(3.5, 0.0, 5.0), 3.5);
        assert_eq!(clamp_f64(2.5, 0.0, 10.0), 2.5);
        assert_eq!(clamp_f64(0.0, 0.0, 5.0), 0.0);
        assert_eq!(clamp_f64(5.0, 0.0, 5.0), 5.0);
    }

    #[test]
    fn test_clamp_f64_below_min() {
        assert_eq!(clamp_f64(-1.5, 0.0, 5.0), 0.0);
        assert_eq!(clamp_f64(-100.0, 0.0, 100.0), 0.0);
        assert_eq!(clamp_f64(-0.1, 0.0, 10.0), 0.0);
    }

    #[test]
    fn test_clamp_f64_above_max() {
        assert_eq!(clamp_f64(7.8, 0.0, 5.0), 5.0);
        assert_eq!(clamp_f64(200.5, 0.0, 100.0), 100.0);
        assert_eq!(clamp_f64(10.1, 0.0, 10.0), 10.0);
    }

    #[test]
    fn test_clamp_f64_negative_range() {
        assert_eq!(clamp_f64(-5.5, -10.0, -1.0), -5.5);
        assert_eq!(clamp_f64(-15.0, -10.0, -1.0), -10.0);
        assert_eq!(clamp_f64(0.0, -10.0, -1.0), -1.0);
    }

    #[test]
    #[should_panic(expected = "min > max")]
    fn test_clamp_f64_inverted_range() {
        // When min > max, clamp panics (Rust's behavior)
        clamp_f64(5.0, 10.0, 0.0);
    }

    #[test]
    fn test_clamp_f64_fractional() {
        assert_eq!(clamp_f64(0.5, 0.0, 1.0), 0.5);
        assert_eq!(clamp_f64(1.5, 0.0, 1.0), 1.0);
        assert_eq!(clamp_f64(-0.5, 0.0, 1.0), 0.0);
    }
}
