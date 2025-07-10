use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::Result;
use rhai::Dynamic;

pub struct LineParser;

impl LineParser {
    pub fn new() -> Self {
        Self
    }
}

impl EventParser for LineParser {
    fn parse(&self, line: &str) -> Result<Event> {
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
    use crate::pipeline::EventParser;

    #[test]
    fn test_line_parser_basic() {
        let parser = LineParser::new();
        let test_line = "This is a simple log line";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should have the line available as a field
        assert!(result.fields.get("line").is_some());
        assert_eq!(
            result
                .fields
                .get("line")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            test_line
        );

        // Original line should also be preserved
        assert_eq!(result.original_line, test_line);

        // No core fields should be extracted from plain text
        assert_eq!(result.level, None);
        assert_eq!(result.message, None);
        assert_eq!(result.ts, None);
    }

    #[test]
    fn test_line_parser_with_structure() {
        let parser = LineParser::new();
        let test_line = "2023-01-01 ERROR Failed to connect";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should have the line available as a field
        assert!(result.fields.get("line").is_some());
        assert_eq!(
            result
                .fields
                .get("line")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            test_line
        );

        // Original line should be preserved
        assert_eq!(result.original_line, test_line);

        // Line parser doesn't extract core fields
        assert_eq!(result.level, None);
        assert_eq!(result.message, None);
    }
}
