use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::Result;
use rhai::Dynamic;

pub struct RawParser;

impl RawParser {
    pub fn new() -> Self {
        Self
    }
}

impl EventParser for RawParser {
    fn parse(&self, line: &str) -> Result<Event> {
        // Preserve the line exactly as-is, including any trailing newlines or backslashes
        // Create event with minimal capacity (just the raw field)
        let mut event = Event::with_capacity(line.to_string(), 1);

        // Set the raw as a field so it's available as event["raw"]
        // This preserves ALL text artifacts: newlines, backslashes, etc.
        event.set_field("raw".to_string(), Dynamic::from(line.to_string()));

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::EventParser;

    #[test]
    fn test_raw_parser_basic() {
        let parser = RawParser::new();
        let test_line = "This is a simple log line";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should have the raw field available
        assert!(result.fields.get("raw").is_some());
        assert_eq!(
            result
                .fields
                .get("raw")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            test_line
        );

        // Original line should also be preserved
        assert_eq!(result.original_line, test_line);

        // No timestamp should be extracted from plain text
        assert_eq!(result.parsed_ts, None);
    }

    #[test]
    fn test_raw_parser_preserves_newlines() {
        let parser = RawParser::new();
        let test_line = "Line with newline\n";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should preserve the newline exactly
        assert_eq!(
            result
                .fields
                .get("raw")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Line with newline\n"
        );

        // Original line should be preserved
        assert_eq!(result.original_line, test_line);
    }

    #[test]
    fn test_raw_parser_preserves_backslashes() {
        let parser = RawParser::new();
        let test_line = "Line with backslash\\\nand continuation";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should preserve backslashes and newlines exactly
        assert_eq!(
            result
                .fields
                .get("raw")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Line with backslash\\\nand continuation"
        );

        // Original line should be preserved
        assert_eq!(result.original_line, test_line);
    }

    #[test]
    fn test_raw_parser_preserves_carriage_returns() {
        let parser = RawParser::new();
        let test_line = "Line with carriage return\r\n";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should preserve CRLF exactly
        assert_eq!(
            result
                .fields
                .get("raw")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Line with carriage return\r\n"
        );

        // Original line should be preserved
        assert_eq!(result.original_line, test_line);
    }
}
