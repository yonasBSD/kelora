use rhai::{Dynamic, Engine};
use std::cell::RefCell;
use std::collections::HashMap;

// Thread-local storage for tracking state
thread_local! {
    static THREAD_TRACKING_STATE: RefCell<HashMap<String, Dynamic>> = RefCell::new(HashMap::new());
}

// Error tracking for summary collection in parallel mode
pub fn track_error(error_type: &str, message: &str) {
    THREAD_TRACKING_STATE.with(|state| {
        let mut state = state.borrow_mut();
        
        // Track error count
        let count_key = format!("__kelora_error_count_{}", error_type);
        let count = state.get(&count_key).cloned().unwrap_or(Dynamic::from(0i64));
        let new_count = count.as_int().unwrap_or(0) + 1;
        state.insert(count_key.clone(), Dynamic::from(new_count));
        state.insert(format!("__op_{}", count_key), Dynamic::from("count"));
        
        // Track error examples (up to 3 per type)
        let examples_key = format!("__kelora_error_examples_{}", error_type);
        let current = state.get(&examples_key).cloned().unwrap_or_else(|| {
            Dynamic::from(rhai::Array::new())
        });
        
        if let Ok(mut arr) = current.into_array() {
            if arr.len() < 3 {
                arr.push(Dynamic::from(message.to_string()));
                state.insert(examples_key.clone(), Dynamic::from(arr));
                state.insert(format!("__op_{}", examples_key), Dynamic::from("error_examples"));
            }
        }
    });
}

