use rhai::Engine;

/// Get an environment variable value, returning empty string if not found
fn get_env_impl(var: &str) -> String {
    std::env::var(var).unwrap_or_default()
}

/// Get an environment variable value with a default fallback
fn get_env_with_default_impl(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.to_string())
}

/// Register environment functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("get_env", get_env_impl);
    engine.register_fn("get_env", get_env_with_default_impl);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_env_existing() {
        // Set a test environment variable
        std::env::set_var("TEST_VAR_EXISTING", "test_value");

        let result = get_env_impl("TEST_VAR_EXISTING");
        assert_eq!(result, "test_value");

        // Clean up
        std::env::remove_var("TEST_VAR_EXISTING");
    }

    #[test]
    fn test_get_env_missing() {
        // Ensure the variable doesn't exist
        std::env::remove_var("TEST_VAR_MISSING");

        let result = get_env_impl("TEST_VAR_MISSING");
        assert_eq!(result, "");
    }

    #[test]
    fn test_get_env_with_default_existing() {
        // Set a test environment variable
        std::env::set_var("TEST_VAR_DEFAULT_EXISTING", "actual_value");

        let result = get_env_with_default_impl("TEST_VAR_DEFAULT_EXISTING", "default_value");
        assert_eq!(result, "actual_value");

        // Clean up
        std::env::remove_var("TEST_VAR_DEFAULT_EXISTING");
    }

    #[test]
    fn test_get_env_with_default_missing() {
        // Ensure the variable doesn't exist
        std::env::remove_var("TEST_VAR_DEFAULT_MISSING");

        let result = get_env_with_default_impl("TEST_VAR_DEFAULT_MISSING", "default_value");
        assert_eq!(result, "default_value");
    }

    #[test]
    fn test_get_env_with_default_empty_default() {
        // Ensure the variable doesn't exist
        std::env::remove_var("TEST_VAR_EMPTY_DEFAULT");

        let result = get_env_with_default_impl("TEST_VAR_EMPTY_DEFAULT", "");
        assert_eq!(result, "");
    }
}
