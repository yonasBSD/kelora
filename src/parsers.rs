use crate::event::Event;
use anyhow::{Context, Result};
use rhai::Dynamic;

pub trait Parser {
    fn parse(&self, line: &str) -> Result<Event, anyhow::Error>;
}

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
        },
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Null => Dynamic::UNIT,
        _ => Dynamic::from(value.to_string()), // Complex types as strings
    }
}

// JSONL Parser
pub struct JsonlParser;

impl JsonlParser {
    pub fn new() -> Self {
        Self
    }
}

impl Parser for JsonlParser {
    fn parse(&self, line: &str) -> Result<Event, anyhow::Error> {
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
            
            event.extract_core_fields();
            Ok(event)
        } else {
            Err(anyhow::anyhow!("Expected JSON object, got: {}", json_value))
        }
    }
}

// Line Parser
pub struct LineParser;

impl LineParser {
    pub fn new() -> Self {
        Self
    }
}

impl Parser for LineParser {
    fn parse(&self, line: &str) -> Result<Event, anyhow::Error> {
        // Create event with minimal capacity (just the line field)
        let mut event = Event::with_capacity(line.to_string(), 1);
        
        // Set the line as a field so it's available as event["line"]
        event.set_field("line".to_string(), Dynamic::from(line.to_string()));
        
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonl_parser_basic() {
        let parser = JsonlParser::new();
        let result = parser
            .parse(r#"{"level":"info","message":"test","count":42}"#)
            .unwrap();

        assert_eq!(result.level, Some("info".to_string()));
        assert_eq!(result.message, Some("test".to_string()));
        assert!(result.fields.get("count").is_some());
        assert_eq!(result.fields.get("count").unwrap().as_int().unwrap(), 42);
    }

    #[test]
    fn test_jsonl_parser_complex() {
        let parser = JsonlParser::new();
        let result = parser
            .parse(r#"{"timestamp":"2023-01-01T12:00:00Z","level":"error","user":"alice","status":404}"#)
            .unwrap();

        assert_eq!(result.level, Some("error".to_string()));
        assert!(result.fields.get("user").is_some());
        assert_eq!(result.fields.get("user").unwrap().clone().into_string().unwrap(), "alice");
        assert!(result.fields.get("status").is_some()); 
        assert_eq!(result.fields.get("status").unwrap().as_int().unwrap(), 404);
    }

    #[test]
    fn test_line_parser_basic() {
        let parser = LineParser::new();
        let test_line = "This is a simple log line";
        let result = parser.parse(test_line).unwrap();

        // Should have the line available as a field
        assert!(result.fields.get("line").is_some());
        assert_eq!(result.fields.get("line").unwrap().clone().into_string().unwrap(), test_line);
        
        // Original line should also be preserved
        assert_eq!(result.original_line, test_line);
        
        // No core fields should be extracted from plain text
        assert_eq!(result.level, None);
        assert_eq!(result.message, None);
        assert_eq!(result.timestamp, None);
    }

    #[test]
    fn test_line_parser_with_structure() {
        let parser = LineParser::new();
        let test_line = "2023-01-01 ERROR Failed to connect";
        let result = parser.parse(test_line).unwrap();

        // Should have the line available as a field
        assert!(result.fields.get("line").is_some());
        assert_eq!(result.fields.get("line").unwrap().clone().into_string().unwrap(), test_line);
        
        // Original line should be preserved
        assert_eq!(result.original_line, test_line);
        
        // Line parser doesn't extract core fields
        assert_eq!(result.level, None);
        assert_eq!(result.message, None);
    }
}