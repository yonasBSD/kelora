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

// Logfmt Parser
pub struct LogfmtParser;

impl LogfmtParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse logfmt line: key1=value1 key2="value with spaces" key3=value3
    /// Adapted from Stelp but converted to work with Kelora's Dynamic system
    fn parse_logfmt_pairs(&self, line: &str) -> Result<Vec<(String, String)>, String> {
        let mut pairs = Vec::new();
        let mut chars = line.chars().peekable();

        while chars.peek().is_some() {
            // Skip whitespace
            while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next();
            }

            if chars.peek().is_none() {
                break;
            }

            // Parse key
            let mut key = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == '=' {
                    break;
                } else if ch == ' ' || ch == '\t' {
                    return Err("Key cannot contain spaces".to_string());
                } else {
                    key.push(chars.next().unwrap());
                }
            }

            if key.is_empty() {
                return Err("Empty key found".to_string());
            }

            // Expect '='
            if chars.next() != Some('=') {
                return Err(format!("Expected '=' after key '{}'", key));
            }

            // Parse value
            let mut value = String::new();
            if chars.peek() == Some(&'"') {
                // Quoted value
                chars.next(); // consume opening quote
                while let Some(ch) = chars.next() {
                    if ch == '"' {
                        // Check for escaped quote
                        if chars.peek() == Some(&'"') {
                            chars.next(); // consume escaped quote
                            value.push('"');
                        } else {
                            break; // end of quoted value
                        }
                    } else if ch == '\\' {
                        // Handle escape sequences
                        if let Some(escaped_ch) = chars.next() {
                            match escaped_ch {
                                'n' => value.push('\n'),
                                't' => value.push('\t'),
                                'r' => value.push('\r'),
                                '\\' => value.push('\\'),
                                '"' => value.push('"'),
                                _ => {
                                    value.push('\\');
                                    value.push(escaped_ch);
                                }
                            }
                        }
                    } else {
                        value.push(ch);
                    }
                }
            } else {
                // Unquoted value - read until space or end
                while let Some(&ch) = chars.peek() {
                    if ch == ' ' || ch == '\t' {
                        break;
                    } else {
                        value.push(chars.next().unwrap());
                    }
                }
            }

            pairs.push((key, value));
        }

        Ok(pairs)
    }

    /// Try to convert string to a numeric Dynamic value, falling back to string
    fn parse_value_to_dynamic(&self, value: String) -> Dynamic {
        // Try integer first
        if let Ok(i) = value.parse::<i64>() {
            return Dynamic::from(i);
        }
        
        // Try float 
        if let Ok(f) = value.parse::<f64>() {
            return Dynamic::from(f);
        }
        
        // Try boolean
        match value.to_lowercase().as_str() {
            "true" => return Dynamic::from(true),
            "false" => return Dynamic::from(false),
            _ => {}
        }
        
        // Default to string
        Dynamic::from(value)
    }
}

impl Parser for LogfmtParser {
    fn parse(&self, line: &str) -> Result<Event, anyhow::Error> {
        let pairs = self.parse_logfmt_pairs(line.trim())
            .map_err(|e| anyhow::anyhow!("Failed to parse logfmt: {}", e))?;

        // Pre-allocate Event with capacity based on number of pairs
        let mut event = Event::with_capacity(line.to_string(), pairs.len());
        
        for (key, value) in pairs {
            // Convert string values to appropriate Dynamic types
            let dynamic_value = self.parse_value_to_dynamic(value);
            event.set_field(key, dynamic_value);
        }
        
        // Extract core fields (level, message, timestamp) from the parsed data
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

    #[test]
    fn test_logfmt_parser_basic() {
        let parser = LogfmtParser::new();
        let result = parser
            .parse(r#"level=info message="test message" count=42"#)
            .unwrap();

        assert_eq!(result.level, Some("info".to_string()));
        assert_eq!(result.message, Some("test message".to_string()));
        assert!(result.fields.get("count").is_some());
        assert_eq!(result.fields.get("count").unwrap().as_int().unwrap(), 42);
    }

    #[test]
    fn test_logfmt_parser_types() {
        let parser = LogfmtParser::new();
        let result = parser
            .parse(r#"str="hello" int=123 float=3.14 bool_true=true bool_false=false"#)
            .unwrap();

        assert_eq!(result.fields.get("str").unwrap().clone().into_string().unwrap(), "hello");
        assert_eq!(result.fields.get("int").unwrap().as_int().unwrap(), 123);
        assert_eq!(result.fields.get("float").unwrap().as_float().unwrap(), 3.14);
        assert_eq!(result.fields.get("bool_true").unwrap().as_bool().unwrap(), true);
        assert_eq!(result.fields.get("bool_false").unwrap().as_bool().unwrap(), false);
    }

    #[test]
    fn test_logfmt_parser_quoted_values() {
        let parser = LogfmtParser::new();
        let result = parser
            .parse(r#"key1="value with spaces" key2="value with \"quotes\"" key3=simple"#)
            .unwrap();

        assert_eq!(result.fields.get("key1").unwrap().clone().into_string().unwrap(), "value with spaces");
        assert_eq!(result.fields.get("key2").unwrap().clone().into_string().unwrap(), "value with \"quotes\"");
        assert_eq!(result.fields.get("key3").unwrap().clone().into_string().unwrap(), "simple");
    }

    #[test]
    fn test_logfmt_parser_escape_sequences() {
        let parser = LogfmtParser::new();
        let result = parser
            .parse(r#"newline="line1\nline2" tab="col1\tcol2" backslash="back\\slash""#)
            .unwrap();

        assert_eq!(result.fields.get("newline").unwrap().clone().into_string().unwrap(), "line1\nline2");
        assert_eq!(result.fields.get("tab").unwrap().clone().into_string().unwrap(), "col1\tcol2");
        assert_eq!(result.fields.get("backslash").unwrap().clone().into_string().unwrap(), "back\\slash");
    }

    #[test]
    fn test_logfmt_parser_empty_values() {
        let parser = LogfmtParser::new();
        let result = parser
            .parse(r#"empty="" quoted_empty="" unquoted_value=value"#)
            .unwrap();

        assert_eq!(result.fields.get("empty").unwrap().clone().into_string().unwrap(), "");
        assert_eq!(result.fields.get("quoted_empty").unwrap().clone().into_string().unwrap(), "");
        assert_eq!(result.fields.get("unquoted_value").unwrap().clone().into_string().unwrap(), "value");
    }

    #[test]
    fn test_logfmt_parser_core_fields() {
        let parser = LogfmtParser::new();
        let result = parser
            .parse(r#"timestamp=2023-01-01T12:00:00Z level=error message="Connection failed" user=alice"#)
            .unwrap();

        // Core fields should be extracted
        assert_eq!(result.level, Some("error".to_string()));
        assert_eq!(result.message, Some("Connection failed".to_string()));
        assert!(result.timestamp.is_some());
        
        // Other fields should be available
        assert_eq!(result.fields.get("user").unwrap().clone().into_string().unwrap(), "alice");
    }

    #[test]
    fn test_logfmt_parser_errors() {
        let parser = LogfmtParser::new();
        
        // Missing equals sign
        assert!(parser.parse("key value").is_err());
        
        // Empty key
        assert!(parser.parse("=value").is_err());
        
        // Key with spaces
        assert!(parser.parse("key with spaces=value").is_err());
    }
}