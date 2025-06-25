use crate::event::Event;

pub trait Formatter {
    fn format(&self, event: &Event) -> String;
}

// JSON formatter
pub struct JsonFormatter;

impl JsonFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Formatter for JsonFormatter {
    fn format(&self, event: &Event) -> String {
        let mut json_obj = serde_json::Map::new();

        // Add core fields if they exist
        if let Some(timestamp) = &event.timestamp {
            json_obj.insert(
                "timestamp".to_string(),
                serde_json::Value::String(timestamp.to_rfc3339()),
            );
        }

        if let Some(level) = &event.level {
            json_obj.insert(
                "level".to_string(),
                serde_json::Value::String(level.clone()),
            );
        }

        if let Some(message) = &event.message {
            json_obj.insert(
                "message".to_string(),
                serde_json::Value::String(message.clone()),
            );
        }

        // Add all other fields
        for (key, value) in &event.fields {
            json_obj.insert(key.clone(), value.clone());
        }

        serde_json::to_string(&serde_json::Value::Object(json_obj))
            .unwrap_or_else(|_| "{}".to_string())
    }
}

// Text formatter (logfmt-style)
pub struct TextFormatter;

impl TextFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Formatter for TextFormatter {
    fn format(&self, event: &Event) -> String {
        let mut parts = Vec::new();

        // Add core fields first if they exist
        if let Some(timestamp) = &event.timestamp {
            parts.push(format!(
                "timestamp=\"{}\"",
                timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ")
            ));
        }

        if let Some(level) = &event.level {
            parts.push(format!("level=\"{}\"", level));
        }

        if let Some(message) = &event.message {
            parts.push(format!("message=\"{}\"", escape_quotes(message)));
        }

        // Add other fields in sorted order
        let mut field_keys: Vec<_> = event.fields.keys().collect();
        field_keys.sort();

        for key in field_keys {
            if let Some(value) = event.fields.get(key) {
                let formatted_value = format_json_value(value);
                parts.push(format!("{}={}", key, formatted_value));
            }
        }

        parts.join(" ")
    }
}

fn format_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => format!("\"{}\"", escape_quotes(s)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                f.to_string()
            } else {
                n.to_string()
            }
        }
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => format!("\"{}\"", escape_quotes(&value.to_string())),
    }
}

fn escape_quotes(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_json_formatter_empty_event() {
        let event = Event::default();
        let formatter = JsonFormatter::new();
        let result = formatter.format(&event);
        assert!(result.starts_with('{') && result.ends_with('}'));
    }

    #[test]
    fn test_json_formatter_with_fields() {
        let mut event = Event::default();
        event.level = Some("INFO".to_string());
        event.message = Some("Test message".to_string());
        event.set_field("user".to_string(), serde_json::json!("alice"));
        event.set_field("status".to_string(), serde_json::json!(200));

        let formatter = JsonFormatter::new();
        let result = formatter.format(&event);

        assert!(result.contains("\"level\":\"INFO\""));
        assert!(result.contains("\"message\":\"Test message\""));
        assert!(result.contains("\"user\":\"alice\""));
        assert!(result.contains("\"status\":200"));
    }

    #[test]
    fn test_text_formatter() {
        let mut event = Event::default();
        event.level = Some("INFO".to_string());
        event.set_field("user".to_string(), serde_json::json!("alice"));
        event.set_field("count".to_string(), serde_json::json!(42));

        let formatter = TextFormatter::new();
        let result = formatter.format(&event);

        assert!(result.contains("level=\"INFO\""));
        assert!(result.contains("user=\"alice\""));
        assert!(result.contains("count=42"));
    }

    #[test]
    fn test_escape_quotes() {
        assert_eq!(escape_quotes("hello"), "hello");
        assert_eq!(escape_quotes("hello \"world\""), "hello \\\"world\\\"");
        assert_eq!(escape_quotes("path\\to\\file"), "path\\\\to\\\\file");
    }
}