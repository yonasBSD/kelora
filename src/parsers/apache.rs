use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::{Context, Result};
use regex::Regex;
use rhai::Dynamic;

pub struct ApacheParser {
    combined_regex: Regex,
    common_regex: Regex,
}

impl ApacheParser {
    pub fn new() -> Result<Self> {
        // Apache Combined Log Format pattern
        // Example: 192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234 "http://www.example.com/" "Mozilla/4.08"
        let combined_regex = Regex::new(
            r#"^(\S+) (\S+) (\S+) \[([^\]]+)\] "([^"]*)" (\d+) (\S+)(?: "([^"]*)" "([^"]*)")?$"#,
        )
        .context("Failed to compile Apache Combined Log Format regex")?;

        // Apache Common Log Format pattern
        // Example: 192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234
        let common_regex = Regex::new(r#"^(\S+) (\S+) (\S+) \[([^\]]+)\] "([^"]*)" (\d+) (\S+)$"#)
            .context("Failed to compile Apache Common Log Format regex")?;

        Ok(Self {
            combined_regex,
            common_regex,
        })
    }

    /// Parse HTTP request string into method, path, and protocol
    fn parse_request(request: &str, event: &mut Event) {
        let parts: Vec<&str> = request.splitn(3, ' ').collect();
        if let Some(method) = parts.first() {
            event.set_field("method".to_string(), Dynamic::from(method.to_string()));
        }
        if let Some(path) = parts.get(1) {
            event.set_field("path".to_string(), Dynamic::from(path.to_string()));
        }
        if let Some(protocol) = parts.get(2) {
            event.set_field("protocol".to_string(), Dynamic::from(protocol.to_string()));
        }
    }

    /// Try to parse as Combined Log Format first
    fn try_parse_combined(&self, line: &str) -> Option<Event> {
        if let Some(captures) = self.combined_regex.captures(line) {
            let mut event = Event::with_capacity(line.to_string(), 12);

            // IP address
            if let Some(ip) = captures.get(1) {
                event.set_field("ip".to_string(), Dynamic::from(ip.as_str().to_string()));
            }

            // Identity (usually -)
            if let Some(identity) = captures.get(2) {
                let identity_str = identity.as_str();
                if identity_str != "-" {
                    event.set_field(
                        "identity".to_string(),
                        Dynamic::from(identity_str.to_string()),
                    );
                }
            }

            // User (usually -)
            if let Some(user) = captures.get(3) {
                let user_str = user.as_str();
                if user_str != "-" {
                    event.set_field("user".to_string(), Dynamic::from(user_str.to_string()));
                }
            }

            // Timestamp
            if let Some(timestamp) = captures.get(4) {
                event.set_field(
                    "timestamp".to_string(),
                    Dynamic::from(timestamp.as_str().to_string()),
                );
            }

            // Request
            if let Some(request) = captures.get(5) {
                let request_str = request.as_str();
                event.set_field(
                    "request".to_string(),
                    Dynamic::from(request_str.to_string()),
                );
                Self::parse_request(request_str, &mut event);
            }

            // Status code
            if let Some(status) = captures.get(6) {
                if let Ok(status_code) = status.as_str().parse::<i64>() {
                    event.set_field("status".to_string(), Dynamic::from(status_code));
                }
            }

            // Bytes
            if let Some(bytes) = captures.get(7) {
                let bytes_str = bytes.as_str();
                if bytes_str != "-" {
                    if let Ok(bytes_num) = bytes_str.parse::<i64>() {
                        event.set_field("bytes".to_string(), Dynamic::from(bytes_num));
                    }
                }
            }

            // Referer (Combined format only)
            if let Some(referer) = captures.get(8) {
                let referer_str = referer.as_str();
                if referer_str != "-" {
                    event.set_field(
                        "referer".to_string(),
                        Dynamic::from(referer_str.to_string()),
                    );
                }
            }

            // User agent (Combined format only)
            if let Some(user_agent) = captures.get(9) {
                let ua_str = user_agent.as_str();
                if ua_str != "-" {
                    event.set_field("user_agent".to_string(), Dynamic::from(ua_str.to_string()));
                }
            }

            event.extract_timestamp();
            Some(event)
        } else {
            None
        }
    }

    /// Try to parse as Common Log Format
    fn try_parse_common(&self, line: &str) -> Option<Event> {
        if let Some(captures) = self.common_regex.captures(line) {
            let mut event = Event::with_capacity(line.to_string(), 10);

            // IP address
            if let Some(ip) = captures.get(1) {
                event.set_field("ip".to_string(), Dynamic::from(ip.as_str().to_string()));
            }

            // Identity (usually -)
            if let Some(identity) = captures.get(2) {
                let identity_str = identity.as_str();
                if identity_str != "-" {
                    event.set_field(
                        "identity".to_string(),
                        Dynamic::from(identity_str.to_string()),
                    );
                }
            }

            // User (usually -)
            if let Some(user) = captures.get(3) {
                let user_str = user.as_str();
                if user_str != "-" {
                    event.set_field("user".to_string(), Dynamic::from(user_str.to_string()));
                }
            }

            // Timestamp
            if let Some(timestamp) = captures.get(4) {
                event.set_field(
                    "timestamp".to_string(),
                    Dynamic::from(timestamp.as_str().to_string()),
                );
            }

            // Request
            if let Some(request) = captures.get(5) {
                let request_str = request.as_str();
                event.set_field(
                    "request".to_string(),
                    Dynamic::from(request_str.to_string()),
                );
                Self::parse_request(request_str, &mut event);
            }

            // Status code
            if let Some(status) = captures.get(6) {
                if let Ok(status_code) = status.as_str().parse::<i64>() {
                    event.set_field("status".to_string(), Dynamic::from(status_code));
                }
            }

            // Bytes
            if let Some(bytes) = captures.get(7) {
                let bytes_str = bytes.as_str();
                if bytes_str != "-" {
                    if let Ok(bytes_num) = bytes_str.parse::<i64>() {
                        event.set_field("bytes".to_string(), Dynamic::from(bytes_num));
                    }
                }
            }

            event.extract_timestamp();
            Some(event)
        } else {
            None
        }
    }
}

impl EventParser for ApacheParser {
    fn parse(&self, line: &str) -> Result<Event> {
        // Try Combined format first, then Common format
        if let Some(event) = self.try_parse_combined(line) {
            Ok(event)
        } else if let Some(event) = self.try_parse_common(line) {
            Ok(event)
        } else {
            Err(anyhow::anyhow!("Failed to parse Apache log line: {}", line))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::EventParser;

    #[test]
    fn test_apache_combined_format() {
        let parser = ApacheParser::new().unwrap();
        let line = r#"192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234 "http://www.example.com/" "Mozilla/4.08""#;
        let result = EventParser::parse(&parser, line).unwrap();

        assert_eq!(
            result
                .fields
                .get("ip")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "192.168.1.1"
        );
        assert_eq!(
            result
                .fields
                .get("user")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "user"
        );
        assert_eq!(
            result
                .fields
                .get("method")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "GET"
        );
        assert_eq!(
            result
                .fields
                .get("path")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "/index.html"
        );
        assert_eq!(
            result
                .fields
                .get("protocol")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "HTTP/1.0"
        );
        assert_eq!(result.fields.get("status").unwrap().as_int().unwrap(), 200);
        assert_eq!(result.fields.get("bytes").unwrap().as_int().unwrap(), 1234);
        assert_eq!(
            result
                .fields
                .get("referer")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "http://www.example.com/"
        );
        assert_eq!(
            result
                .fields
                .get("user_agent")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Mozilla/4.08"
        );
    }

    #[test]
    fn test_apache_common_format() {
        let parser = ApacheParser::new().unwrap();
        let line = r#"192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234"#;
        let result = EventParser::parse(&parser, line).unwrap();

        assert_eq!(
            result
                .fields
                .get("ip")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "192.168.1.1"
        );
        assert_eq!(
            result
                .fields
                .get("user")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "user"
        );
        assert_eq!(
            result
                .fields
                .get("method")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "GET"
        );
        assert_eq!(
            result
                .fields
                .get("path")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "/index.html"
        );
        assert_eq!(
            result
                .fields
                .get("protocol")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "HTTP/1.0"
        );
        assert_eq!(result.fields.get("status").unwrap().as_int().unwrap(), 200);
        assert_eq!(result.fields.get("bytes").unwrap().as_int().unwrap(), 1234);
        assert!(result.fields.get("referer").is_none());
        assert!(result.fields.get("user_agent").is_none());
    }

    #[test]
    fn test_apache_with_dashes() {
        let parser = ApacheParser::new().unwrap();
        let line = r#"127.0.0.1 - - [25/Dec/1995:10:00:00 +0000] "GET / HTTP/1.0" 200 -"#;
        let result = EventParser::parse(&parser, line).unwrap();

        assert_eq!(
            result
                .fields
                .get("ip")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "127.0.0.1"
        );
        assert!(result.fields.get("identity").is_none());
        assert!(result.fields.get("user").is_none());
        assert_eq!(
            result
                .fields
                .get("method")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "GET"
        );
        assert_eq!(
            result
                .fields
                .get("path")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "/"
        );
        assert_eq!(result.fields.get("status").unwrap().as_int().unwrap(), 200);
        assert!(result.fields.get("bytes").is_none());
    }

    #[test]
    fn test_apache_invalid_format() {
        let parser = ApacheParser::new().unwrap();
        let line = "This is not an Apache log line";
        assert!(EventParser::parse(&parser, line).is_err());
    }
}
