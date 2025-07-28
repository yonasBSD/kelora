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
        let line = Self::strip_ansi_codes(line.trim());

        // Split on first | to check for Compose prefix
        let (source, payload) = if let Some(pipe_pos) = line.find('|') {
            let source = line[..pipe_pos].trim();
            let payload = line[pipe_pos + 1..].trim();
            (Some(source.to_string()), payload.to_string())
        } else {
            (None, line.clone())
        };

        // Try to extract timestamp from start of payload
        let (ts_str, remaining_msg) = Self::extract_timestamp_and_message(&payload);

        // Try to extract log level from the remaining message
        let (level, msg) = Self::extract_log_level(remaining_msg);

        // Create event with appropriate capacity
        let capacity = if source.is_some() && ts_str.is_some() && level.is_some() {
            4
        } else if [source.is_some(), ts_str.is_some(), level.is_some()]
            .iter()
            .filter(|&&x| x)
            .count()
            == 2
        {
            3
        } else if [source.is_some(), ts_str.is_some(), level.is_some()]
            .iter()
            .any(|&x| x)
        {
            2
        } else {
            1
        };
        let mut event = Event::with_capacity(line.clone(), capacity);

        // Set required msg field
        event.set_field("msg".to_string(), Dynamic::from(msg.to_string()));

        // Set optional src field (from Compose prefix)
        if let Some(source_name) = source {
            if !source_name.is_empty() {
                event.set_field("src".to_string(), Dynamic::from(source_name));
            }
        }

        // Set optional ts field if found
        if let Some(timestamp_str) = ts_str {
            event.set_field("ts".to_string(), Dynamic::from(timestamp_str.to_string()));
        }

        // Set optional level field if found
        if let Some(log_level) = level {
            event.set_field("level".to_string(), Dynamic::from(log_level));
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

    /// Extract log level from message if present
    /// Recognizes common log level patterns at the beginning of messages:
    /// - "INFO: message" -> (Some("INFO"), "message")
    /// - "ERROR: something failed" -> (Some("ERROR"), "something failed")
    /// - "INFO:     INFO     07/28/2025..." -> (Some("INFO"), "INFO     07/28/2025...")
    ///
    /// Returns (level, remaining_message)
    fn extract_log_level(msg: &str) -> (Option<String>, &str) {
        let msg = msg.trim();

        // Look for level followed by colon
        if let Some(colon_pos) = msg.find(':') {
            let potential_level = msg[..colon_pos].trim();
            let remaining = msg[colon_pos + 1..].trim();

            // Check if it looks like a log level (common levels)
            if Self::looks_like_log_level(potential_level) {
                return (Some(potential_level.to_uppercase()), remaining);
            }
        }

        // No level found
        (None, msg)
    }

    /// Check if a string looks like a log level
    /// Recognizes common log levels: DEBUG, INFO, WARN, WARNING, ERROR, FATAL, TRACE
    fn looks_like_log_level(s: &str) -> bool {
        if s.is_empty() || s.len() > 10 {
            return false;
        }

        let upper = s.to_uppercase();
        matches!(
            upper.as_str(),
            "DEBUG"
                | "INFO"
                | "WARN"
                | "WARNING"
                | "ERROR"
                | "FATAL"
                | "TRACE"
                | "ERR"
                | "DBG"
                | "WRN"
        )
    }

    /// Strip ANSI color codes and other escape sequences from a string
    /// This handles common ANSI escape sequences that Docker may include in logs
    fn strip_ansi_codes(input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // Found escape character, look for '[' to start ANSI sequence
                if chars.peek() == Some(&'[') {
                    chars.next(); // consume '['

                    // Skip until we find a letter (which ends the ANSI sequence)
                    while let Some(&next_ch) = chars.peek() {
                        chars.next();
                        if next_ch.is_ascii_alphabetic() {
                            break;
                        }
                    }
                } else {
                    // Not an ANSI sequence, keep the escape character
                    result.push(ch);
                }
            } else {
                result.push(ch);
            }
        }

        result
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

    #[test]
    fn test_ansi_color_stripping() {
        let parser = DockerParser::new();

        // Test with ANSI color codes in message
        let line = "web_1 | 2024-07-27T12:34:56Z \x1b[32mINFO\x1b[0m: Application started";
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
            "Application started"
        );
        assert_eq!(
            result
                .fields
                .get("level")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "INFO"
        );
        assert!(result.fields.get("ts").is_some());
    }

    #[test]
    fn test_ansi_color_stripping_complex() {
        let parser = DockerParser::new();

        // Test with complex ANSI sequences (like your example)
        let line = "docker_compose-background-1 | 2025-07-28T10:14:19.885Z INFO: \x1b[32mtasks.py\x1b[0m:85 check_for_vespa_sync_task started";
        let result = parser.parse(line).unwrap();

        assert_eq!(
            result
                .fields
                .get("src")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "docker_compose-background-1"
        );
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "tasks.py:85 check_for_vespa_sync_task started"
        );
        assert_eq!(
            result
                .fields
                .get("level")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "INFO"
        );
        assert!(result.fields.get("ts").is_some());
    }

    #[test]
    fn test_strip_ansi_codes_unit() {
        // Test the strip function directly
        assert_eq!(DockerParser::strip_ansi_codes("normal text"), "normal text");
        assert_eq!(
            DockerParser::strip_ansi_codes("\x1b[32mgreen\x1b[0m"),
            "green"
        );
        assert_eq!(
            DockerParser::strip_ansi_codes("\x1b[31;1mred bold\x1b[0m"),
            "red bold"
        );
        assert_eq!(
            DockerParser::strip_ansi_codes("prefix \x1b[33myellow\x1b[0m suffix"),
            "prefix yellow suffix"
        );
        assert_eq!(DockerParser::strip_ansi_codes("\x1b[0m"), "");
    }

    #[test]
    fn test_log_level_extraction() {
        let parser = DockerParser::new();

        // Test with INFO level
        let line = "web-1 | 2024-07-27T12:34:56Z INFO: Application started successfully";
        let result = parser.parse(line).unwrap();

        assert_eq!(
            result
                .fields
                .get("level")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "INFO"
        );
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Application started successfully"
        );

        // Test with ERROR level
        let line = "api-1 | 2024-07-27T12:34:56Z ERROR: Database connection failed";
        let result = parser.parse(line).unwrap();

        assert_eq!(
            result
                .fields
                .get("level")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "ERROR"
        );
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Database connection failed"
        );
    }

    #[test]
    fn test_log_level_extraction_your_format() {
        let parser = DockerParser::new();

        // Test with your specific log format
        let line = "docker_compose-background-1 | 2025-07-28T10:14:19.885Z INFO:     INFO     07/28/2025 10:14:19 AM        tasks.py:85";
        let result = parser.parse(line).unwrap();

        assert_eq!(
            result
                .fields
                .get("level")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "INFO"
        );
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "INFO     07/28/2025 10:14:19 AM        tasks.py:85"
        );

        // Test with WARNING level
        let line = "docker_compose-background-1 | 2025-07-28T10:14:24.438Z WARNING:  WARNING  07/28/2025 10:14:24 AM        tasks.py:397";
        let result = parser.parse(line).unwrap();

        assert_eq!(
            result
                .fields
                .get("level")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "WARNING"
        );
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "WARNING  07/28/2025 10:14:24 AM        tasks.py:397"
        );
    }

    #[test]
    fn test_no_log_level_extraction() {
        let parser = DockerParser::new();

        // Test without log level
        let line = "web-1 | 2024-07-27T12:34:56Z Starting application on port 8080";
        let result = parser.parse(line).unwrap();

        assert!(result.fields.get("level").is_none());
        assert_eq!(
            result
                .fields
                .get("msg")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Starting application on port 8080"
        );
    }

    #[test]
    fn test_extract_log_level_unit() {
        // Test the extract function directly
        assert_eq!(
            DockerParser::extract_log_level("INFO: message"),
            (Some("INFO".to_string()), "message")
        );
        assert_eq!(
            DockerParser::extract_log_level("ERROR: failed"),
            (Some("ERROR".to_string()), "failed")
        );
        assert_eq!(
            DockerParser::extract_log_level("debug: trace info"),
            (Some("DEBUG".to_string()), "trace info")
        );
        assert_eq!(
            DockerParser::extract_log_level("Just a message"),
            (None, "Just a message")
        );
        assert_eq!(
            DockerParser::extract_log_level("NOT_A_LEVEL: message"),
            (None, "NOT_A_LEVEL: message")
        );

        // Test edge cases
        assert_eq!(DockerParser::extract_log_level(""), (None, ""));
        assert_eq!(
            DockerParser::extract_log_level("WARN:"),
            (Some("WARN".to_string()), "")
        );
    }
}
