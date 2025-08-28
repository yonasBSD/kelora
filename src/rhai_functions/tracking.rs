use rhai::{Dynamic, Engine};
use std::cell::RefCell;
use std::collections::HashMap;

// Thread-local storage for tracking state
thread_local! {
    pub static THREAD_TRACKING_STATE: RefCell<HashMap<String, Dynamic>> = RefCell::new(HashMap::new());
}

/// Unified error tracking function that handles counts, samples, and verbose output
/// This replaces both stats-based and tracking-based error mechanisms
/// Note: This function has 8 parameters because it needs to handle diverse error contexts:
/// location info (line_num, filename), content (message, original_line), error classification (error_type),
/// and output control (verbose, quiet, config). Grouping these would add complexity without benefit.
#[allow(clippy::too_many_arguments)]
pub fn track_error(
    error_type: &str,
    line_num: Option<usize>,
    message: &str,
    original_line: Option<&str>,
    filename: Option<&str>,
    verbose: u8,
    quiet: bool,
    config: Option<&crate::pipeline::PipelineConfig>,
) {
    // Use tracking infrastructure for error counting in all modes
    // This ensures consistent mapreduce behavior and avoids double counting

    THREAD_TRACKING_STATE.with(|state| {
        let mut state = state.borrow_mut();

        // Track error count by type - uses "count" operation for summing across workers
        let count_key = format!("__kelora_error_count_{}", error_type);
        let current_count = state
            .get(&count_key)
            .cloned()
            .unwrap_or(Dynamic::from(0i64));
        let new_count = current_count.as_int().unwrap_or(0) + 1;
        state.insert(count_key.clone(), Dynamic::from(new_count));
        state.insert(format!("__op_{}", count_key), Dynamic::from("count"));

        // Output verbose errors - use ordered capture system for proper interleaving
        if verbose > 0 && !quiet {
            // Enhanced format with filename for immediate verbose output
            let color_mode = config
                .map(|c| &c.color_mode)
                .unwrap_or(&crate::config::ColorMode::Auto);
            let use_colors = crate::tty::should_use_colors_with_mode(color_mode);
            let no_emoji = if let Some(cfg) = config {
                cfg.no_emoji || std::env::var("NO_EMOJI").is_ok()
            } else {
                std::env::var("NO_EMOJI").is_ok()
            };
            let use_emoji = use_colors && !no_emoji;
            let prefix = if use_emoji { "🧱" } else { "kelora:" };

            let formatted_error = if let (Some(line), Some(fname)) = (line_num, filename) {
                format!(
                    "{} {}:{}: {} - {}",
                    prefix, fname, line, error_type, message
                )
            } else if let Some(line) = line_num {
                format!("{} line {}: {} - {}", prefix, line, error_type, message)
            } else {
                format!("{} {} - {}", prefix, error_type, message)
            };

            if crate::rhai_functions::strings::is_parallel_mode() {
                // In parallel mode, capture stderr message for ordered output later
                crate::rhai_functions::strings::capture_stderr(formatted_error);
            } else {
                // In sequential mode, output immediately but also capture for consistency
                crate::rhai_functions::strings::capture_stderr(formatted_error.clone());
                eprintln!("{}", formatted_error);
            }
        }

        // Track error samples (max 3 per type) - uses "unique" operation for deduplication
        // This stores examples for display but doesn't affect the total count
        let samples_key = format!("__kelora_error_samples_{}", error_type);
        let current_samples = state
            .get(&samples_key)
            .cloned()
            .unwrap_or_else(|| Dynamic::from(rhai::Array::new()));

        if let Ok(mut arr) = current_samples.into_array() {
            // Only store up to 3 samples per error type
            if arr.len() < 3 {
                // Create a sample object containing error details and original line
                let mut sample_obj = rhai::Map::new();
                sample_obj.insert("error_type".into(), Dynamic::from(error_type.to_string()));
                sample_obj.insert(
                    "line_num".into(),
                    Dynamic::from(line_num.unwrap_or(0) as i64),
                );
                sample_obj.insert("message".into(), Dynamic::from(message.to_string()));
                if let Some(line) = original_line {
                    sample_obj.insert("original_line".into(), Dynamic::from(line.to_string()));
                }
                // Store filename if available
                if let Some(filename) = filename {
                    sample_obj.insert("filename".into(), Dynamic::from(filename.to_string()));
                }

                arr.push(Dynamic::from(sample_obj));
            }

            state.insert(samples_key.clone(), Dynamic::from(arr));
            state.insert(format!("__op_{}", samples_key), Dynamic::from("unique"));
        }
    });
}

