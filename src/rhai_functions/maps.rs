use rhai::{Array, Dynamic, Engine, Map};
use serde_json;

/// Registers map merge/enrich operators to the Rhai engine.
pub fn register_functions(engine: &mut Engine) {
    // event.merge(map): overwrites existing keys
    engine.register_fn("merge", |lhs: &mut Map, rhs: Map| {
        for (k, v) in rhs {
            lhs.insert(k, v);
        }
    });

    // event.enrich(map): inserts only if key is missing
    engine.register_fn("enrich", |lhs: &mut Map, rhs: Map| {
        for (k, v) in rhs {
            lhs.entry(k).or_insert(v);
        }
    });

    // event += map: shorthand for merge()
    engine.register_fn("+=", |lhs: &mut Map, rhs: Map| {
        for (k, v) in rhs {
            lhs.insert(k, v);
        }
    });

    // get_path(map, path, default): Extract value from nested path like "first[4].subkey.subsub[4]"
    engine.register_fn(
        "get_path",
        |map: Map, path: &str, default: Dynamic| -> Dynamic {
            get_path_impl(&Dynamic::from(map), path).unwrap_or(default)
        },
    );

    // get_path(map, path): Extract value from nested path with null default
    engine.register_fn("get_path", |map: Map, path: &str| -> Dynamic {
        get_path_impl(&Dynamic::from(map), path).unwrap_or(Dynamic::UNIT)
    });

    // get_path(json_string, path, default): Parse JSON string then extract value
    engine.register_fn(
        "get_path",
        |json_str: &str, path: &str, default: Dynamic| -> Dynamic {
            if let Ok(parsed) = parse_json_string(json_str) {
                get_path_impl(&parsed, path).unwrap_or(default)
            } else {
                default
            }
        },
    );

    // get_path(json_string, path): Parse JSON string then extract value with null default
    engine.register_fn("get_path", |json_str: &str, path: &str| -> Dynamic {
        if let Ok(parsed) = parse_json_string(json_str) {
            get_path_impl(&parsed, path).unwrap_or(Dynamic::UNIT)
        } else {
            Dynamic::UNIT
        }
    });
}

