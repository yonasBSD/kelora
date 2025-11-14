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

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::Dynamic;
    use std::fs;
    use std::io::Write;

    // Helper to clear thread-local state between tests
    fn clear_conf_state() {
        INIT_MAP.with(|m| {
            *m.write().unwrap() = None;
        });
        IS_BEGIN_PHASE.with(|b| {
            *b.write().unwrap() = false;
        });
    }

    #[test]
    fn test_set_begin_phase_true() {
        clear_conf_state();

        set_begin_phase(true);
        assert!(is_begin_phase());

        clear_conf_state();
    }

    #[test]
    fn test_set_begin_phase_false() {
        clear_conf_state();

        set_begin_phase(false);
        assert!(!is_begin_phase());

        clear_conf_state();
    }

    #[test]
    fn test_begin_phase_transitions() {
        clear_conf_state();

        // Start false
        assert!(!is_begin_phase());

        // Set to true
        set_begin_phase(true);
        assert!(is_begin_phase());

        // Set back to false
        set_begin_phase(false);
        assert!(!is_begin_phase());

        // Set to true again
        set_begin_phase(true);
        assert!(is_begin_phase());

        clear_conf_state();
    }

    #[test]
    fn test_set_init_map_basic() {
        clear_conf_state();

        let mut map = rhai::Map::new();
        map.insert("key1".into(), Dynamic::from(42i64));
        map.insert("key2".into(), Dynamic::from("value"));

        set_init_map(map.clone());

        // Verify map was set
        INIT_MAP.with(|m| {
            let stored_map = m.read().unwrap();
            assert!(stored_map.is_some());
            let stored = stored_map.as_ref().unwrap();
            assert_eq!(stored.len(), 2);
            assert_eq!(stored.get("key1").unwrap().as_int().unwrap(), 42);
            assert_eq!(
                stored.get("key2").unwrap().clone().into_string().unwrap(),
                "value"
            );
        });

        clear_conf_state();
    }

    #[test]
    fn test_set_init_map_empty() {
        clear_conf_state();

        let map = rhai::Map::new();
        set_init_map(map);

        INIT_MAP.with(|m| {
            let stored_map = m.read().unwrap();
            assert!(stored_map.is_some());
            assert_eq!(stored_map.as_ref().unwrap().len(), 0);
        });

        clear_conf_state();
    }

    #[test]
    fn test_set_init_map_overwrite() {
        clear_conf_state();

        // Set first map
        let mut map1 = rhai::Map::new();
        map1.insert("old_key".into(), Dynamic::from(1i64));
        set_init_map(map1);

        // Set second map (should overwrite)
        let mut map2 = rhai::Map::new();
        map2.insert("new_key".into(), Dynamic::from(2i64));
        set_init_map(map2);

        INIT_MAP.with(|m| {
            let stored_map = m.read().unwrap();
            let stored = stored_map.as_ref().unwrap();
            assert!(!stored.contains_key("old_key"));
            assert!(stored.contains_key("new_key"));
            assert_eq!(stored.get("new_key").unwrap().as_int().unwrap(), 2);
        });

        clear_conf_state();
    }

    #[test]
    fn test_deep_freeze_map_basic() {
        clear_conf_state();

        let mut map = rhai::Map::new();
        map.insert("frozen_key".into(), Dynamic::from(999i64));

        deep_freeze_map(&mut map);

        // Verify map was stored via set_init_map
        INIT_MAP.with(|m| {
            let stored_map = m.read().unwrap();
            assert!(stored_map.is_some());
            let stored = stored_map.as_ref().unwrap();
            assert_eq!(stored.get("frozen_key").unwrap().as_int().unwrap(), 999);
        });

        clear_conf_state();
    }

    #[test]
    fn test_deep_freeze_map_with_nested_values() {
        clear_conf_state();

        let mut inner_map = rhai::Map::new();
        inner_map.insert("nested".into(), Dynamic::from("deep"));

        let mut map = rhai::Map::new();
        map.insert("outer".into(), Dynamic::from(100i64));
        map.insert("inner".into(), Dynamic::from(inner_map));

        deep_freeze_map(&mut map);

        INIT_MAP.with(|m| {
            let stored_map = m.read().unwrap();
            assert!(stored_map.is_some());
            let stored = stored_map.as_ref().unwrap();
            assert_eq!(stored.get("outer").unwrap().as_int().unwrap(), 100);
            assert!(stored.contains_key("inner"));
        });

        clear_conf_state();
    }

    #[test]
    fn test_read_file_impl_not_in_begin_phase() {
        clear_conf_state();

        // Ensure we're not in begin phase
        set_begin_phase(false);

        let result = read_file_impl("/tmp/test.txt".to_string());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("can only be called during --begin phase"));

        clear_conf_state();
    }

    #[test]
    fn test_read_file_impl_in_begin_phase_file_not_found() {
        clear_conf_state();

        set_begin_phase(true);

        let result = read_file_impl("/nonexistent/file/path.txt".to_string());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read file"));

        clear_conf_state();
    }

    #[test]
    fn test_read_file_impl_success() {
        clear_conf_state();

        // Create a temporary file
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("kelora_test_read_file.txt");
        let test_content = "Hello, World!\nLine 2\nLine 3";

        fs::write(&temp_file, test_content).unwrap();

        set_begin_phase(true);

        let result = read_file_impl(temp_file.to_string_lossy().to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_content);

        // Cleanup
        fs::remove_file(&temp_file).unwrap();
        clear_conf_state();
    }

    #[test]
    fn test_read_file_impl_with_bom() {
        clear_conf_state();

        // Create a temporary file with BOM
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("kelora_test_read_file_bom.txt");
        let content_with_bom = "\u{feff}Content after BOM";

        fs::write(&temp_file, content_with_bom).unwrap();

        set_begin_phase(true);

        let result = read_file_impl(temp_file.to_string_lossy().to_string());
        assert!(result.is_ok());
        // BOM should be stripped
        assert_eq!(result.unwrap(), "Content after BOM");

        // Cleanup
        fs::remove_file(&temp_file).unwrap();
        clear_conf_state();
    }

    #[test]
    fn test_read_file_impl_empty_file() {
        clear_conf_state();

        // Create an empty temporary file
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("kelora_test_read_file_empty.txt");

        fs::write(&temp_file, "").unwrap();

        set_begin_phase(true);

        let result = read_file_impl(temp_file.to_string_lossy().to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");

        // Cleanup
        fs::remove_file(&temp_file).unwrap();
        clear_conf_state();
    }

    #[test]
    fn test_read_lines_impl_not_in_begin_phase() {
        clear_conf_state();

        set_begin_phase(false);

        let result = read_lines_impl("/tmp/test.txt".to_string());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("can only be called during --begin phase"));

        clear_conf_state();
    }

    #[test]
    fn test_read_lines_impl_file_not_found() {
        clear_conf_state();

        set_begin_phase(true);

        let result = read_lines_impl("/nonexistent/file/path.txt".to_string());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read file"));

        clear_conf_state();
    }

    #[test]
    fn test_read_lines_impl_success() {
        clear_conf_state();

        // Create a temporary file with multiple lines
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("kelora_test_read_lines.txt");
        let test_content = "Line 1\nLine 2\nLine 3";

        fs::write(&temp_file, test_content).unwrap();

        set_begin_phase(true);

        let result = read_lines_impl(temp_file.to_string_lossy().to_string());
        assert!(result.is_ok());

        let lines = result.unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].clone().into_string().unwrap(), "Line 1");
        assert_eq!(lines[1].clone().into_string().unwrap(), "Line 2");
        assert_eq!(lines[2].clone().into_string().unwrap(), "Line 3");

        // Cleanup
        fs::remove_file(&temp_file).unwrap();
        clear_conf_state();
    }

    #[test]
    fn test_read_lines_impl_with_bom() {
        clear_conf_state();

        // Create a temporary file with BOM
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("kelora_test_read_lines_bom.txt");
        let content_with_bom = "\u{feff}Line 1\nLine 2";

        fs::write(&temp_file, content_with_bom).unwrap();

        set_begin_phase(true);

        let result = read_lines_impl(temp_file.to_string_lossy().to_string());
        assert!(result.is_ok());

        let lines = result.unwrap();
        assert_eq!(lines.len(), 2);
        // BOM should be stripped from first line
        assert_eq!(lines[0].clone().into_string().unwrap(), "Line 1");
        assert_eq!(lines[1].clone().into_string().unwrap(), "Line 2");

        // Cleanup
        fs::remove_file(&temp_file).unwrap();
        clear_conf_state();
    }

    #[test]
    fn test_read_lines_impl_empty_file() {
        clear_conf_state();

        // Create an empty temporary file
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("kelora_test_read_lines_empty.txt");

        fs::write(&temp_file, "").unwrap();

        set_begin_phase(true);

        let result = read_lines_impl(temp_file.to_string_lossy().to_string());
        assert!(result.is_ok());

        let lines = result.unwrap();
        // Empty file produces empty array (no lines)
        assert_eq!(lines.len(), 0);

        // Cleanup
        fs::remove_file(&temp_file).unwrap();
        clear_conf_state();
    }

    #[test]
    fn test_read_lines_impl_single_line_no_newline() {
        clear_conf_state();

        // Create a file with single line and no trailing newline
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("kelora_test_read_lines_single.txt");

        let mut file = fs::File::create(&temp_file).unwrap();
        file.write_all(b"Single line").unwrap();
        // Explicitly don't write a newline

        set_begin_phase(true);

        let result = read_lines_impl(temp_file.to_string_lossy().to_string());
        assert!(result.is_ok());

        let lines = result.unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].clone().into_string().unwrap(), "Single line");

        // Cleanup
        fs::remove_file(&temp_file).unwrap();
        clear_conf_state();
    }

    #[test]
    fn test_read_lines_impl_trailing_newline() {
        clear_conf_state();

        // Create a file with trailing newline
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("kelora_test_read_lines_trailing.txt");
        let test_content = "Line 1\nLine 2\n";

        fs::write(&temp_file, test_content).unwrap();

        set_begin_phase(true);

        let result = read_lines_impl(temp_file.to_string_lossy().to_string());
        assert!(result.is_ok());

        let lines = result.unwrap();
        // Trailing newline should not create an extra empty line
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].clone().into_string().unwrap(), "Line 1");
        assert_eq!(lines[1].clone().into_string().unwrap(), "Line 2");

        // Cleanup
        fs::remove_file(&temp_file).unwrap();
        clear_conf_state();
    }

    #[test]
    fn test_thread_local_isolation() {
        clear_conf_state();

        // Set a map and begin phase
        let mut map = rhai::Map::new();
        map.insert("isolated".into(), Dynamic::from(123i64));
        set_init_map(map);
        set_begin_phase(true);

        // Verify state is set
        assert!(is_begin_phase());
        INIT_MAP.with(|m| {
            assert!(m.read().unwrap().is_some());
        });

        // Clear and verify
        clear_conf_state();
        assert!(!is_begin_phase());
        INIT_MAP.with(|m| {
            assert!(m.read().unwrap().is_none());
        });
    }

    #[test]
    fn test_init_map_with_various_types() {
        clear_conf_state();

        let mut map = rhai::Map::new();
        map.insert("int".into(), Dynamic::from(42i64));
        map.insert("float".into(), Dynamic::from(2.5f64));
        map.insert("string".into(), Dynamic::from("test"));
        map.insert("bool".into(), Dynamic::from(true));

        let arr = vec![Dynamic::from(1i64), Dynamic::from(2i64)];
        map.insert("array".into(), Dynamic::from(arr));

        set_init_map(map.clone());

        INIT_MAP.with(|m| {
            let stored_map = m.read().unwrap();
            let stored = stored_map.as_ref().unwrap();
            assert_eq!(stored.get("int").unwrap().as_int().unwrap(), 42);
            assert_eq!(
                stored.get("string").unwrap().clone().into_string().unwrap(),
                "test"
            );
            assert!(stored.get("bool").unwrap().as_bool().unwrap());
        });

        clear_conf_state();
    }
}
