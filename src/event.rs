#![allow(dead_code)]
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rhai::Dynamic;
use serde::{Deserialize, Serialize};

/// Flattening style for nested data structures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlattenStyle {
    /// Use dots for objects and brackets for arrays: "user.name", "items[0].value"
    #[default]
    Bracket,
    /// Use dots everywhere: "user.name", "items.0.value"
    Dot,
    /// Use underscores everywhere: "user_name", "items_0_value"
    Underscore,
}

impl FlattenStyle {
    /// Format an object field key
    fn format_object_key(&self, parent: &str, key: &str) -> String {
        match self {
            FlattenStyle::Bracket | FlattenStyle::Dot => {
                if parent.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", parent, key)
                }
            }
            FlattenStyle::Underscore => {
                if parent.is_empty() {
                    key.to_string()
                } else {
                    format!("{}_{}", parent, key)
                }
            }
        }
    }

    /// Format an array index key
    fn format_array_key(&self, parent: &str, index: usize) -> String {
        match self {
            FlattenStyle::Bracket => {
                if parent.is_empty() {
                    format!("[{}]", index)
                } else {
                    format!("{}[{}]", parent, index)
                }
            }
            FlattenStyle::Dot => {
                if parent.is_empty() {
                    index.to_string()
                } else {
                    format!("{}.{}", parent, index)
                }
            }
            FlattenStyle::Underscore => {
                if parent.is_empty() {
                    index.to_string()
                } else {
                    format!("{}_{}", parent, index)
                }
            }
        }
    }
}

/// Flatten a Dynamic value into a flat map of key-value pairs
///
/// This function recursively traverses nested maps and arrays, generating
/// flat keys according to the specified style. It respects max_depth to
/// prevent infinite recursion and memory issues.
///
/// # Arguments
/// * `value` - The Dynamic value to flatten
/// * `style` - The flattening style (Bracket, Dot, Underscore)
/// * `max_depth` - Maximum recursion depth (0 = unlimited)
pub fn flatten_dynamic(
    value: &Dynamic,
    style: FlattenStyle,
    max_depth: usize,
) -> IndexMap<String, Dynamic> {
    let mut result = IndexMap::new();
    // Convert max_depth=0 to unlimited
    let effective_max_depth = if max_depth == 0 {
        usize::MAX
    } else {
        max_depth
    };
    flatten_dynamic_recursive(value, "", style, 0, effective_max_depth, &mut result);
    result
}

/// Recursive helper for flatten_dynamic
fn flatten_dynamic_recursive(
    value: &Dynamic,
    prefix: &str,
    style: FlattenStyle,
    current_depth: usize,
    max_depth: usize,
    result: &mut IndexMap<String, Dynamic>,
) {
    // If we've reached max depth, store as-is
    if current_depth >= max_depth {
        let key = if prefix.is_empty() {
            "value".to_string()
        } else {
            prefix.to_string()
        };
        result.insert(key, value.clone());
        return;
    }

    if let Some(map) = value.clone().try_cast::<rhai::Map>() {
        // Handle Rhai Map (object)
        if map.is_empty() {
            // Empty objects become null values
            let key = if prefix.is_empty() {
                "value".to_string()
            } else {
                prefix.to_string()
            };
            result.insert(key, Dynamic::UNIT);
        } else {
            for (key, val) in map {
                let new_key = style.format_object_key(prefix, key.as_ref());
                flatten_dynamic_recursive(
                    &val,
                    &new_key,
                    style,
                    current_depth + 1,
                    max_depth,
                    result,
                );
            }
        }
    } else if let Some(array) = value.clone().try_cast::<rhai::Array>() {
        // Handle Rhai Array
        if array.is_empty() {
            // Empty arrays become null values
            let key = if prefix.is_empty() {
                "value".to_string()
            } else {
                prefix.to_string()
            };
            result.insert(key, Dynamic::UNIT);
        } else {
            for (index, val) in array.iter().enumerate() {
                let new_key = style.format_array_key(prefix, index);
                flatten_dynamic_recursive(
                    val,
                    &new_key,
                    style,
                    current_depth + 1,
                    max_depth,
                    result,
                );
            }
        }
    } else {
        // Scalar value - store it
        let key = if prefix.is_empty() {
            "value".to_string()
        } else {
            prefix.to_string()
        };
        result.insert(key, value.clone());
    }
}