/// Parse a path segment that might contain array indices
fn parse_path_segment(segment: &str) -> Vec<PathComponent> {
    let mut components = Vec::new();
    let mut current = String::new();
    let mut in_brackets = false;

    for ch in segment.chars() {
        match ch {
            '[' if !in_brackets => {
                if !current.is_empty() {
                    components.push(PathComponent::Key(current.clone()));
                    current.clear();
                }
                in_brackets = true;
            }
            ']' if in_brackets => {
                if let Ok(index) = current.parse::<i64>() {
                    components.push(PathComponent::Index(index));
                }
                current.clear();
                in_brackets = false;
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() && !in_brackets {
        components.push(PathComponent::Key(current));
    }

    components
}

/// Parse a full path string into components
fn parse_path(path: &str) -> Vec<PathComponent> {
    let mut components = Vec::new();

    for segment in path.split('.') {
        if !segment.is_empty() {
            components.extend(parse_path_segment(segment));
        }
    }

    components
}

#[derive(Debug, Clone)]
enum PathComponent {
    Key(String),
    Index(i64),
}

/// Navigate through a nested structure using parsed path components
fn get_path_impl(value: &Dynamic, path: &str) -> Option<Dynamic> {
    let components = parse_path(path);
    let mut current = value.clone();

    for component in components {
        match component {
            PathComponent::Key(key) => {
                let next_value = {
                    if let Some(map) = current.read_lock::<Map>() {
                        map.get(key.as_str()).cloned()
                    } else {
                        None
                    }
                };

                if let Some(next_value) = next_value {
                    current = next_value;
                } else {
                    return None;
                }
            }
            PathComponent::Index(index) => {
                let next_value = {
                    if let Some(array) = current.read_lock::<Array>() {
                        let idx = if index < 0 {
                            // Negative indexing from the end
                            array.len() as i64 + index
                        } else {
                            index
                        };

                        if idx >= 0 && (idx as usize) < array.len() {
                            Some(array[idx as usize].clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                if let Some(next_value) = next_value {
                    current = next_value;
                } else {
                    return None;
                }
            }
        }
    }

    Some(current)
}

/// Parse a JSON string into a Dynamic value
fn parse_json_string(json_str: &str) -> Result<Dynamic, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(json_str)?;
    Ok(json_to_dynamic(value))
}

/// Convert a serde_json::Value to a Rhai Dynamic
fn json_to_dynamic(value: serde_json::Value) -> Dynamic {
    match value {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::UNIT
            }
        }
        serde_json::Value::String(s) => Dynamic::from(s),
        serde_json::Value::Array(arr) => {
            let mut rhai_array = Array::new();
            for item in arr {
                rhai_array.push(json_to_dynamic(item));
            }
            Dynamic::from(rhai_array)
        }
        serde_json::Value::Object(obj) => {
            let mut rhai_map = Map::new();
            for (k, v) in obj {
                rhai_map.insert(k.into(), json_to_dynamic(v));
            }
            Dynamic::from(rhai_map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::{Array, Dynamic, Engine, Map};

    fn create_test_engine() -> Engine {
        let mut engine = Engine::new();
        register_functions(&mut engine);
        engine
    }

    fn create_test_map() -> Map {
        let mut map = Map::new();
        map.insert("name".into(), Dynamic::from("alice"));
        map.insert("age".into(), Dynamic::from(25i64));

        let mut scores = Array::new();
        scores.push(Dynamic::from(10i64));
        scores.push(Dynamic::from(20i64));
        scores.push(Dynamic::from(30i64));
        map.insert("scores".into(), Dynamic::from(scores));

        let mut details = Map::new();
        details.insert("city".into(), Dynamic::from("Boston"));
        details.insert("active".into(), Dynamic::from(true));

        let mut items = Array::new();
        let mut item1 = Map::new();
        item1.insert("id".into(), Dynamic::from(1i64));
        item1.insert("name".into(), Dynamic::from("item1"));
        items.push(Dynamic::from(item1));

        let mut item2 = Map::new();
        item2.insert("id".into(), Dynamic::from(2i64));
        item2.insert("name".into(), Dynamic::from("item2"));
        items.push(Dynamic::from(item2));

        details.insert("items".into(), Dynamic::from(items));
        map.insert("details".into(), Dynamic::from(details));

        map
    }

    #[test]
    fn test_get_path_basic_key_access() {
        let engine = create_test_engine();
        let map = create_test_map();

        let result: String = engine
            .eval_with_scope(
                &mut rhai::Scope::new(),
                &format!("get_path(#{:?}, \"name\")", map),
            )
            .unwrap();

        assert_eq!(result, "alice");
    }

    #[test]
    fn test_get_path_array_access() {
        let engine = create_test_engine();
        let map = create_test_map();

        let result: i64 = engine
            .eval_with_scope(
                &mut rhai::Scope::new(),
                &format!("get_path(#{:?}, \"scores[1]\")", map),
            )
            .unwrap();

        assert_eq!(result, 20);
    }

    #[test]
    fn test_get_path_negative_array_access() {
        let engine = create_test_engine();
        let map = create_test_map();

        let result: i64 = engine
            .eval_with_scope(
                &mut rhai::Scope::new(),
                &format!("get_path(#{:?}, \"scores[-1]\")", map),
            )
            .unwrap();

        assert_eq!(result, 30);
    }

    #[test]
    fn test_get_path_nested_object_access() {
        let engine = create_test_engine();
        let map = create_test_map();

        let result: String = engine
            .eval_with_scope(
                &mut rhai::Scope::new(),
                &format!("get_path(#{:?}, \"details.city\")", map),
            )
            .unwrap();

        assert_eq!(result, "Boston");
    }

    #[test]
    fn test_get_path_deeply_nested_access() {
        let engine = create_test_engine();
        let map = create_test_map();

        let result: String = engine
            .eval_with_scope(
                &mut rhai::Scope::new(),
                &format!("get_path(#{:?}, \"details.items[1].name\")", map),
            )
            .unwrap();

        assert_eq!(result, "item2");
    }

    #[test]
    fn test_get_path_missing_key_with_default() {
        let engine = create_test_engine();
        let map = create_test_map();

        let result: String = engine
            .eval_with_scope(
                &mut rhai::Scope::new(),
                &format!("get_path(#{:?}, \"nonexistent\", \"default\")", map),
            )
            .unwrap();

        assert_eq!(result, "default");
    }

    #[test]
    fn test_get_path_missing_key_without_default() {
        let engine = create_test_engine();
        let map = create_test_map();

        let result: Dynamic = engine
            .eval_with_scope(
                &mut rhai::Scope::new(),
                &format!("get_path(#{:?}, \"nonexistent\")", map),
            )
            .unwrap();

        assert!(result.is_unit());
    }

    #[test]
    fn test_get_path_invalid_array_index() {
        let engine = create_test_engine();
        let map = create_test_map();

        let result: i64 = engine
            .eval_with_scope(
                &mut rhai::Scope::new(),
                &format!("get_path(#{:?}, \"scores[99]\", 0)", map),
            )
            .unwrap();

        assert_eq!(result, 0);
    }

    #[test]
    fn test_get_path_json_string_parsing() {
        let engine = create_test_engine();
        let json_str = r#"{"user": {"name": "bob", "scores": [100, 200, 300]}}"#;

        let result: String = engine
            .eval(&format!(
                "get_path(\"{}\", \"user.name\")",
                json_str.replace("\"", "\\\"")
            ))
            .unwrap();

        assert_eq!(result, "bob");
    }

    #[test]
    fn test_get_path_json_string_with_array() {
        let engine = create_test_engine();
        let json_str = r#"{"user": {"name": "bob", "scores": [100, 200, 300]}}"#;

        let result: i64 = engine
            .eval(&format!(
                "get_path(\"{}\", \"user.scores[0]\")",
                json_str.replace("\"", "\\\"")
            ))
            .unwrap();

        assert_eq!(result, 100);
    }

    #[test]
    fn test_get_path_json_string_with_default() {
        let engine = create_test_engine();
        let json_str = r#"{"user": {"name": "bob"}}"#;

        let result: String = engine
            .eval(&format!(
                "get_path(\"{}\", \"user.missing\", \"fallback\")",
                json_str.replace("\"", "\\\"")
            ))
            .unwrap();

        assert_eq!(result, "fallback");
    }

    #[test]
    fn test_get_path_invalid_json_string() {
        let engine = create_test_engine();
        let invalid_json = "invalid json";

        let result: String = engine
            .eval(&format!(
                "get_path(\"{}\", \"any.path\", \"default\")",
                invalid_json
            ))
            .unwrap();

        assert_eq!(result, "default");
    }

    #[test]
    fn test_parse_path_components() {
        let components = parse_path("user.scores[0]");
        assert_eq!(components.len(), 3);

        match &components[0] {
            PathComponent::Key(key) => assert_eq!(key, "user"),
            _ => assert!(false, "Expected key component"),
        }

        match &components[1] {
            PathComponent::Key(key) => assert_eq!(key, "scores"),
            _ => assert!(false, "Expected key component"),
        }

        match &components[2] {
            PathComponent::Index(index) => assert_eq!(*index, 0),
            _ => assert!(false, "Expected index component"),
        }
    }

    #[test]
    fn test_parse_path_negative_index() {
        let components = parse_path("items[-1]");
        assert_eq!(components.len(), 2);

        match &components[0] {
            PathComponent::Key(key) => assert_eq!(key, "items"),
            _ => assert!(false, "Expected key component"),
        }

        match &components[1] {
            PathComponent::Index(index) => assert_eq!(*index, -1),
            _ => assert!(false, "Expected index component"),
        }
    }

    #[test]
    fn test_parse_path_complex() {
        let components = parse_path("data.items[2].metadata.tags[0]");
        assert_eq!(components.len(), 6);

        let expected = vec![
            PathComponent::Key("data".to_string()),
            PathComponent::Key("items".to_string()),
            PathComponent::Index(2),
            PathComponent::Key("metadata".to_string()),
            PathComponent::Key("tags".to_string()),
            PathComponent::Index(0),
        ];

        for (i, expected_component) in expected.iter().enumerate() {
            match (expected_component, &components[i]) {
                (PathComponent::Key(expected_key), PathComponent::Key(actual_key)) => {
                    assert_eq!(expected_key, actual_key);
                }
                (PathComponent::Index(expected_index), PathComponent::Index(actual_index)) => {
                    assert_eq!(expected_index, actual_index);
                }
                _ => assert!(false, "Component mismatch at index {}", i),
            }
        }
    }

    #[test]
    fn test_json_to_dynamic_conversion() {
        let json_val = serde_json::json!({
            "string": "hello",
            "number": 42,
            "boolean": true,
            "null": null,
            "array": [1, 2, 3],
            "object": {"nested": "value"}
        });

        let dynamic = json_to_dynamic(json_val);

        {
            let map = dynamic.read_lock::<Map>().unwrap();
            assert_eq!(map.get("string").unwrap().clone().cast::<String>(), "hello");
            assert_eq!(map.get("number").unwrap().clone().cast::<i64>(), 42);
            assert_eq!(map.get("boolean").unwrap().clone().cast::<bool>(), true);
            assert!(map.get("null").unwrap().is_unit());
        }

        {
            let map = dynamic.read_lock::<Map>().unwrap();
            let array = map.get("array").unwrap().read_lock::<Array>().unwrap();
            assert_eq!(array.len(), 3);
            assert_eq!(array[0].clone().cast::<i64>(), 1);
        }

        {
            let map = dynamic.read_lock::<Map>().unwrap();
            let nested_map = map.get("object").unwrap().read_lock::<Map>().unwrap();
            assert_eq!(
                nested_map.get("nested").unwrap().clone().cast::<String>(),
                "value"
            );
        }
    }
}
