use rhai::{Engine, Dynamic};
use std::collections::HashMap;
use std::cell::RefCell;

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

    engine.register_fn("track_min", |key: &str, value: i64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(i64::MAX));
            let current_val = current.as_int().unwrap_or(i64::MAX);
            if value < current_val {
                state.insert(key.to_string(), Dynamic::from(value));
                // Store operation type metadata for parallel merging
                state.insert(format!("__op_{}", key), Dynamic::from("min"));
            }
        });
    });

    engine.register_fn("track_max", |key: &str, value: i64| {
        THREAD_TRACKING_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let current = state.get(key).cloned().unwrap_or(Dynamic::from(i64::MIN));
            let current_val = current.as_int().unwrap_or(i64::MIN);
            if value > current_val {
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
                if !arr.iter().any(|v| v.clone().into_string().unwrap_or_default() == value) {
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
            let current = state.get(key).cloned().unwrap_or_else(|| {
                Dynamic::from(rhai::Map::new())
            });
            
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
    THREAD_TRACKING_STATE.with(|state| {
        state.borrow().clone()
    })
}

