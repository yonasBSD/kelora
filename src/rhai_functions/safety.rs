use rhai::{Dynamic, Engine, ImmutableString, Map};

/// Safe field access with default value
/// Usage: get_path(e, "user.role", "guest")
pub fn get_path(event: Map, path: ImmutableString, default: Dynamic) -> Dynamic {
    let path_str = path.as_str();
    let parts: Vec<&str> = path_str.split('.').collect();
    
    let mut current_map = event;
    
    for (i, part) in parts.iter().enumerate() {
        if let Some(value) = current_map.get(*part).cloned() {
            if i == parts.len() - 1 {
                // Last part - return the value
                return value;
            } else {
                // Intermediate part - must be a map to continue
                if let Some(nested_map) = value.read_lock::<Map>() {
                    current_map = nested_map.clone();
                } else {
                    return default;
                }
            }
        } else {
            return default;
        }
    }
    
    default
}

/// Check if a path exists in the event
/// Usage: has_path(e, "user.role")
pub fn has_path(event: Map, path: ImmutableString) -> bool {
    let path_str = path.as_str();
    let parts: Vec<&str> = path_str.split('.').collect();
    
    let mut current_map = event;
    
    for (i, part) in parts.iter().enumerate() {
        if let Some(value) = current_map.get(*part).cloned() {
            if i == parts.len() - 1 {
                // Last part - it exists
                return true;
            } else {
                // Intermediate part - must be a map to continue
                if let Some(nested_map) = value.read_lock::<Map>() {
                    current_map = nested_map.clone();
                } else {
                    return false;
                }
            }
        } else {
            return false;
        }
    }
    
    false
}

/// Safe equality check for a path
/// Usage: path_equals(e, "user.role", "admin")
pub fn path_equals(event: Map, path: ImmutableString, expected: Dynamic) -> bool {
    let path_str = path.as_str();
    let parts: Vec<&str> = path_str.split('.').collect();
    
    let mut current_map = event;
    
    for (i, part) in parts.iter().enumerate() {
        if let Some(value) = current_map.get(*part).cloned() {
            if i == parts.len() - 1 {
                // Last part - compare with expected
                return value.type_name() == expected.type_name() && value.to_string() == expected.to_string();
            } else {
                // Intermediate part - must be a map to continue
                if let Some(nested_map) = value.read_lock::<Map>() {
                    current_map = nested_map.clone();
                } else {
                    return false;
                }
            }
        } else {
            return false;
        }
    }
    
    false
}

/// Convert value to number with default fallback
/// Usage: to_number(e.amount, 0)
pub fn to_number(value: Dynamic, default: Dynamic) -> Dynamic {
    // Try to convert to i64 first
    if let Ok(num) = value.as_int() {
        return Dynamic::from(num);
    }
    
    // Try to convert to f64
    if let Ok(num) = value.as_float() {
        return Dynamic::from(num);
    }
    
    // Try to parse string as number
    if let Some(s) = value.read_lock::<ImmutableString>() {
        if let Ok(num) = s.parse::<i64>() {
            return Dynamic::from(num);
        }
        if let Ok(num) = s.parse::<f64>() {
            return Dynamic::from(num);
        }
    }
    
    // Return default if conversion failed
    default
}

/// Convert value to boolean with default fallback
/// Usage: to_bool(e.active, false)
pub fn to_bool(value: Dynamic, default: Dynamic) -> Dynamic {
    // Already a boolean
    if let Ok(b) = value.as_bool() {
        return Dynamic::from(b);
    }
    
    // String conversion
    if let Some(s) = value.read_lock::<ImmutableString>() {
        let s_lower = s.to_lowercase();
        match s_lower.as_str() {
            "true" | "yes" | "1" | "on" => return Dynamic::from(true),
            "false" | "no" | "0" | "off" => return Dynamic::from(false),
            _ => {}
        }
    }
    
    // Number conversion (0 = false, non-zero = true)
    if let Ok(num) = value.as_int() {
        return Dynamic::from(num != 0);
    }
    if let Ok(num) = value.as_float() {
        return Dynamic::from(num != 0.0);
    }
    
    // Return default if conversion failed
    default
}

