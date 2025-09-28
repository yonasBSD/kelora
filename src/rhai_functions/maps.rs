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

    // Default flatten() - uses bracket style, unlimited depth
    engine.register_fn("flatten", |map: Map| -> Map {
        let dynamic_map = Dynamic::from(map);
        let flattened = flatten_dynamic(&dynamic_map, FlattenStyle::default(), 0);
        convert_indexmap_to_rhai_map(flattened)
    });

    // flatten(style) - specify style, unlimited depth
    engine.register_fn("flatten", |map: Map, style: &str| -> Map {
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

    // flatten(style, max_depth) - full control
    engine.register_fn("flatten", |map: Map, style: &str, max_depth: i64| -> Map {
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
    });

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
    engine.register_fn("has_field", |map: Map, key: rhai::ImmutableString| -> bool {
        map.get(key.as_str()).map_or(false, |value| !value.is_unit())
    });
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
}
