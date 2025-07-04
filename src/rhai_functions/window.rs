use rhai::{Array, Dynamic, Engine, EvalAltResult};

/// Register window-related functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    // Register window helper functions
    engine.register_fn("window_values", window_values);
    engine.register_fn("window_numbers", window_numbers);
}

/// Extract field values from all events in the window
///
/// Extracts the specified field from each event in the window where the field exists.
/// Events without the field are skipped (not included in results).
///
/// # Arguments
/// * `window` - Array of events from the window context
/// * `field_name` - Name of the field to extract from each event
///
/// # Returns
/// Array of field values (as strings) from events that have the field.
/// Results are in window order: [current_value, previous_value, older_value, ...]
///
/// # Examples
/// ```rhai
/// // Get all status codes from window events
/// let statuses = window_values(window, "status");  // ["200", "404", "200"]
///
/// // Get all user names (skips events without "user" field)
/// let users = window_values(window, "user");       // ["alice", "bob"]
///
/// // Get log messages
/// let messages = window_values(window, "message"); // ["Login successful", "Access denied"]
/// ```
///
/// # Implementation Notes
/// - Only includes events where the field exists and is not null
/// - Field values are converted to strings for consistent output
/// - Empty result array if no events have the specified field
/// - Maintains window order (current event first)
fn window_values(window: Array, field_name: String) -> Result<Array, Box<EvalAltResult>> {
    let mut results = Array::new();

    for event_dynamic in window {
        // Try to convert to map (event object)
        if let Some(event_map) = event_dynamic.try_cast::<rhai::Map>() {
            // Extract field value if it exists (convert field_name to str for key lookup)
            if let Some(field_value) = event_map.get(field_name.as_str()) {
                // Convert to string representation
                let value_str = if field_value.is_string() {
                    field_value.clone().into_string().unwrap_or_default()
                } else {
                    field_value.to_string()
                };

                // Only include non-empty values
                if !value_str.is_empty() {
                    results.push(Dynamic::from(value_str));
                }
            }
        }
    }

    Ok(results)
}