pub fn register_functions(engine: &mut Engine) {
    // Track functions using thread-local storage - clean user API
    // Store operation metadata for proper parallel merging
    engine.register_fn("track_count", |key: &str| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let count = state.get(key).cloned().unwrap_or(Dynamic::from(0i64));
            let new_count = count.as_int().unwrap_or(0) + 1;
            state.insert(key.to_string(), Dynamic::from(new_count));
            // Store operation type metadata for parallel merging
            state.insert(format!("__op_{}", key), Dynamic::from("count"));
        });
    });

    engine.register_fn("track_count", |key: &str, delta: i64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let count = state.get(key).cloned().unwrap_or(Dynamic::from(0i64));
            let new_count = count.as_int().unwrap_or(0) + delta;
            state.insert(key.to_string(), Dynamic::from(new_count));
            // Store operation type metadata for parallel merging
            state.insert(format!("__op_{}", key), Dynamic::from("count"));
        });
    });

    engine.register_fn("track_count", |key: &str, delta: i32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let count = state.get(key).cloned().unwrap_or(Dynamic::from(0i64));
            let new_count = count.as_int().unwrap_or(0) + (delta as i64);
            state.insert(key.to_string(), Dynamic::from(new_count));
            // Store operation type metadata for parallel merging
            state.insert(format!("__op_{}", key), Dynamic::from("count"));
        });
    });

    engine.register_fn("track_count", |key: &str, delta: f64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let count = state.get(key).cloned().unwrap_or(Dynamic::from(0i64));
            let new_count = count.as_int().unwrap_or(0) + (delta as i64);
            state.insert(key.to_string(), Dynamic::from(new_count));
            // Store operation type metadata for parallel merging
            state.insert(format!("__op_{}", key), Dynamic::from("count"));
        });
    });

    engine.register_fn("track_count", |key: &str, delta: f32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let count = state.get(key).cloned().unwrap_or(Dynamic::from(0i64));
            let new_count = count.as_int().unwrap_or(0) + (delta as i64);
            state.insert(key.to_string(), Dynamic::from(new_count));
            // Store operation type metadata for parallel merging
            state.insert(format!("__op_{}", key), Dynamic::from("count"));
        });
    });

    // track_min overloads for different number types
    engine.register_fn("track_min", |key: &str, value: i64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state
                .get(key)
                .cloned()
                .unwrap_or(Dynamic::from(f64::INFINITY));
            let current_val = if current.is_int() {
                current.as_int().unwrap_or(i64::MAX) as f64
            } else {
                current.as_float().unwrap_or(f64::INFINITY)
            };
            let value_f64 = value as f64;
            if value_f64 < current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("min"));
            }
        });
    });

    engine.register_fn("track_min", |key: &str, value: i32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state
                .get(key)
                .cloned()
                .unwrap_or(Dynamic::from(f64::INFINITY));
            let current_val = if current.is_int() {
                current.as_int().unwrap_or(i64::MAX) as f64
            } else {
                current.as_float().unwrap_or(f64::INFINITY)
            };
            let value_f64 = value as f64;
            if value_f64 < current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("min"));
            }
        });
    });

    engine.register_fn("track_min", |key: &str, value: f64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state
                .get(key)
                .cloned()
                .unwrap_or(Dynamic::from(f64::INFINITY));
            let current_val = if current.is_int() {
                current.as_int().unwrap_or(i64::MAX) as f64
            } else {
                current.as_float().unwrap_or(f64::INFINITY)
            };
            if value < current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("min"));
            }
        });
    });

    engine.register_fn("track_min", |key: &str, value: f32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state
                .get(key)
                .cloned()
                .unwrap_or(Dynamic::from(f64::INFINITY));
            let current_val = if current.is_int() {
                current.as_int().unwrap_or(i64::MAX) as f64
            } else {
                current.as_float().unwrap_or(f64::INFINITY)
            };
            let value_f64 = value as f64;
            if value_f64 < current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("min"));
            }
        });
    });

    // track_max overloads for different number types
    engine.register_fn("track_max", |key: &str, value: i64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state
                .get(key)
                .cloned()
                .unwrap_or(Dynamic::from(f64::NEG_INFINITY));
            let current_val = if current.is_int() {
                current.as_int().unwrap_or(i64::MIN) as f64
            } else {
                current.as_float().unwrap_or(f64::NEG_INFINITY)
            };
            let value_f64 = value as f64;
            if value_f64 > current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("max"));
            }
        });
    });

    engine.register_fn("track_max", |key: &str, value: i32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state
                .get(key)
                .cloned()
                .unwrap_or(Dynamic::from(f64::NEG_INFINITY));
            let current_val = if current.is_int() {
                current.as_int().unwrap_or(i64::MIN) as f64
            } else {
                current.as_float().unwrap_or(f64::NEG_INFINITY)
            };
            let value_f64 = value as f64;
            if value_f64 > current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("max"));
            }
        });
    });

    engine.register_fn("track_max", |key: &str, value: f64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state
                .get(key)
                .cloned()
                .unwrap_or(Dynamic::from(f64::NEG_INFINITY));
            let current_val = if current.is_int() {
                current.as_int().unwrap_or(i64::MIN) as f64
            } else {
                current.as_float().unwrap_or(f64::NEG_INFINITY)
            };
            if value > current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("max"));
            }
        });
    });

    engine.register_fn("track_max", |key: &str, value: f32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state
                .get(key)
                .cloned()
                .unwrap_or(Dynamic::from(f64::NEG_INFINITY));
            let current_val = if current.is_int() {
                current.as_int().unwrap_or(i64::MIN) as f64
            } else {
                current.as_float().unwrap_or(f64::NEG_INFINITY)
            };
            let value_f64 = value as f64;
            if value_f64 > current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("max"));
            }
        });
    });

    engine.register_fn("track_unique", |key: &str, value: &str| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing set or create new one
            let current = state.get(key).cloned().unwrap_or_else(|| {
                // Create a new array to store unique values
                Dynamic::from(rhai::Array::new())
            });

            if let Ok(mut arr) = current.into_array() {
                let value_dynamic = Dynamic::from(value.to_string());
                // Check if value already exists in array
                if !arr
                    .iter()
                    .any(|v| v.clone().into_string().unwrap_or_default() == value)
                {
                    arr.push(value_dynamic);
                }
                state.insert(key.to_string(), Dynamic::from(arr));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("unique"));
            }
        });
    });

    engine.register_fn("track_unique", |key: &str, value: i64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing set or create new one
            let current = state.get(key).cloned().unwrap_or_else(|| {
                // Create a new array to store unique values
                Dynamic::from(rhai::Array::new())
            });

            if let Ok(mut arr) = current.into_array() {
                let value_dynamic = Dynamic::from(value);
                // Check if value already exists in array
                if !arr.iter().any(|v| v.as_int().unwrap_or(i64::MIN) == value) {
                    arr.push(value_dynamic);
                }
                state.insert(key.to_string(), Dynamic::from(arr));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("unique"));
            }
        });
    });

    engine.register_fn("track_unique", |key: &str, value: i32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing set or create new one
            let current = state.get(key).cloned().unwrap_or_else(|| {
                // Create a new array to store unique values
                Dynamic::from(rhai::Array::new())
            });

            if let Ok(mut arr) = current.into_array() {
                let value_dynamic = Dynamic::from(value);
                // Check if value already exists in array
                if !arr
                    .iter()
                    .any(|v| v.as_int().unwrap_or(i64::MIN) == (value as i64))
                {
                    arr.push(value_dynamic);
                }
                state.insert(key.to_string(), Dynamic::from(arr));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("unique"));
            }
        });
    });

    engine.register_fn("track_unique", |key: &str, value: f64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing set or create new one
            let current = state.get(key).cloned().unwrap_or_else(|| {
                // Create a new array to store unique values
                Dynamic::from(rhai::Array::new())
            });

            if let Ok(mut arr) = current.into_array() {
                let value_dynamic = Dynamic::from(value);
                // Check if value already exists in array
                if !arr
                    .iter()
                    .any(|v| v.as_float().unwrap_or(f64::NAN) == value)
                {
                    arr.push(value_dynamic);
                }
                state.insert(key.to_string(), Dynamic::from(arr));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("unique"));
            }
        });
    });

    engine.register_fn("track_unique", |key: &str, value: f32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing set or create new one
            let current = state.get(key).cloned().unwrap_or_else(|| {
                // Create a new array to store unique values
                Dynamic::from(rhai::Array::new())
            });

            if let Ok(mut arr) = current.into_array() {
                let value_dynamic = Dynamic::from(value);
                // Check if value already exists in array
                if !arr
                    .iter()
                    .any(|v| v.as_float().unwrap_or(f64::NAN) == (value as f64))
                {
                    arr.push(value_dynamic);
                }
                state.insert(key.to_string(), Dynamic::from(arr));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("unique"));
            }
        });
    });

    engine.register_fn("track_bucket", |key: &str, bucket: &str| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing map or create new one
            let current = state
                .get(key)
                .cloned()
                .unwrap_or_else(|| Dynamic::from(rhai::Map::new()));

            if let Some(mut map) = current.try_cast::<rhai::Map>() {
                let count = map.get(bucket).cloned().unwrap_or(Dynamic::from(0i64));
                let new_count = count.as_int().unwrap_or(0) + 1;
                map.insert(bucket.into(), Dynamic::from(new_count));
                state.insert(key.to_string(), Dynamic::from(map));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("bucket"));
            }
        });
    });

    engine.register_fn("track_bucket", |key: &str, bucket: i64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing map or create new one
            let current = state
                .get(key)
                .cloned()
                .unwrap_or_else(|| Dynamic::from(rhai::Map::new()));

            if let Some(mut map) = current.try_cast::<rhai::Map>() {
                let bucket_str = bucket.to_string();
                let count = map
                    .get(bucket_str.as_str())
                    .cloned()
                    .unwrap_or(Dynamic::from(0i64));
                let new_count = count.as_int().unwrap_or(0) + 1;
                map.insert(bucket_str.into(), Dynamic::from(new_count));
                state.insert(key.to_string(), Dynamic::from(map));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("bucket"));
            }
        });
    });

    engine.register_fn("track_bucket", |key: &str, bucket: i32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing map or create new one
            let current = state
                .get(key)
                .cloned()
                .unwrap_or_else(|| Dynamic::from(rhai::Map::new()));

            if let Some(mut map) = current.try_cast::<rhai::Map>() {
                let bucket_str = bucket.to_string();
                let count = map
                    .get(bucket_str.as_str())
                    .cloned()
                    .unwrap_or(Dynamic::from(0i64));
                let new_count = count.as_int().unwrap_or(0) + 1;
                map.insert(bucket_str.into(), Dynamic::from(new_count));
                state.insert(key.to_string(), Dynamic::from(map));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("bucket"));
            }
        });
    });

    engine.register_fn("track_bucket", |key: &str, bucket: f64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing map or create new one
            let current = state
                .get(key)
                .cloned()
                .unwrap_or_else(|| Dynamic::from(rhai::Map::new()));

            if let Some(mut map) = current.try_cast::<rhai::Map>() {
                let bucket_str = bucket.to_string();
                let count = map
                    .get(bucket_str.as_str())
                    .cloned()
                    .unwrap_or(Dynamic::from(0i64));
                let new_count = count.as_int().unwrap_or(0) + 1;
                map.insert(bucket_str.into(), Dynamic::from(new_count));
                state.insert(key.to_string(), Dynamic::from(map));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("bucket"));
            }
        });
    });

    engine.register_fn("track_bucket", |key: &str, bucket: f32| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            // Get existing map or create new one
            let current = state
                .get(key)
                .cloned()
                .unwrap_or_else(|| Dynamic::from(rhai::Map::new()));

            if let Some(mut map) = current.try_cast::<rhai::Map>() {
                let bucket_str = bucket.to_string();
                let count = map
                    .get(bucket_str.as_str())
                    .cloned()
                    .unwrap_or(Dynamic::from(0i64));
                let new_count = count.as_int().unwrap_or(0) + 1;
                map.insert(bucket_str.into(), Dynamic::from(new_count));
                state.insert(key.to_string(), Dynamic::from(map));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("bucket"));
            }
        });
    });
}

