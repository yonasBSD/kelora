use crate::event::{flatten_dynamic, FlattenStyle};
use indexmap::IndexMap;
use rhai::{Dynamic, Engine, Map};

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

    // Flattening functions

    // Default flattened() - uses bracket style, unlimited depth
    engine.register_fn("flattened", |map: Map| -> Map {
        let dynamic_map = Dynamic::from(map);
        let flattened = flatten_dynamic(&dynamic_map, FlattenStyle::default(), 0);
        convert_indexmap_to_rhai_map(flattened)
    });

    // flattened(style) - specify style, unlimited depth
    engine.register_fn("flattened", |map: Map, style: &str| -> Map {
        let flatten_style = match style {
            "dot" => FlattenStyle::Dot,
            "bracket" => FlattenStyle::Bracket,
            "underscore" => FlattenStyle::Underscore,
            _ => FlattenStyle::default(), // Default to bracket for unknown styles
        };
        let dynamic_map = Dynamic::from(map);
        let flattened = flatten_dynamic(&dynamic_map, flatten_style, 0);
        convert_indexmap_to_rhai_map(flattened)
    });

    // flattened(style, max_depth) - full control
    engine.register_fn(
        "flattened",
        |map: Map, style: &str, max_depth: i64| -> Map {
            let flatten_style = match style {
                "dot" => FlattenStyle::Dot,
                "bracket" => FlattenStyle::Bracket,
                "underscore" => FlattenStyle::Underscore,
                _ => FlattenStyle::default(),
            };
            let max_depth = if max_depth < 0 { 0 } else { max_depth as usize }; // negative = unlimited
            let dynamic_map = Dynamic::from(map);
            let flattened = flatten_dynamic(&dynamic_map, flatten_style, max_depth);
            convert_indexmap_to_rhai_map(flattened)
        },
    );

    // flatten_field(field_name) - flatten just one field from the map
    engine.register_fn("flatten_field", |map: &Map, field_name: &str| -> Map {
        let mut result = Map::new();

        if let Some(field_value) = map.get(field_name) {
            let flattened = flatten_dynamic(field_value, FlattenStyle::default(), 0);
            for (key, value) in flattened {
                let full_key = if key == "value" {
                    field_name.to_string()
                } else {
                    format!("{}.{}", field_name, key)
                };
                result.insert(full_key.into(), value);
            }
        }

        result
    });

    // map.has_field(key) - check if map contains key AND value is not unit ()
    engine.register_fn(
        "has_field",
        |map: Map, key: rhai::ImmutableString| -> bool {
            map.get(key.as_str()).is_some_and(|value| !value.is_unit())
        },
    );

    // map.rename_field(old_name, new_name) - rename a field, returns true if successful
    engine.register_fn("rename_field", rename_field);
}

/// Rename a field in the map
/// Returns true if old_name existed and was renamed, false otherwise
/// If new_name already exists, it will be overwritten
pub fn rename_field(map: &mut Map, old_name: &str, new_name: &str) -> bool {
    if let Some(value) = map.remove(old_name) {
        map.insert(new_name.into(), value);
        true
    } else {
        false
    }
}

/// Convert IndexMap<String, Dynamic> to rhai::Map
fn convert_indexmap_to_rhai_map(indexmap: IndexMap<String, Dynamic>) -> Map {
    let mut map = Map::new();
    for (key, value) in indexmap {
        map.insert(key.into(), value);
    }
    map
}

#[cfg(test)]
mod tests {
    use rhai::Map;

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

        let dynamic = crate::event::json_to_dynamic(&json_val);

