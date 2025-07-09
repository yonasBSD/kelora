use rhai::{Dynamic, Engine};
use std::cell::RefCell;
use std::collections::HashMap;

// Thread-local storage for tracking state
thread_local! {
    static THREAD_TRACKING_STATE: RefCell<HashMap<String, Dynamic>> = RefCell::new(HashMap::new());
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
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(f64::INFINITY));
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
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(f64::INFINITY));
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
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(f64::INFINITY));
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
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(f64::INFINITY));
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
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(f64::NEG_INFINITY));
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
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(f64::NEG_INFINITY));
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
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(f64::NEG_INFINITY));
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
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(f64::NEG_INFINITY));
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
                if !arr
                    .iter()
                    .any(|v| v.as_int().unwrap_or(i64::MIN) == value)
                {
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
                let count = map.get(bucket_str.as_str()).cloned().unwrap_or(Dynamic::from(0i64));
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
                let count = map.get(bucket_str.as_str()).cloned().unwrap_or(Dynamic::from(0i64));
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
                let count = map.get(bucket_str.as_str()).cloned().unwrap_or(Dynamic::from(0i64));
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
                let count = map.get(bucket_str.as_str()).cloned().unwrap_or(Dynamic::from(0i64));
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