// Expose the thread-local state management functions for engine.rs
pub fn set_thread_tracking_state(tracked: &HashMap<String, Dynamic>) {
    THREAD_TRACKING_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.clear();
        for (k, v) in tracked {
            state.insert(k.clone(), v.clone());
        }
    });
}

pub fn get_thread_tracking_state() -> HashMap<String, Dynamic> {
    THREAD_TRACKING_STATE.with(|state| state.borrow().clone())
}

/// Merge thread-local tracking state into context tracker for sequential mode
pub fn merge_thread_tracking_to_context(ctx: &mut crate::pipeline::PipelineContext) {
    let thread_state = get_thread_tracking_state();
    for (key, value) in thread_state {
        ctx.tracker.insert(key, value);
    }
}

/// Create a dynamic map that gives access to current metrics state
fn get_metrics_map() -> Dynamic {
    Dynamic::from(rhai::Map::new()) // Will be populated by accessing current state
}

/// Format metrics for CLI output according to specification
pub fn format_metrics_output(tracked: &HashMap<String, Dynamic>) -> String {
    let mut output = String::new();
    
    // Filter out internal keys (operation metadata and stats)
    let mut user_values: Vec<_> = tracked
        .iter()
        .filter(|(k, _)| !k.starts_with("__op_") && !k.starts_with("__kelora_stats_"))
        .collect();

    if user_values.is_empty() {
        return "No metrics tracked".to_string();
    }

    // Sort by key for consistent output
    user_values.sort_by_key(|(k, _)| k.as_str());

    for (key, value) in user_values {
        if value.is::<rhai::Map>() {
            // Handle unique tracking format: { count: N, sample: [...] }
            if let Some(map) = value.clone().try_cast::<rhai::Map>() {
                if let (Some(count), Some(sample)) = (map.get("count"), map.get("sample")) {
                    if let Ok(sample_array) = sample.clone().into_array() {
                        let sample_strings: Vec<String> = sample_array
                            .iter()
                            .take(3) // Only show first 3 in output
                            .map(|v| format!("\"{}\"", v.clone().into_string().unwrap_or_default()))
                            .collect();
                        output.push_str(&format!(
                            "{:<12} = {{ count: {}, sample: [{}] }}\n",
                            key,
                            count.as_int().unwrap_or(0),
                            sample_strings.join(", ")
                        ));
                        continue;
                    }
                }
            }
        }
        
        // Handle regular values (count, max, etc.)
        if value.is_int() {
            output.push_str(&format!("{:<12} = {}\n", key, value.as_int().unwrap_or(0)));
        } else if value.is_float() {
            output.push_str(&format!("{:<12} = {}\n", key, value.as_float().unwrap_or(0.0)));
        } else {
            output.push_str(&format!("{:<12} = {}\n", key, value.to_string()));
        }
    }

    output.trim_end().to_string()
}