/// Flatten an entire Event's fields
pub fn flatten_event_fields(
    event: &Event,
    style: FlattenStyle,
    max_depth: usize,
) -> IndexMap<String, Dynamic> {
    let mut result = IndexMap::new();

    for (key, value) in &event.fields {
        if value.clone().try_cast::<rhai::Map>().is_some() {
            // Flatten nested objects
            let flattened = flatten_dynamic(value, style, max_depth);
            for (flat_key, flat_value) in flattened {
                let full_key = style.format_object_key(key, &flat_key);
                result.insert(full_key, flat_value);
            }
        } else if value.clone().try_cast::<rhai::Array>().is_some() {
            // Flatten arrays
            let flattened = flatten_dynamic(value, style, max_depth);
            for (flat_key, flat_value) in flattened {
                let full_key = if flat_key == "value" {
                    key.clone()
                } else {
                    style.format_object_key(key, &flat_key)
                };
                result.insert(full_key, flat_value);
            }
        } else {
            // Scalar values - keep as-is
            result.insert(key.clone(), value.clone());
        }
    }

    result
}

/// Convert serde_json::Value to rhai::Dynamic recursively
/// This is the single source of truth for JSON to Rhai conversion
pub fn json_to_dynamic(value: &serde_json::Value) -> Dynamic {
    match value {
        serde_json::Value::String(s) => Dynamic::from(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from(i)
            } else if let Some(f) = n.as_f64() {
                Dynamic::from(f)
            } else {
                Dynamic::from(n.to_string())
            }
        }
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Array(arr) => {
            // Convert JSON array to Rhai array recursively
            let mut rhai_array = rhai::Array::new();
            for item in arr {
                rhai_array.push(json_to_dynamic(item));
            }
            Dynamic::from(rhai_array)
        }
        serde_json::Value::Object(obj) => {
            // Convert JSON object to Rhai map recursively
            let mut rhai_map = rhai::Map::new();
            for (key, val) in obj {
                rhai_map.insert(key.clone().into(), json_to_dynamic(val));
            }
            Dynamic::from(rhai_map)
        }
    }
}

/// Core field name constants to ensure consistency across the codebase
pub const TIMESTAMP_FIELD_NAMES: &[&str] = &[
    "ts",
    "_ts",
    "timestamp",
    "at",
    "time",
    "@timestamp",
    "log_timestamp",
    "event_time",
    "datetime",
    "date_time",
    "created_at",
    "logged_at",
    "_t",
    "@t",
    "t",
];

pub const LEVEL_FIELD_NAMES: &[&str] = &[
    "level",
    "lvl",
    "severity",
    "log_level",
    "loglevel",
    "priority",
    "sev",
    "@level",
    "log_severity",
    "error_level",
    "event_level",
    "_level",
    "@l",
];

pub const MESSAGE_FIELD_NAMES: &[&str] = &[
    "msg",
    "message",
    "content",
    "data",
    "log",
    "text",
    "description",
    "details",
    "body",
    "payload",
    "event_message",
    "log_message",
    "_message",
    "@message",
    "@m",
];

#[derive(Debug, Clone, Default)]
pub struct Event {
    pub fields: IndexMap<String, Dynamic>,
    pub original_line: String,
    pub line_num: Option<usize>,
    pub filename: Option<String>,
    /// Parsed timestamp field for efficient timestamp operations
    /// This is populated automatically when timestamps are extracted from fields
    pub parsed_ts: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

impl Event {
    pub fn with_capacity(original_line: String, capacity: usize) -> Self {
        Self {
            fields: IndexMap::with_capacity(capacity),
            original_line,
            line_num: None,
            filename: None,
            parsed_ts: None,
        }
    }

    pub fn default_with_line(line: String) -> Self {
        Self {
            original_line: line,
            ..Default::default()
        }
    }

    pub fn set_field(&mut self, key: String, value: Dynamic) {
        self.fields.insert(key, value);
    }

    pub fn set_metadata(&mut self, line_num: usize, filename: Option<String>) {
        self.line_num = Some(line_num);
        self.filename = filename;
    }

    /// Filter to only show specified keys, keeping only fields that actually exist
    pub fn filter_keys(&mut self, keys: &[String]) {
        let mut new_fields = IndexMap::with_capacity(keys.len());

        // Only include fields that are both requested and exist
        for key in keys {
            if let Some(value) = self.fields.get(key) {
                new_fields.insert(key.clone(), value.clone());
            }
        }

        self.fields = new_fields;
    }

    /// Try to parse and extract timestamp from the fields map
    pub fn extract_timestamp(&mut self) {
        self.extract_timestamp_with_parser(None);
    }

