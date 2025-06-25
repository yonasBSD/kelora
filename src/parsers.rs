use crate::event::Event;
use anyhow::{Context, Result};

pub trait Parser {
    fn parse(&self, line: &str) -> Result<Event, anyhow::Error>;
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

        let mut event = Event {
            original_line: line.to_string(),
            ..Default::default()
        };

        if let serde_json::Value::Object(map) = json_value {
            for (key, value) in map {
                event.set_field(key, value);
            }
        } else {
            return Err(anyhow::anyhow!("Expected JSON object, got: {}", json_value));
        }

        event.extract_core_fields();
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
        assert_eq!(result.fields.get("count"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_jsonl_parser_complex() {
        let parser = JsonlParser::new();
        let result = parser
            .parse(r#"{"timestamp":"2023-01-01T12:00:00Z","level":"error","user":"alice","status":404}"#)
            .unwrap();

        assert_eq!(result.level, Some("error".to_string()));
        assert_eq!(result.fields.get("user"), Some(&serde_json::json!("alice")));
        assert_eq!(result.fields.get("status"), Some(&serde_json::json!(404)));
    }
}