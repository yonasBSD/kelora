use rhai::{Array, Dynamic, Engine, EvalAltResult};

/// Register window-related functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    // Register window helper functions
    engine.register_fn("window_values", window_values);
    engine.register_fn("window_numbers", window_numbers);
    // Register array statistical functions
    engine.register_fn("percentile", percentile);
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

/// Calculate percentile of numeric array
///
/// Calculates the specified percentile of numeric values in an array using linear
/// interpolation. Works with any array of numeric values, including results from
/// window_numbers() or any other source.
///
/// # Arguments
/// * `arr` - Array of numeric values (integers, floats, or convertible strings)
/// * `p` - Percentile to calculate (0-100, e.g., 95 for P95)
///
/// # Returns
/// Percentile value as a float using linear interpolation method.
/// Non-numeric values in the array are filtered out before calculation.
///
/// # Examples
/// ```rhai
/// // P95 response time from window
/// let p95 = window_numbers(window, "response_time").percentile(95);
/// if p95 > 0.5 { print("SLA breach: P95=" + p95) }
///
/// // Median calculation
/// let median = [1, 2, 3, 4, 5].percentile(50);  // 3.0
///
/// // P99 latency monitoring
/// let p99 = window_numbers(window, "latency").percentile(99);
/// ```
///
/// # Error Cases
/// - Empty array or no numeric values: returns error
/// - Percentile outside 0-100 range: returns error
///
/// # Implementation Notes
/// - Uses linear interpolation for percentile calculation
/// - Automatically filters out non-numeric values
/// - Handles mixed type arrays (int, float, string numbers)
/// - Maintains precision with floating-point results
fn percentile(arr: Array, p: f64) -> Result<f64, Box<EvalAltResult>> {
    if arr.is_empty() {
        return Err("Cannot calculate percentile of empty array".into());
    }

    if !(0.0..=100.0).contains(&p) {
        return Err("Percentile must be between 0 and 100".into());
    }

    // Convert Dynamic values to f64, filtering out non-numeric values
    let mut values: Vec<f64> = Vec::new();

    for item in arr {
        let number_opt = if item.is_int() {
            Some(item.as_int().unwrap_or(0) as f64)
        } else if item.is_float() {
            Some(item.as_float().unwrap_or(0.0))
        } else if item.is_bool() {
            Some(if item.as_bool().unwrap_or(false) {
                1.0
            } else {
                0.0
            })
        } else if item.is_string() {
            // Try to parse string as number
            let str_value = item.clone().into_string().unwrap_or_default();
            str_value.parse::<f64>().ok()
        } else {
            // Try parsing string representation
            item.to_string().parse::<f64>().ok()
        };

        if let Some(number) = number_opt {
            values.push(number);
        }
    }

    if values.is_empty() {
        return Err("No numeric values found in array".into());
    }

    // Sort values for percentile calculation
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Calculate percentile using linear interpolation
    let index = (p / 100.0) * (values.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;

    if lower == upper {
        // Exact index, no interpolation needed
        Ok(values[lower])
    } else {
        // Linear interpolation between lower and upper values
        let weight = index - lower as f64;
        Ok(values[lower] * (1.0 - weight) + values[upper] * weight)
    }
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
        window.push(create_test_event(vec![("value", Dynamic::from(2.5))])); // float
        window.push(create_test_event(vec![("value", Dynamic::from(true))])); // bool -> 1.0
        window.push(create_test_event(vec![("value", Dynamic::from(false))])); // bool -> 0.0

        let result = window_numbers(window, "value".to_string()).unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].as_float().unwrap(), 42.0);
        assert_eq!(result[1].as_float().unwrap(), 2.5);
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

    #[test]
    fn test_percentile_basic() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(1.0));
        arr.push(Dynamic::from(2.0));
        arr.push(Dynamic::from(3.0));
        arr.push(Dynamic::from(4.0));
        arr.push(Dynamic::from(5.0));

        // Test median (50th percentile)
        let median = percentile(arr.clone(), 50.0).unwrap();
        assert_eq!(median, 3.0);

        // Test minimum (0th percentile)
        let min = percentile(arr.clone(), 0.0).unwrap();
        assert_eq!(min, 1.0);

        // Test maximum (100th percentile)
        let max = percentile(arr, 100.0).unwrap();
        assert_eq!(max, 5.0);
    }

    #[test]
    fn test_percentile_interpolation() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(1.0));
        arr.push(Dynamic::from(2.0));
        arr.push(Dynamic::from(3.0));
        arr.push(Dynamic::from(4.0));

        // Test 25th percentile (should interpolate between 1 and 2)
        let p25 = percentile(arr.clone(), 25.0).unwrap();
        assert_eq!(p25, 1.75);

        // Test 75th percentile (should interpolate between 3 and 4)
        let p75 = percentile(arr, 75.0).unwrap();
        assert_eq!(p75, 3.25);
    }

    #[test]
    fn test_percentile_mixed_types() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(42i64)); // int
        arr.push(Dynamic::from(2.5)); // float
        arr.push(Dynamic::from("123.5")); // string number
        arr.push(Dynamic::from(true)); // bool -> 1.0
        arr.push(Dynamic::from(false)); // bool -> 0.0

        let median = percentile(arr, 50.0).unwrap();
        // Sorted: [0.0, 1.0, 2.5, 42.0, 123.5] -> median = 2.5
        assert_eq!(median, 2.5);
    }

    #[test]
    fn test_percentile_filters_non_numeric() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(1.0));
        arr.push(Dynamic::from("not_a_number"));
        arr.push(Dynamic::from(3.0));
        arr.push(Dynamic::from(5.0));

        let median = percentile(arr, 50.0).unwrap();
        // Should ignore "not_a_number", work with [1.0, 3.0, 5.0] -> median = 3.0
        assert_eq!(median, 3.0);
    }

    #[test]
    fn test_percentile_error_cases() {
        let empty_arr = Array::new();
        assert!(percentile(empty_arr, 50.0).is_err());

        let mut non_numeric_arr = Array::new();
        non_numeric_arr.push(Dynamic::from("text"));
        non_numeric_arr.push(Dynamic::from("more_text"));
        assert!(percentile(non_numeric_arr, 50.0).is_err());

        let mut valid_arr = Array::new();
        valid_arr.push(Dynamic::from(1.0));
        assert!(percentile(valid_arr.clone(), -1.0).is_err()); // Invalid percentile
        assert!(percentile(valid_arr, 101.0).is_err()); // Invalid percentile
    }

    #[test]
    fn test_percentile_single_value() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(42.0));

        // All percentiles should return the single value
        assert_eq!(percentile(arr.clone(), 0.0).unwrap(), 42.0);
        assert_eq!(percentile(arr.clone(), 50.0).unwrap(), 42.0);
        assert_eq!(percentile(arr, 100.0).unwrap(), 42.0);
    }
}