    /// Try to parse and extract timestamp from the fields map with optional adaptive parser
    pub fn extract_timestamp_with_parser(
        &mut self,
        parser: Option<&mut crate::timestamp::AdaptiveTsParser>,
    ) {
        self.extract_timestamp_with_config(parser, &crate::timestamp::TsConfig::default());
    }

    /// Extract timestamp with configuration
    pub fn extract_timestamp_with_config(
        &mut self,
        parser: Option<&mut crate::timestamp::AdaptiveTsParser>,
        ts_config: &crate::timestamp::TsConfig,
    ) {
        // Extract and parse timestamp with comprehensive field recognition
        if self.parsed_ts.is_none() {
            if let Some((_field_name, ts_str)) =
                crate::timestamp::identify_timestamp_field(&self.fields, ts_config)
            {
                let parsed_ts = if let Some(parser) = parser {
                    parser.parse_ts_with_config(
                        &ts_str,
                        ts_config.custom_format.as_deref(),
                        ts_config.default_timezone.as_deref(),
                    )
                } else {
                    // Use the enhanced adaptive parser as default
                    let mut default_parser = crate::timestamp::AdaptiveTsParser::new();
                    default_parser.parse_ts_with_config(
                        &ts_str,
                        ts_config.custom_format.as_deref(),
                        ts_config.default_timezone.as_deref(),
                    )
                };

                if let Some(ts) = parsed_ts {
                    self.parsed_ts = Some(ts);
                }
            }
        }
    }
}

impl std::fmt::Display for FieldValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldValue::String(s) => write!(f, "{}", s),
            FieldValue::Number(n) => write!(f, "{}", n),
            FieldValue::Boolean(b) => write!(f, "{}", b),
            FieldValue::Null => write!(f, "null"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::{Array, Map};

    #[test]
    fn test_flatten_dynamic_simple_object() {
        let mut map = Map::new();
        map.insert("name".into(), Dynamic::from("alice"));
        map.insert("age".into(), Dynamic::from(25i64));

        let dynamic_map = Dynamic::from(map);
        let flattened = flatten_dynamic(&dynamic_map, FlattenStyle::Bracket, 10);

        assert_eq!(flattened.get("name").unwrap().to_string(), "alice");
        assert_eq!(flattened.get("age").unwrap().to_string(), "25");
    }

    #[test]
    fn test_flatten_dynamic_nested_object() {
        let mut inner_map = Map::new();
        inner_map.insert("street".into(), Dynamic::from("123 Main St"));
        inner_map.insert("city".into(), Dynamic::from("Boston"));

        let mut outer_map = Map::new();
        outer_map.insert("name".into(), Dynamic::from("alice"));
        outer_map.insert("address".into(), Dynamic::from(inner_map));

        let dynamic_map = Dynamic::from(outer_map);
        let flattened = flatten_dynamic(&dynamic_map, FlattenStyle::Bracket, 10);

        assert_eq!(flattened.get("name").unwrap().to_string(), "alice");
        assert_eq!(
            flattened.get("address.street").unwrap().to_string(),
            "123 Main St"
        );
        assert_eq!(flattened.get("address.city").unwrap().to_string(), "Boston");
    }

    #[test]
    fn test_flatten_dynamic_array() {
        let array = vec![
            Dynamic::from("item1"),
            Dynamic::from("item2"),
            Dynamic::from(42i64),
        ];

        let dynamic_array = Dynamic::from(array);
        let flattened = flatten_dynamic(&dynamic_array, FlattenStyle::Bracket, 10);

        assert_eq!(flattened.get("[0]").unwrap().to_string(), "item1");
        assert_eq!(flattened.get("[1]").unwrap().to_string(), "item2");
        assert_eq!(flattened.get("[2]").unwrap().to_string(), "42");
    }

    #[test]
    fn test_flatten_dynamic_mixed_structure() {
        let mut item1 = Map::new();
        item1.insert("id".into(), Dynamic::from(1i64));
        item1.insert("name".into(), Dynamic::from("first"));

        let mut item2 = Map::new();
        item2.insert("id".into(), Dynamic::from(2i64));
        item2.insert("name".into(), Dynamic::from("second"));

        let array = vec![Dynamic::from(item1), Dynamic::from(item2)];

        let mut root = Map::new();
        root.insert("user".into(), Dynamic::from("alice"));
        root.insert("items".into(), Dynamic::from(array));

        let dynamic_root = Dynamic::from(root);
        let flattened = flatten_dynamic(&dynamic_root, FlattenStyle::Bracket, 10);

        assert_eq!(flattened.get("user").unwrap().to_string(), "alice");
        assert_eq!(flattened.get("items[0].id").unwrap().to_string(), "1");
        assert_eq!(flattened.get("items[0].name").unwrap().to_string(), "first");
        assert_eq!(flattened.get("items[1].id").unwrap().to_string(), "2");
        assert_eq!(
            flattened.get("items[1].name").unwrap().to_string(),
            "second"
        );
    }

    #[test]
    fn test_flatten_styles() {
        let mut inner = Map::new();
        inner.insert("value".into(), Dynamic::from(42i64));

        let array = vec![Dynamic::from(inner)];

        let mut root = Map::new();
        root.insert("data".into(), Dynamic::from(array));

        let dynamic_root = Dynamic::from(root);

        // Test bracket style
        let bracket = flatten_dynamic(&dynamic_root, FlattenStyle::Bracket, 10);
        assert!(bracket.contains_key("data[0].value"));

        // Test dot style
        let dot = flatten_dynamic(&dynamic_root, FlattenStyle::Dot, 10);
        assert!(dot.contains_key("data.0.value"));

        // Test underscore style
        let underscore = flatten_dynamic(&dynamic_root, FlattenStyle::Underscore, 10);
        assert!(underscore.contains_key("data_0_value"));
    }

    #[test]
    fn test_flatten_max_depth() {
        let mut deep = Map::new();
        deep.insert("level4".into(), Dynamic::from("deep"));

        let mut level3 = Map::new();
        level3.insert("level3".into(), Dynamic::from(deep));

        let mut level2 = Map::new();
        level2.insert("level2".into(), Dynamic::from(level3));

        let mut level1 = Map::new();
        level1.insert("level1".into(), Dynamic::from(level2));

        let dynamic_root = Dynamic::from(level1);

        // With max_depth=2, should stop at level2
        let flattened = flatten_dynamic(&dynamic_root, FlattenStyle::Bracket, 2);

        // Should have flattened up to level2 but not deeper
        assert!(flattened.contains_key("level1.level2"));
        assert!(!flattened.contains_key("level1.level2.level3.level4"));
    }

    #[test]
    fn test_flatten_empty_structures() {
        let empty_map = Map::new();
        let empty_array = Array::new();

        let flattened_map = flatten_dynamic(&Dynamic::from(empty_map), FlattenStyle::Bracket, 10);
        let flattened_array =
            flatten_dynamic(&Dynamic::from(empty_array), FlattenStyle::Bracket, 10);

        // Empty structures should produce a single null value
        assert_eq!(flattened_map.len(), 1);
        assert!(flattened_map.get("value").unwrap().is_unit());

        assert_eq!(flattened_array.len(), 1);
        assert!(flattened_array.get("value").unwrap().is_unit());
    }

    #[test]
    fn test_flatten_unlimited_depth() {
        // Create a very deeply nested structure
        let mut deep = Map::new();
        deep.insert("level8".into(), Dynamic::from("deepest"));

        let mut level7 = Map::new();
        level7.insert("level7".into(), Dynamic::from(deep));

        let mut level6 = Map::new();
        level6.insert("level6".into(), Dynamic::from(level7));

        let mut level5 = Map::new();
        level5.insert("level5".into(), Dynamic::from(level6));

        let mut level4 = Map::new();
        level4.insert("level4".into(), Dynamic::from(level5));

        let mut level3 = Map::new();
        level3.insert("level3".into(), Dynamic::from(level4));

        let mut level2 = Map::new();
        level2.insert("level2".into(), Dynamic::from(level3));

        let mut level1 = Map::new();
        level1.insert("level1".into(), Dynamic::from(level2));

        let dynamic_root = Dynamic::from(level1);

        // With max_depth=0 (unlimited), should flatten completely
        let unlimited = flatten_dynamic(&dynamic_root, FlattenStyle::Bracket, 0);

        // Should have fully flattened the deep structure
        assert!(unlimited.contains_key("level1.level2.level3.level4.level5.level6.level7.level8"));
        assert_eq!(
            unlimited
                .get("level1.level2.level3.level4.level5.level6.level7.level8")
                .unwrap()
                .to_string(),
            "deepest"
        );

        // Compare with limited depth
        let limited = flatten_dynamic(&dynamic_root, FlattenStyle::Bracket, 3);

        // Should stop at level 3 and contain the remaining structure as a string
        assert!(limited.contains_key("level1.level2.level3"));
        assert!(!limited.contains_key("level1.level2.level3.level4.level5.level6.level7.level8"));
    }
}
