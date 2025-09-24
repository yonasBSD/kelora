use crate::event::Event;
use crate::pipeline::EventParser;
use crate::rhai_functions::columns::{parse_cols_whitespace, parse_cols_with_sep, set_parse_cols_strict};
use anyhow::Result;

/// Parser for column-based text input using parse_cols spec
pub struct ColsParser {
    spec: String,
    separator: Option<String>,
}

impl ColsParser {
    pub fn new(spec: String, separator: Option<String>) -> Self {
        Self { spec, separator }
    }
}

impl EventParser for ColsParser {
    fn parse(&self, line: &str) -> Result<Event> {
        // Set strict mode to false for resilient parsing
        set_parse_cols_strict(false);

        let result = if let Some(ref sep) = self.separator {
            parse_cols_with_sep(line, &self.spec, sep)
        } else {
            parse_cols_whitespace(line, &self.spec)
        };

        match result {
            Ok(map) => {
                let mut event = Event::default_with_line(line.to_string());
                for (key, value) in map {
                    // Insert Rhai Dynamic values directly into event
                    event.fields.insert(key.to_string(), value);
                }
                Ok(event)
            }
            Err(err) => {
                // Return error for the parser to handle according to strict/resilient mode
                Err(anyhow::anyhow!("parse_cols error: {}", err))
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cols_parser_whitespace() {
        let parser = ColsParser::new("ts(2) level *msg".to_string(), None);
        let result = parser.parse("2025-09-22 12:33:44 INFO hello world").unwrap();

        assert_eq!(result.fields.get("ts").unwrap().clone().into_string().unwrap(), "2025-09-22 12:33:44");
        assert_eq!(result.fields.get("level").unwrap().clone().into_string().unwrap(), "INFO");
        assert_eq!(result.fields.get("msg").unwrap().clone().into_string().unwrap(), "hello world");
    }

    #[test]
    fn test_cols_parser_with_separator() {
        let parser = ColsParser::new("ts(2) level *msg".to_string(), Some("|".to_string()));
        let result = parser.parse("2025-09-22|12:33:44|INFO|hello|world").unwrap();

        assert_eq!(result.fields.get("ts").unwrap().clone().into_string().unwrap(), "2025-09-22|12:33:44");
        assert_eq!(result.fields.get("level").unwrap().clone().into_string().unwrap(), "INFO");
        assert_eq!(result.fields.get("msg").unwrap().clone().into_string().unwrap(), "hello|world");
    }

    #[test]
    fn test_cols_parser_shortage() {
        let parser = ColsParser::new("ts level user action".to_string(), None);
        let result = parser.parse("2025-09-22 INFO alice").unwrap();

        assert_eq!(result.fields.get("ts").unwrap().clone().into_string().unwrap(), "2025-09-22");
        assert_eq!(result.fields.get("level").unwrap().clone().into_string().unwrap(), "INFO");
        assert_eq!(result.fields.get("user").unwrap().clone().into_string().unwrap(), "alice");
        assert!(result.fields.get("action").unwrap().is_unit()); // Unit type for missing fields
    }
}