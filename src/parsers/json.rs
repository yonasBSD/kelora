#![allow(dead_code)]
use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::{Context, Result};

pub struct JsonlParser;

impl JsonlParser {
    pub fn new() -> Self {
        Self
    }
}

impl EventParser for JsonlParser {
    fn parse(&self, line: &str) -> Result<Event> {
        let line = line.trim_end_matches('\n').trim_end_matches('\r');
        let json_value: serde_json::Value = serde_json::from_str(line).with_context(|| {
            format!(
                "Failed to parse JSON: {}",
                crate::config::format_error_line(line)
            )
        })?;

        if let serde_json::Value::Object(ref map) = json_value {
            // Pre-allocate HashMap with capacity based on JSON object size
            let mut event = Event::with_capacity(line.to_string(), map.len());

            for (key, value) in map {
                // Convert serde_json::Value to rhai::Dynamic using shared function
                let dynamic_value = crate::event::json_to_dynamic(value);
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
    fn test_json_parser_basic() {
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
    fn test_json_parser_complex() {
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