/// Check if any errors occurred based on tracking data
#[allow(dead_code)] // Used by main.rs binary target, not detected by clippy in lib context
pub fn has_errors_in_tracking(tracked: &HashMap<String, Dynamic>) -> bool {
    for (key, value) in tracked {
        if let Some(_error_type) = key.strip_prefix("__kelora_error_count_") {
            if let Ok(count) = value.as_int() {
                if count > 0 {
                    return true;
                }
            }
        }
    }
    false
}

/// Extract error summary from tracking state with different verbosity levels
#[allow(dead_code)] // Used by main.rs binary target, not detected by clippy in lib context
pub fn extract_error_summary_from_tracking(
    tracked: &HashMap<String, Dynamic>,
    verbose: u8,
) -> Option<String> {
    let mut total_errors = 0;
    let mut error_types = Vec::new();
    let mut sample_objects: Vec<rhai::Map> = Vec::new();

    // Collect error counts by type
    for (key, value) in tracked {
        if let Some(error_type) = key.strip_prefix("__kelora_error_count_") {
            if let Ok(count) = value.as_int() {
                if count > 0 {
                    total_errors += count;
                    error_types.push((error_type.to_string(), count));
                }
            }
        }
    }

    if total_errors == 0 {
        return None;
    }

    // Collect sample objects with structured data
    for (key, value) in tracked {
        if let Some(_error_type) = key.strip_prefix("__kelora_error_samples_") {
            if let Ok(sample_array) = value.clone().into_array() {
                for sample in sample_array {
                    if let Some(sample_map) = sample.try_cast::<rhai::Map>() {
                        sample_objects.push(sample_map);
                    }
                }
            }
        }
    }

    // Use the new concise format
    let mut summary = String::new();

    // Determine primary error type for header
    let primary_error_type = if error_types.len() == 1 {
        &error_types[0].0
    } else {
        // For mixed types, use generic "errors"
        "mixed"
    };

    // Format header: "🧱 parse errors: N total" or "🧱 mixed errors: N total"
    if primary_error_type == "mixed" {
        summary.push_str(&format!("🧱 mixed errors: {} total", total_errors));
    } else {
        summary.push_str(&format!(
            "🧱 {} errors: {} total",
            primary_error_type, total_errors
        ));
    }

    // Show up to 3 examples with filename:line format
    let mut shown_samples = 0;
    for sample_obj in &sample_objects {
        if shown_samples >= 3 {
            break;
        }

        // Extract data from sample object
        let line_num = sample_obj
            .get("line_num")
            .and_then(|v| v.as_int().ok())
            .unwrap_or(0);
        let message = sample_obj
            .get("message")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "unknown error".to_string());
        let filename = sample_obj
            .get("filename")
            .and_then(|v| v.clone().into_string().ok())
            .unwrap_or_else(|| "stdin".to_string());
        let original_line = sample_obj
            .get("original_line")
            .and_then(|v| v.clone().into_string().ok());

        // Format: "  filename:line: error message"
        summary.push_str(&format!("\n  {}:{}: {}", filename, line_num, message));

        // In verbose mode, add indented original line
        if verbose > 0 {
            if let Some(orig_line) = original_line {
                // Truncate very long lines for readability
                let display_line = if orig_line.len() > 100 {
                    format!("{}...", &orig_line[..97])
                } else {
                    orig_line
                };
                summary.push_str(&format!("\n    {}", display_line));
            }
        }

        shown_samples += 1;
    }

    // Add "more errors not shown" if total errors exceed samples shown
    if total_errors as usize > shown_samples {
        let remaining = total_errors as usize - shown_samples;
        summary.push_str(&format!("\n  [{} more errors not shown]", remaining));
    }

    Some(summary)
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
#[allow(dead_code)] // Planned feature for parallel mode metrics merging
pub fn merge_thread_tracking_to_context(ctx: &mut crate::pipeline::PipelineContext) {
    let thread_state = get_thread_tracking_state();
    for (key, value) in thread_state {
        ctx.tracker.insert(key, value);
    }
}