/// Register safety functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("get_path", get_path);
    engine.register_fn("has_path", has_path);
    engine.register_fn("path_equals", path_equals);
    engine.register_fn("to_number", to_number);
    engine.register_fn("to_bool", to_bool);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::Map;

    fn create_test_event() -> Map {
        let mut event = Map::new();
        event.insert("name".into(), Dynamic::from("alice"));
        event.insert("score".into(), Dynamic::from(85));
        
        let mut user = Map::new();
        user.insert("role".into(), Dynamic::from("admin"));
        user.insert("active".into(), Dynamic::from(true));
        event.insert("user".into(), Dynamic::from(user));
        
        event
    }

    #[test]
    fn test_get_path_existing() {
        let event = create_test_event();
        let result = get_path(event.clone(), "name".into(), Dynamic::from("default"));
        assert_eq!(result.as_string().unwrap(), "alice");
        
        let result = get_path(event, "user.role".into(), Dynamic::from("guest"));
        assert_eq!(result.as_string().unwrap(), "admin");
    }

    #[test]
    fn test_get_path_missing() {
        let event = create_test_event();
        let result = get_path(event.clone(), "missing".into(), Dynamic::from("default"));
        assert_eq!(result.as_string().unwrap(), "default");
        
        let result = get_path(event, "user.missing".into(), Dynamic::from("guest"));
        assert_eq!(result.as_string().unwrap(), "guest");
    }

    #[test]
    fn test_has_path() {
        let event = create_test_event();
        assert!(has_path(event.clone(), "name".into()));
        assert!(has_path(event.clone(), "user.role".into()));
        assert!(!has_path(event.clone(), "missing".into()));
        assert!(!has_path(event, "user.missing".into()));
    }

    #[test]
    fn test_path_equals() {
        let event = create_test_event();
        assert!(path_equals(event.clone(), "name".into(), Dynamic::from("alice")));
        assert!(path_equals(event.clone(), "user.role".into(), Dynamic::from("admin")));
        assert!(!path_equals(event.clone(), "name".into(), Dynamic::from("bob")));
        assert!(!path_equals(event, "missing".into(), Dynamic::from("anything")));
    }

    #[test]
    fn test_to_number() {
        assert_eq!(to_number(Dynamic::from(42), Dynamic::from(0)).as_int().unwrap(), 42);
        assert_eq!(to_number(Dynamic::from(3.14), Dynamic::from(0.0)).as_float().unwrap(), 3.14);
        assert_eq!(to_number(Dynamic::from("123"), Dynamic::from(0)).as_int().unwrap(), 123);
        assert_eq!(to_number(Dynamic::from("12.5"), Dynamic::from(0.0)).as_float().unwrap(), 12.5);
        assert_eq!(to_number(Dynamic::from("invalid"), Dynamic::from(999)).as_int().unwrap(), 999);
    }

    #[test]
    fn test_to_bool() {
        assert_eq!(to_bool(Dynamic::from(true), Dynamic::from(false)).as_bool().unwrap(), true);
        assert_eq!(to_bool(Dynamic::from("yes"), Dynamic::from(false)).as_bool().unwrap(), true);
        assert_eq!(to_bool(Dynamic::from("1"), Dynamic::from(false)).as_bool().unwrap(), true);
        assert_eq!(to_bool(Dynamic::from("false"), Dynamic::from(true)).as_bool().unwrap(), false);
        assert_eq!(to_bool(Dynamic::from(1), Dynamic::from(false)).as_bool().unwrap(), true);
        assert_eq!(to_bool(Dynamic::from(0), Dynamic::from(true)).as_bool().unwrap(), false);
        assert_eq!(to_bool(Dynamic::from("invalid"), Dynamic::from(true)).as_bool().unwrap(), true);
    }
}