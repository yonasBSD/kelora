#![allow(dead_code)]
use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::Result;
use rhai::Dynamic;

pub struct DockerParser;

impl DockerParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse Docker Compose logs with optional source prefix and timestamp
    /// Format variants:
    /// 1. "web_1    | 2024-07-27T12:34:56.123456789Z GET /health 200"
    /// 2. "db_1     | Connection established"  
    /// 3. "2024-07-27T12:34:56Z GET /api"
    /// 4. "Started app in 3.1s"
    fn parse_docker_line(&self, line: &str) -> Result<Event> {
        let line = line.trim();

        // Split on first | to check for Compose prefix
        let (source, payload) = if let Some(pipe_pos) = line.find('|') {
            let source = line[..pipe_pos].trim();
            let payload = line[pipe_pos + 1..].trim();
            (Some(source), payload)
        } else {
            (None, line)
        };

        // Try to extract timestamp from start of payload
        let (ts_str, msg) = Self::extract_timestamp_and_message(payload);

        // Create event with appropriate capacity
        let capacity = if source.is_some() && ts_str.is_some() {
            3
        } else if source.is_some() || ts_str.is_some() {
            2
        } else {
            1
        };
        let mut event = Event::with_capacity(line.to_string(), capacity);

        // Set required msg field
        event.set_field("msg".to_string(), Dynamic::from(msg.to_string()));

        // Set optional src field (from Compose prefix)
        if let Some(source_name) = source {
            if !source_name.is_empty() {
                event.set_field("src".to_string(), Dynamic::from(source_name.to_string()));
            }
        }

        // Set optional ts field if found
        if let Some(timestamp_str) = ts_str {
            event.set_field("ts".to_string(), Dynamic::from(timestamp_str.to_string()));
        }

        // Let the event extract and parse the timestamp
        event.extract_timestamp();

        Ok(event)
    }

    /// Extract timestamp string from beginning of payload if present
    /// Returns (timestamp_str, remaining_message)
    fn extract_timestamp_and_message(payload: &str) -> (Option<&str>, &str) {
        let payload = payload.trim();

        // Look for space that separates timestamp from message
        if let Some(space_pos) = payload.find(' ') {
            let potential_ts = &payload[..space_pos];
            let remaining = payload[space_pos..].trim();

            // Check if the first part looks like a timestamp
            // Docker timestamps are typically ISO8601/RFC3339 format
            if Self::looks_like_timestamp(potential_ts) {
                return (Some(potential_ts), remaining);
            }
        }

        // Check if entire payload is a timestamp (no message part)
        if Self::looks_like_timestamp(payload) {
            return (Some(payload), "");
        }

        // No timestamp found at beginning
        (None, payload)
    }

    /// Check if a string looks like a Docker timestamp
    /// Recognizes common Docker timestamp patterns:
    /// - 2024-07-27T12:34:56Z
    /// - 2024-07-27T12:34:56.123Z  
    /// - 2024-07-27T12:34:56.123456789Z
    fn looks_like_timestamp(s: &str) -> bool {
        // Simple heuristic: starts with year and contains T and Z (RFC3339/ISO8601)
        s.len() >= 19 &&
        s.starts_with("20") &&  // Years 2000-2099
        s.contains('T') &&
        (s.ends_with('Z') || s.contains('+') || s.contains('-'))
    }
}

impl EventParser for DockerParser {
    fn parse(&self, line: &str) -> Result<Event> {
        self.parse_docker_line(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::EventParser;

    #[test]
    fn test_compose_with_timestamp() {
        let parser = DockerParser::new();
        let line = "web_1    | 2024-07-27T12:34:56.123456789Z GET /health 200";
        let result = parser.parse(line).unwrap();

        assert_eq!(
            result
                .fields
                .get("src")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "web_1"
        );
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "GET /health 200"
        );
        assert!(result.fields.get("ts").is_some());
        assert!(result.parsed_ts.is_some());
    }

    #[test]
    fn test_compose_without_timestamp() {
        let parser = DockerParser::new();
        let line = "db_1     | Connection established";
        let result = parser.parse(line).unwrap();

        assert_eq!(
            result
                .fields
                .get("src")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "db_1"
        );
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Connection established"
        );
        assert!(result.fields.get("ts").is_none());
        assert!(result.parsed_ts.is_none());
    }

    #[test]
    fn test_raw_docker_with_timestamp() {
        let parser = DockerParser::new();
        let line = "2024-07-27T12:34:56Z GET /api";
        let result = parser.parse(line).unwrap();

        assert!(result.fields.get("src").is_none());
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "GET /api"
        );
        assert!(result.fields.get("ts").is_some());
        assert!(result.parsed_ts.is_some());
    }

    #[test]
    fn test_raw_docker_without_timestamp() {
        let parser = DockerParser::new();
        let line = "Started app in 3.1s";
        let result = parser.parse(line).unwrap();

        assert!(result.fields.get("src").is_none());
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Started app in 3.1s"
        );
        assert!(result.fields.get("ts").is_none());
        assert!(result.parsed_ts.is_none());
    }

    #[test]
    fn test_empty_source_handling() {
        let parser = DockerParser::new();
        let line = " | Just a message";
        let result = parser.parse(line).unwrap();

        assert!(result.fields.get("src").is_none());
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Just a message"
        );
    }

    #[test]
    fn test_timestamp_only_line() {
        let parser = DockerParser::new();
        let line = "2024-07-27T12:34:56Z";
        let result = parser.parse(line).unwrap();

        assert!(result.fields.get("src").is_none());
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            ""
        );
        assert!(result.fields.get("ts").is_some());
        assert!(result.parsed_ts.is_some());
    }
}
