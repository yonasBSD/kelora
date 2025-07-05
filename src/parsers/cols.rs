use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::Result;
use rhai::Dynamic;

pub struct ColsParser;

impl ColsParser {
    pub fn new() -> Self {
        Self
    }
}

impl EventParser for ColsParser {
    fn parse(&self, line: &str) -> Result<Event> {
        // Split line on any whitespace and collect into vector
        let fields: Vec<&str> = line.split_whitespace().collect();
        
        // Create event with capacity for the fields plus the original line
        let mut event = Event::with_capacity(line.to_string(), fields.len() + 1);

        // Set the original line as a field so it's available as event["line"]
        event.set_field("line".to_string(), Dynamic::from(line.to_string()));

        // Set numbered columns c1, c2, c3, etc.
        for (i, field) in fields.iter().enumerate() {
            let field_name = format!("c{}", i + 1); // c1, c2, c3, ...
            event.set_field(field_name, Dynamic::from(field.to_string()));
        }

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::EventParser;

    #[test]
    fn test_cols_parser_basic() {
        let parser = ColsParser::new();
        let test_line = "field1 field2 field3";
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

        // Should have numbered columns
        assert_eq!(
            result
                .fields
                .get("c1")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "field1"
        );
        assert_eq!(
            result
                .fields
                .get("c2")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "field2"
        );
        assert_eq!(
            result
                .fields
                .get("c3")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "field3"
        );

        // Original line should be preserved
        assert_eq!(result.original_line, test_line);

        // No core fields should be extracted automatically
        assert_eq!(result.level, None);
        assert_eq!(result.message, None);
        assert_eq!(result.timestamp, None);
    }

    #[test]
    fn test_cols_parser_single_field() {
        let parser = ColsParser::new();
        let test_line = "onlyfield";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should have c1 only
        assert_eq!(
            result
                .fields
                .get("c1")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "onlyfield"
        );

        // c2 should not exist
        assert!(result.fields.get("c2").is_none());
    }

    #[test]
    fn test_cols_parser_empty_line() {
        let parser = ColsParser::new();
        let test_line = "";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should have line field but no c1, c2, etc.
        assert!(result.fields.get("line").is_some());
        assert!(result.fields.get("c1").is_none());
    }

    #[test]
    fn test_cols_parser_multiple_whitespace() {
        let parser = ColsParser::new();
        let test_line = "field1    field2\t\tfield3\n  field4";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should handle multiple whitespace types correctly
        assert_eq!(
            result
                .fields
                .get("c1")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "field1"
        );
        assert_eq!(
            result
                .fields
                .get("c2")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "field2"
        );
        assert_eq!(
            result
                .fields
                .get("c3")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "field3"
        );
        assert_eq!(
            result
                .fields
                .get("c4")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "field4"
        );
    }

    #[test]
    fn test_cols_parser_log_like_data() {
        let parser = ColsParser::new();
        let test_line = "2023-01-01 10:30:00 ERROR database connection_failed timeout=30s";
        let result = EventParser::parse(&parser, test_line).unwrap();

        // Should split into appropriate columns
        assert_eq!(
            result
                .fields
                .get("c1")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "2023-01-01"
        );
        assert_eq!(
            result
                .fields
                .get("c2")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "10:30:00"
        );
        assert_eq!(
            result
                .fields
                .get("c3")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "ERROR"
        );
        assert_eq!(
            result
                .fields
                .get("c4")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "database"
        );
        assert_eq!(
            result
                .fields
                .get("c5")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "connection_failed"
        );
        assert_eq!(
            result
                .fields
                .get("c6")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "timeout=30s"
        );
    }
}