/// Format metrics for JSON output
pub fn format_metrics_json(tracked: &HashMap<String, Dynamic>) -> Result<String, serde_json::Error> {
    let mut json_obj = serde_json::Map::new();
    
    // Filter out internal keys
    for (key, value) in tracked.iter() {
        if key.starts_with("__op_") || key.starts_with("__kelora_stats_") || key.starts_with("__kelora_error_") {
            continue;
        }
        
        if value.is::<rhai::Map>() {
            if let Some(map) = value.clone().try_cast::<rhai::Map>() {
                if let (Some(count), Some(sample)) = (map.get("count"), map.get("sample")) {
                    // Handle unique tracking format
                    let mut unique_obj = serde_json::Map::new();
                    unique_obj.insert("count".to_string(), serde_json::Value::Number(
                        serde_json::Number::from(count.as_int().unwrap_or(0))
                    ));
                    
                    if let Ok(sample_array) = sample.clone().into_array() {
                        let sample_values: Vec<serde_json::Value> = sample_array
                            .iter()
                            .map(|v| serde_json::Value::String(v.clone().into_string().unwrap_or_default()))
                            .collect();
                        unique_obj.insert("sample".to_string(), serde_json::Value::Array(sample_values));
                    }
                    
                    json_obj.insert(key.clone(), serde_json::Value::Object(unique_obj));
                    continue;
                }
            }
        }
        
        // Handle regular values
        if value.is_int() {
            json_obj.insert(key.clone(), serde_json::Value::Number(
                serde_json::Number::from(value.as_int().unwrap_or(0))
            ));
        } else if value.is_float() {
            if let Some(num) = serde_json::Number::from_f64(value.as_float().unwrap_or(0.0)) {
                json_obj.insert(key.clone(), serde_json::Value::Number(num));
            }
        } else {
            json_obj.insert(key.clone(), serde_json::Value::String(value.to_string()));
        }
    }
    
    serde_json::to_string_pretty(&json_obj)
}