/// Create a dynamic map that gives access to current metrics state
#[allow(dead_code)] // Planned feature for exposing metrics to Rhai scripts
fn get_metrics_map() -> Dynamic {
    Dynamic::from(rhai::Map::new()) // Will be populated by accessing current state
}

/// Format metrics for CLI output according to specification
#[allow(dead_code)] // Used by main.rs binary target, not detected by clippy in lib context
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
            output.push_str(&format!(
                "{:<12} = {}\n",
                key,
                value.as_float().unwrap_or(0.0)
            ));
        } else {
            output.push_str(&format!("{:<12} = {}\n", key, value));
        }
    }

    output.trim_end().to_string()
}

/// Format metrics for JSON output
#[allow(dead_code)] // Used by main.rs binary target, not detected by clippy in lib context
pub fn format_metrics_json(
    tracked: &HashMap<String, Dynamic>,
) -> Result<String, serde_json::Error> {
    let mut json_obj = serde_json::Map::new();

    // Filter out internal keys
    for (key, value) in tracked.iter() {
        if key.starts_with("__op_")
            || key.starts_with("__kelora_stats_")
            || key.starts_with("__kelora_error_")
        {
            continue;
        }

        if value.is::<rhai::Map>() {
            if let Some(map) = value.clone().try_cast::<rhai::Map>() {
                if let (Some(count), Some(sample)) = (map.get("count"), map.get("sample")) {
                    // Handle unique tracking format
                    let mut unique_obj = serde_json::Map::new();
                    unique_obj.insert(
                        "count".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(
                            count.as_int().unwrap_or(0),
                        )),
                    );

                    if let Ok(sample_array) = sample.clone().into_array() {
                        let sample_values: Vec<serde_json::Value> = sample_array
                            .iter()
                            .map(|v| {
                                serde_json::Value::String(
                                    v.clone().into_string().unwrap_or_default(),
                                )
                            })
                            .collect();
                        unique_obj.insert(
                            "sample".to_string(),
                            serde_json::Value::Array(sample_values),
                        );
                    }

                    json_obj.insert(key.clone(), serde_json::Value::Object(unique_obj));
                    continue;
                }
            }
        }

        // Handle regular values
        if value.is_int() {
            json_obj.insert(
                key.clone(),
                serde_json::Value::Number(serde_json::Number::from(value.as_int().unwrap_or(0))),
            );
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
#[allow(dead_code)] // Planned feature for error reporting
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
                error_obj.insert(
                    "count".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(count)),
                );

                // Add examples if available
                if let Some(examples_value) = tracked.get(&examples_key) {
                    if let Ok(examples_array) = examples_value.clone().into_array() {
                        let examples: Vec<serde_json::Value> = examples_array
                            .iter()
                            .map(|v| {
                                serde_json::Value::String(
                                    v.clone().into_string().unwrap_or_default(),
                                )
                            })
                            .collect();
                        error_obj
                            .insert("examples".to_string(), serde_json::Value::Array(examples));
                    }
                }

                summary.insert(error_type, serde_json::Value::Object(error_obj));
            }
        }
    }

    if has_errors {
        Some(
            serde_json::to_string_pretty(&summary)
                .unwrap_or_else(|_| "Error serializing summary".to_string()),
        )
    } else {
        None
    }
}

/// Write error summary to file if configured
#[allow(dead_code)] // Planned feature for error reporting
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
