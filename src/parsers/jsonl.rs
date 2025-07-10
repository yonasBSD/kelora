use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::{Context, Result};
use rhai::Dynamic;

/// Convert serde_json::Value to rhai::Dynamic
fn json_to_dynamic(value: &serde_json::Value) -> Dynamic {
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
        _ => Dynamic::from(value.to_string()), // Complex types as strings
    }
}

pub struct JsonlParser;

impl JsonlParser {
    pub fn new() -> Self {
        Self
    }
}

impl EventParser for JsonlParser {
    fn parse(&self, line: &str) -> Result<Event> {
        let json_value: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("Failed to parse JSON: {}", line))?;

        if let serde_json::Value::Object(ref map) = json_value {
            // Pre-allocate HashMap with capacity based on JSON object size
            let mut event = Event::with_capacity(line.to_string(), map.len());

            for (key, value) in map {
                // Convert serde_json::Value to rhai::Dynamic
                let dynamic_value = json_to_dynamic(value);
                event.set_field(key.clone(), dynamic_value);
            }

            event.extract_timestamp();
            Ok(event)
        } else {
            Err(anyhow::anyhow!("Expected JSON object, got: {}", json_value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::EventParser;

    #[test]
    fn test_jsonl_parser_basic() {
        let parser = JsonlParser::new();
        let result =
            EventParser::parse(&parser, r#"{"level":"info","message":"test","count":42}"#).unwrap();

        assert_eq!(
            result
                .fields
                .get("level")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "info"
        );
        assert_eq!(
            result
                .fields
                .get("message")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "test"
        );
        assert!(result.fields.get("count").is_some());
        assert_eq!(result.fields.get("count").unwrap().as_int().unwrap(), 42);
    }

    #[test]
    fn test_jsonl_parser_complex() {
        let parser = JsonlParser::new();
        let result = EventParser::parse(
            &parser,
            r#"{"timestamp":"2023-01-01T12:00:00Z","level":"error","user":"alice","status":404}"#,
        )
        .unwrap();

        assert_eq!(
            result
                .fields
                .get("level")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "error"
        );
        assert!(result.fields.get("user").is_some());
        assert_eq!(
            result
                .fields
                .get("user")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "alice"
        );
        assert!(result.fields.get("status").is_some());
        assert_eq!(result.fields.get("status").unwrap().as_int().unwrap(), 404);
    }
}
