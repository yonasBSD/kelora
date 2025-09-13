use rhai::{Dynamic, Engine};
use std::fs;
use std::sync::RwLock;

thread_local! {
    static INIT_MAP: RwLock<Option<rhai::Map>> = const { RwLock::new(None) };
    static IS_BEGIN_PHASE: RwLock<bool> = const { RwLock::new(false) };
}

/// Set the conf map for the current thread
pub fn set_init_map(map: rhai::Map) {
    INIT_MAP.with(|m| {
        *m.write().unwrap() = Some(map);
    });
}

/// Set whether we're currently in the begin phase
pub fn set_begin_phase(is_begin: bool) {
    IS_BEGIN_PHASE.with(|b| {
        *b.write().unwrap() = is_begin;
    });
}

/// Check if we're currently in the begin phase
pub fn is_begin_phase() -> bool {
    IS_BEGIN_PHASE.with(|b| *b.read().unwrap())
}

/// Read a file as a string with UTF-8 validation and BOM handling
fn read_file_impl(path: String) -> Result<String, Box<rhai::EvalAltResult>> {
    if !is_begin_phase() {
        return Err("read_file() can only be called during --begin phase".into());
    }

    let content = fs::read_to_string(&path).map_err(|e| {
        Box::<rhai::EvalAltResult>::from(format!("Failed to read file '{}': {}", path, e))
    })?;

    // Strip UTF-8 BOM if present
    let content = if let Some(stripped) = content.strip_prefix('\u{feff}') {
        stripped
    } else {
        &content
    };

    Ok(content.to_string())
}

/// Read a file as lines with UTF-8 validation and newline stripping
fn read_lines_impl(path: String) -> Result<rhai::Array, Box<rhai::EvalAltResult>> {
    if !is_begin_phase() {
        return Err("read_lines() can only be called during --begin phase".into());
    }

    let content = fs::read_to_string(&path).map_err(|e| {
        Box::<rhai::EvalAltResult>::from(format!("Failed to read file '{}': {}", path, e))
    })?;

    // Strip UTF-8 BOM if present
    let content = if let Some(stripped) = content.strip_prefix('\u{feff}') {
        stripped
    } else {
        &content
    };

    // Split into lines and strip newlines
    let lines: rhai::Array = content
        .lines()
        .map(|line| Dynamic::from(line.to_string()))
        .collect();

    Ok(lines)
}

/// Register conf-related functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("read_file", read_file_impl);
    engine.register_fn("read_lines", read_lines_impl);
}

/// Deep freeze a Rhai map recursively
/// Note: In Rhai, we can't actually freeze data structures at runtime,
/// but we can implement immutability through access control and error checking
pub fn deep_freeze_map(map: &mut rhai::Map) {
    // Store the frozen conf map in thread-local storage for access control
    set_init_map(map.clone());

    // Recursively process nested structures for consistency
    for (_, value) in map.iter_mut() {
        deep_freeze_dynamic(value);
    }
}

/// Deep freeze a Dynamic value recursively
/// Note: This is a placeholder implementation since Rhai doesn't support true immutability
fn deep_freeze_dynamic(_value: &mut Dynamic) {
    // In a full implementation, we would recursively process nested structures
    // For now, this is a placeholder since the actual immutability is enforced
    // through access control in the engine
}