        // Test that JSON arrays become proper Rhai arrays
        if let Some(map) = dynamic.clone().try_cast::<Map>() {
            assert_eq!(map.get("string").unwrap().clone().cast::<String>(), "hello");
            assert_eq!(map.get("number").unwrap().clone().cast::<i64>(), 42);
            assert!(map.get("boolean").unwrap().clone().cast::<bool>());
            assert!(map.get("null").unwrap().is_unit());

            // Test array conversion
            if let Some(array) = map.get("array").unwrap().clone().try_cast::<rhai::Array>() {
                assert_eq!(array.len(), 3);
                assert_eq!(array[0].clone().cast::<i64>(), 1);
            } else {
                panic!("Array field is not a proper Rhai array");
            }

            // Test nested object conversion
            if let Some(nested_map) = map.get("object").unwrap().clone().try_cast::<Map>() {
                assert_eq!(
                    nested_map.get("nested").unwrap().clone().cast::<String>(),
                    "value"
                );
            } else {
                panic!("Object field is not a proper Rhai map");
            }
        } else {
            panic!("Root object is not a proper Rhai map");
        }
    }

    #[test]
    fn test_rename_field_success() {
        use super::*;
        use rhai::Dynamic;

        let mut map = Map::new();
        map.insert("old_name".into(), Dynamic::from("value"));
        map.insert("other".into(), Dynamic::from(42i64));

        let result = rename_field(&mut map, "old_name", "new_name");

        assert!(result);
        assert!(!map.contains_key("old_name"));
        assert_eq!(
            map.get("new_name").unwrap().clone().cast::<String>(),
            "value"
        );
        assert_eq!(map.get("other").unwrap().as_int().unwrap(), 42i64);
    }

    #[test]
    fn test_rename_field_missing_source() {
        use super::*;
        use rhai::Dynamic;

        let mut map = Map::new();
        map.insert("existing".into(), Dynamic::from("value"));

        let result = rename_field(&mut map, "nonexistent", "new_name");

        assert!(!result);
        assert!(map.contains_key("existing"));
        assert!(!map.contains_key("new_name"));
    }

    #[test]
    fn test_rename_field_overwrite_target() {
        use super::*;
        use rhai::Dynamic;

        let mut map = Map::new();
        map.insert("old_name".into(), Dynamic::from("new_value"));
        map.insert("new_name".into(), Dynamic::from("old_value"));

        let result = rename_field(&mut map, "old_name", "new_name");

        assert!(result);
        assert!(!map.contains_key("old_name"));
        assert_eq!(
            map.get("new_name").unwrap().clone().cast::<String>(),
            "new_value"
        );
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_rename_field_same_name() {
        use super::*;
        use rhai::Dynamic;

        let mut map = Map::new();
        map.insert("field".into(), Dynamic::from("value"));

        let result = rename_field(&mut map, "field", "field");

        assert!(result);
        assert_eq!(map.get("field").unwrap().clone().cast::<String>(), "value");
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_rename_field_rhai_integration() {
        use super::*;
        use rhai::Engine;

        let mut engine = Engine::new();
        register_functions(&mut engine);

        let result = engine
            .eval::<bool>(
                r#"
            let e = #{timestamp: "2024-01-01", level: "info"};
            e.rename_field("timestamp", "ts")
        "#,
            )
            .unwrap();

        assert!(result);

        let map = engine
            .eval::<Map>(
                r#"
            let e = #{timestamp: "2024-01-01", level: "info"};
            e.rename_field("timestamp", "ts");
            e
        "#,
            )
            .unwrap();

        assert!(!map.contains_key("timestamp"));
        assert_eq!(
            map.get("ts").unwrap().clone().cast::<String>(),
            "2024-01-01"
        );
        assert_eq!(map.get("level").unwrap().clone().cast::<String>(), "info");
    }

    #[test]
    fn test_rename_field_chained() {
        use super::*;
        use rhai::Engine;

        let mut engine = Engine::new();
        register_functions(&mut engine);

        let map = engine
            .eval::<Map>(
                r#"
            let e = #{a: 1, b: 2, c: 3};
            e.rename_field("a", "x");
            e.rename_field("b", "y");
            e.rename_field("c", "z");
            e
        "#,
            )
            .unwrap();

        assert_eq!(map.get("x").unwrap().clone().cast::<i64>(), 1);
        assert_eq!(map.get("y").unwrap().clone().cast::<i64>(), 2);
        assert_eq!(map.get("z").unwrap().clone().cast::<i64>(), 3);
        assert!(!map.contains_key("a"));
        assert!(!map.contains_key("b"));
        assert!(!map.contains_key("c"));
    }
}
