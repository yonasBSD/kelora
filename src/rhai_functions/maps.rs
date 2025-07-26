use rhai::{Engine, Map};

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
            assert_eq!(map.get("boolean").unwrap().clone().cast::<bool>(), true);
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
                assert_eq!(nested_map.get("nested").unwrap().clone().cast::<String>(), "value");
            } else {
                panic!("Object field is not a proper Rhai map");
            }
        } else {
            panic!("Root object is not a proper Rhai map");
        }
    }
}