/// Extract error summary from tracking state
pub fn extract_error_summary(tracked: &HashMap<String, Dynamic>) -> Option<String> {
    let mut has_errors = false;
    let mut summary = serde_json::Map::new();
    
    // Collect error types and their counts
    let mut error_types: std::collections::HashSet<String> = std::collections::HashSet::new();
    for key in tracked.keys() {
        if let Some(suffix) = key.strip_prefix("__kelora_error_count_") {
            error_types.insert(suffix.to_string());
        }
    }
    
    for error_type in error_types {
        let count_key = format!("__kelora_error_count_{}", error_type);
        let examples_key = format!("__kelora_error_examples_{}", error_type);
        
        if let Some(count_value) = tracked.get(&count_key) {
            let count = count_value.as_int().unwrap_or(0);
            if count > 0 {
                has_errors = true;
                let mut error_obj = serde_json::Map::new();
                error_obj.insert("count".to_string(), serde_json::Value::Number(
                    serde_json::Number::from(count)
                ));
                
                // Add examples if available
                if let Some(examples_value) = tracked.get(&examples_key) {
                    if let Ok(examples_array) = examples_value.clone().into_array() {
                        let examples: Vec<serde_json::Value> = examples_array
                            .iter()
                            .map(|v| serde_json::Value::String(v.clone().into_string().unwrap_or_default()))
                            .collect();
                        error_obj.insert("examples".to_string(), serde_json::Value::Array(examples));
                    }
                }
                
                summary.insert(error_type, serde_json::Value::Object(error_obj));
            }
        }
    }
    
    if has_errors {
        Some(serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "Error serializing summary".to_string()))
    } else {
        None
    }
}

/// Write error summary to file if configured
pub fn write_error_summary_to_file(
    tracked: &HashMap<String, Dynamic>,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(summary) = extract_error_summary(tracked) {
        use std::fs::File;
        use std::io::Write;
        let mut file = File::create(file_path)?;
        file.write_all(summary.as_bytes())?;
    }
    Ok(())
}