/// Extract numeric field values from all events in the window
///
/// Extracts the specified field from each event in the window and attempts to
/// convert it to a number. Events without the field or non-numeric values are skipped.
///
/// # Arguments
/// * `window` - Array of events from the window context  
/// * `field_name` - Name of the field to extract from each event
///
/// # Returns
/// Array of numeric values (as floats) from events that have parseable numeric fields.
/// Results are in window order: [current_value, previous_value, older_value, ...]
///
/// # Examples
/// ```rhai
/// // Get all response times as numbers
/// let times = window_numbers(window, "response_time");  // [0.15, 0.23, 0.89]
///
/// // Get all error counts
/// let errors = window_numbers(window, "error_count");   // [0, 2, 1]
///
/// // Get status codes as numbers
/// let codes = window_numbers(window, "status");         // [200, 404, 500]
/// ```
///
/// # Parsing Behavior
/// - String values are parsed as numbers ("123" -> 123.0, "45.67" -> 45.67)
/// - Integer values are converted to floats (123 -> 123.0)
/// - Boolean values: true -> 1.0, false -> 0.0
/// - Non-parseable values are skipped (not included in results)
/// - null/missing fields are skipped
///
/// # Implementation Notes
/// - Results are always returned as floating-point numbers for consistency
/// - Empty result array if no events have parseable numeric values for the field
/// - Maintains window order (current event first)
fn window_numbers(window: Array, field_name: String) -> Result<Array, Box<EvalAltResult>> {
    let mut results = Array::new();

    for event_dynamic in window {
        // Try to convert to map (event object)
        if let Some(event_map) = event_dynamic.try_cast::<rhai::Map>() {
            // Extract field value if it exists (convert field_name to str for key lookup)
            if let Some(field_value) = event_map.get(field_name.as_str()) {
                // Try to convert to number
                let number_opt = if field_value.is_int() {
                    Some(field_value.as_int().unwrap_or(0) as f64)
                } else if field_value.is_float() {
                    Some(field_value.as_float().unwrap_or(0.0))
                } else if field_value.is_bool() {
                    Some(if field_value.as_bool().unwrap_or(false) {
                        1.0
                    } else {
                        0.0
                    })
                } else if field_value.is_string() {
                    // Try to parse string as number
                    let str_value = field_value.clone().into_string().unwrap_or_default();
                    str_value.parse::<f64>().ok()
                } else {
                    // Try parsing string representation
                    field_value.to_string().parse::<f64>().ok()
                };

                // Include if we successfully parsed a number
                if let Some(number) = number_opt {
                    results.push(Dynamic::from(number));
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::{Array, Dynamic, Map};

    fn create_test_event(fields: Vec<(&str, Dynamic)>) -> Dynamic {
        let mut map = Map::new();
        for (key, value) in fields {
            map.insert(key.into(), value);
        }
        Dynamic::from(map)
    }

    #[test]
    fn test_window_values_basic() {
        let mut window = Array::new();

        // Add events with status field
        window.push(create_test_event(vec![("status", Dynamic::from("200"))]));
        window.push(create_test_event(vec![("status", Dynamic::from("404"))]));
        window.push(create_test_event(vec![("status", Dynamic::from("500"))]));

        let result = window_values(window, "status".to_string()).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].clone().into_string().unwrap(), "200");
        assert_eq!(result[1].clone().into_string().unwrap(), "404");
        assert_eq!(result[2].clone().into_string().unwrap(), "500");
    }

    #[test]
    fn test_window_values_missing_fields() {
        let mut window = Array::new();

        // Mix of events with and without status field
        window.push(create_test_event(vec![("status", Dynamic::from("200"))]));
        window.push(create_test_event(vec![("message", Dynamic::from("error"))])); // no status
        window.push(create_test_event(vec![("status", Dynamic::from("404"))]));

        let result = window_values(window, "status".to_string()).unwrap();

        assert_eq!(result.len(), 2); // Only events with status field
        assert_eq!(result[0].clone().into_string().unwrap(), "200");
        assert_eq!(result[1].clone().into_string().unwrap(), "404");
    }

    #[test]
    fn test_window_numbers_basic() {
        let mut window = Array::new();

        // Add events with numeric fields
        window.push(create_test_event(vec![(
            "response_time",
            Dynamic::from(0.15),
        )]));
        window.push(create_test_event(vec![(
            "response_time",
            Dynamic::from(0.23),
        )]));
        window.push(create_test_event(vec![(
            "response_time",
            Dynamic::from(0.89),
        )]));

        let result = window_numbers(window, "response_time".to_string()).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].as_float().unwrap(), 0.15);
        assert_eq!(result[1].as_float().unwrap(), 0.23);
        assert_eq!(result[2].as_float().unwrap(), 0.89);
    }

    #[test]
    fn test_window_numbers_string_parsing() {
        let mut window = Array::new();

        // Add events with string numeric values
        window.push(create_test_event(vec![("count", Dynamic::from("123"))]));
        window.push(create_test_event(vec![("count", Dynamic::from("45.67"))]));
        window.push(create_test_event(vec![("count", Dynamic::from("invalid"))])); // should be skipped

        let result = window_numbers(window, "count".to_string()).unwrap();

        assert_eq!(result.len(), 2); // Only parseable numbers
        assert_eq!(result[0].as_float().unwrap(), 123.0);
        assert_eq!(result[1].as_float().unwrap(), 45.67);
    }

    #[test]
    fn test_window_numbers_mixed_types() {
        let mut window = Array::new();

        // Mix of different numeric types
        window.push(create_test_event(vec![("value", Dynamic::from(42i64))])); // int
        window.push(create_test_event(vec![("value", Dynamic::from(3.14))])); // float
        window.push(create_test_event(vec![("value", Dynamic::from(true))])); // bool -> 1.0
        window.push(create_test_event(vec![("value", Dynamic::from(false))])); // bool -> 0.0

        let result = window_numbers(window, "value".to_string()).unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].as_float().unwrap(), 42.0);
        assert_eq!(result[1].as_float().unwrap(), 3.14);
        assert_eq!(result[2].as_float().unwrap(), 1.0);
        assert_eq!(result[3].as_float().unwrap(), 0.0);
    }

    #[test]
    fn test_empty_window() {
        let window = Array::new();

        let values_result = window_values(window.clone(), "field".to_string()).unwrap();
        let numbers_result = window_numbers(window, "field".to_string()).unwrap();

        assert_eq!(values_result.len(), 0);
        assert_eq!(numbers_result.len(), 0);
    }
}
