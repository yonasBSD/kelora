use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::{Context, Result};
use rhai::Dynamic;
use regex::Regex;


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

impl EventParser for JsonlParser {
    fn parse(&self, line: &str) -> Result<Event> {
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

impl EventParser for LineParser {
    fn parse(&self, line: &str) -> Result<Event> {
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

impl EventParser for LogfmtParser {
    fn parse(&self, line: &str) -> Result<Event> {
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

// Syslog Parser
pub struct SyslogParser {
    rfc5424_regex: Regex,
    rfc3164_regex: Regex,
}

impl SyslogParser {
    pub fn new() -> Result<Self> {
        let rfc5424_regex = Regex::new(
            r"^<(\d{1,3})>(\d+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)(?:\s+(.*))?$",
        ).context("Failed to compile RFC5424 regex")?;

        let rfc3164_regex = Regex::new(
            r"^(?:<(\d{1,3})>)?(\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})\s+(\S+)\s+([^:\[\s]+)(?:\[(\d+)\])?\s*:\s*(.*)$"
        ).context("Failed to compile RFC3164 regex")?;

        Ok(Self {
            rfc5424_regex,
            rfc3164_regex,
        })
    }

    /// Parse priority value into facility and severity
    fn parse_priority(priority: u32) -> (u32, u32) {
        let facility = priority >> 3;
        let severity = priority & 7;
        (facility, severity)
    }

    /// Try to parse as RFC5424 format first
    fn try_parse_rfc5424(&self, line: &str) -> Option<Event> {
        if let Some(captures) = self.rfc5424_regex.captures(line) {
            let priority_str = captures.get(1)?.as_str();
            let priority: u32 = priority_str.parse().ok()?;
            
            // Validate priority range (0-191)
            if priority > 191 {
                return None;
            }

            let (facility, severity) = Self::parse_priority(priority);
            
            // Pre-allocate with expected field count
            let mut event = Event::with_capacity(line.to_string(), 10);
            
            // Set priority fields
            event.set_field("pri".to_string(), Dynamic::from(priority as i64));
            event.set_field("facility".to_string(), Dynamic::from(facility as i64));
            event.set_field("severity".to_string(), Dynamic::from(severity as i64));
            
            // Set version
            if let Some(version) = captures.get(2) {
                if let Ok(v) = version.as_str().parse::<i64>() {
                    event.set_field("version".to_string(), Dynamic::from(v));
                }
            }
            
            // Set timestamp
            if let Some(ts) = captures.get(3) {
                let ts_str = ts.as_str();
                if ts_str != "-" {
                    event.set_field("timestamp".to_string(), Dynamic::from(ts_str.to_string()));
                }
            }
            
            // Set hostname
            if let Some(host) = captures.get(4) {
                let host_str = host.as_str();
                if host_str != "-" {
                    event.set_field("host".to_string(), Dynamic::from(host_str.to_string()));
                }
            }
            
            // Set program name
            if let Some(prog) = captures.get(5) {
                let prog_str = prog.as_str();
                if prog_str != "-" {
                    event.set_field("prog".to_string(), Dynamic::from(prog_str.to_string()));
                }
            }
            
            // Set process ID
            if let Some(pid) = captures.get(6) {
                let pid_str = pid.as_str();
                if pid_str != "-" {
                    if let Ok(pid_num) = pid_str.parse::<i64>() {
                        event.set_field("pid".to_string(), Dynamic::from(pid_num));
                    } else {
                        event.set_field("pid".to_string(), Dynamic::from(pid_str.to_string()));
                    }
                }
            }
            
            // Set message ID
            if let Some(msgid) = captures.get(7) {
                let msgid_str = msgid.as_str();
                if msgid_str != "-" {
                    event.set_field("msgid".to_string(), Dynamic::from(msgid_str.to_string()));
                }
            }
            
            // Set structured data (skip for now, treat as part of message)
            // if let Some(sd) = captures.get(8) {
            //     let sd_str = sd.as_str();
            //     if sd_str != "-" {
            //         event.set_field("sd".to_string(), Dynamic::from(sd_str.to_string()));
            //     }
            // }
            
            // Set message
            if let Some(msg) = captures.get(9) {
                event.set_field("msg".to_string(), Dynamic::from(msg.as_str().to_string()));
            }
            
            event.extract_core_fields();
            Some(event)
        } else {
            None
        }
    }

    /// Try to parse as RFC3164 format
    fn try_parse_rfc3164(&self, line: &str) -> Option<Event> {
        if let Some(captures) = self.rfc3164_regex.captures(line) {
            // Pre-allocate with expected field count
            let mut event = Event::with_capacity(line.to_string(), 8);
            
            // Set priority fields if present
            if let Some(priority_match) = captures.get(1) {
                let priority: u32 = priority_match.as_str().parse().ok()?;
                
                // Validate priority range (0-191)
                if priority > 191 {
                    return None;
                }

                let (facility, severity) = Self::parse_priority(priority);
                
                event.set_field("pri".to_string(), Dynamic::from(priority as i64));
                event.set_field("facility".to_string(), Dynamic::from(facility as i64));
                event.set_field("severity".to_string(), Dynamic::from(severity as i64));
            }
            
            // Set timestamp (group 2 now since priority is group 1)
            if let Some(ts) = captures.get(2) {
                event.set_field("timestamp".to_string(), Dynamic::from(ts.as_str().to_string()));
            }
            
            // Set hostname (group 3)
            if let Some(host) = captures.get(3) {
                event.set_field("host".to_string(), Dynamic::from(host.as_str().to_string()));
            }
            
            // Set program name (group 4)
            if let Some(prog) = captures.get(4) {
                event.set_field("prog".to_string(), Dynamic::from(prog.as_str().to_string()));
            }
            
            // Set process ID (optional, group 5)
            if let Some(pid) = captures.get(5) {
                if let Ok(pid_num) = pid.as_str().parse::<i64>() {
                    event.set_field("pid".to_string(), Dynamic::from(pid_num));
                } else {
                    event.set_field("pid".to_string(), Dynamic::from(pid.as_str().to_string()));
                }
            }
            
            // Set message (group 6)
            if let Some(msg) = captures.get(6) {
                event.set_field("msg".to_string(), Dynamic::from(msg.as_str().to_string()));
            }
            
            event.extract_core_fields();
            Some(event)
        } else {
            None
        }
    }
}

impl EventParser for SyslogParser {
    fn parse(&self, line: &str) -> Result<Event> {
        // Try RFC5424 first, then RFC3164
        if let Some(event) = self.try_parse_rfc5424(line) {
            Ok(event)
        } else if let Some(event) = self.try_parse_rfc3164(line) {
            Ok(event)
        } else {
            Err(anyhow::anyhow!("Failed to parse syslog line: {}", line))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::EventParser;

    #[test]
    fn test_jsonl_parser_basic() {
        let parser = JsonlParser::new();
        let result = EventParser::parse(&parser, r#"{"level":"info","message":"test","count":42}"#)
            .unwrap();

        assert_eq!(result.level, Some("info".to_string()));
        assert_eq!(result.message, Some("test".to_string()));
        assert!(result.fields.get("count").is_some());
        assert_eq!(result.fields.get("count").unwrap().as_int().unwrap(), 42);
    }

    #[test]
    fn test_jsonl_parser_complex() {
        let parser = JsonlParser::new();
        let result = EventParser::parse(&parser, r#"{"timestamp":"2023-01-01T12:00:00Z","level":"error","user":"alice","status":404}"#)
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
        let result = EventParser::parse(&parser, test_line).unwrap();

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
        let result = EventParser::parse(&parser, test_line).unwrap();

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
        let result = EventParser::parse(&parser, r#"level=info message="test message" count=42"#)
            .unwrap();

        assert_eq!(result.level, Some("info".to_string()));
        assert_eq!(result.message, Some("test message".to_string()));
        assert!(result.fields.get("count").is_some());
        assert_eq!(result.fields.get("count").unwrap().as_int().unwrap(), 42);
    }

    #[test]
    fn test_logfmt_parser_types() {
        let parser = LogfmtParser::new();
        let result = EventParser::parse(&parser, r#"str="hello" int=123 float=3.14 bool_true=true bool_false=false"#)
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
        let result = EventParser::parse(&parser, r#"key1="value with spaces" key2="value with \"quotes\"" key3=simple"#)
            .unwrap();

        assert_eq!(result.fields.get("key1").unwrap().clone().into_string().unwrap(), "value with spaces");
        assert_eq!(result.fields.get("key2").unwrap().clone().into_string().unwrap(), "value with \"quotes\"");
        assert_eq!(result.fields.get("key3").unwrap().clone().into_string().unwrap(), "simple");
    }

    #[test]
    fn test_logfmt_parser_escape_sequences() {
        let parser = LogfmtParser::new();
        let result = EventParser::parse(&parser, r#"newline="line1\nline2" tab="col1\tcol2" backslash="back\\slash""#)
            .unwrap();

        assert_eq!(result.fields.get("newline").unwrap().clone().into_string().unwrap(), "line1\nline2");
        assert_eq!(result.fields.get("tab").unwrap().clone().into_string().unwrap(), "col1\tcol2");
        assert_eq!(result.fields.get("backslash").unwrap().clone().into_string().unwrap(), "back\\slash");
    }

    #[test]
    fn test_logfmt_parser_empty_values() {
        let parser = LogfmtParser::new();
        let result = EventParser::parse(&parser, r#"empty="" quoted_empty="" unquoted_value=value"#)
            .unwrap();

        assert_eq!(result.fields.get("empty").unwrap().clone().into_string().unwrap(), "");
        assert_eq!(result.fields.get("quoted_empty").unwrap().clone().into_string().unwrap(), "");
        assert_eq!(result.fields.get("unquoted_value").unwrap().clone().into_string().unwrap(), "value");
    }

    #[test]
    fn test_logfmt_parser_core_fields() {
        let parser = LogfmtParser::new();
        let result = EventParser::parse(&parser, r#"timestamp=2023-01-01T12:00:00Z level=error message="Connection failed" user=alice"#)
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
        assert!(EventParser::parse(&parser, "key value").is_err());
        
        // Empty key
        assert!(EventParser::parse(&parser, "=value").is_err());
        
        // Key with spaces
        assert!(EventParser::parse(&parser, "key with spaces=value").is_err());
    }

    #[test]
    fn test_syslog_parser_rfc5424() {
        let parser = SyslogParser::new().unwrap();
        let line = "<165>1 2023-10-11T22:14:15.003Z server01 sshd 1234 ID47 - Failed password for user";
        let result = EventParser::parse(&parser, line).unwrap();

        // Check priority parsing
        assert_eq!(result.fields.get("pri").unwrap().as_int().unwrap(), 165);
        assert_eq!(result.fields.get("facility").unwrap().as_int().unwrap(), 20); // 165 >> 3 = 20
        assert_eq!(result.fields.get("severity").unwrap().as_int().unwrap(), 5);  // 165 & 7 = 5
        
        // Check other fields
        assert_eq!(result.fields.get("version").unwrap().as_int().unwrap(), 1);
        assert_eq!(result.fields.get("timestamp").unwrap().clone().into_string().unwrap(), "2023-10-11T22:14:15.003Z");
        assert_eq!(result.fields.get("host").unwrap().clone().into_string().unwrap(), "server01");
        assert_eq!(result.fields.get("prog").unwrap().clone().into_string().unwrap(), "sshd");
        assert_eq!(result.fields.get("pid").unwrap().as_int().unwrap(), 1234);
        assert_eq!(result.fields.get("msgid").unwrap().clone().into_string().unwrap(), "ID47");
        assert_eq!(result.fields.get("msg").unwrap().clone().into_string().unwrap(), "Failed password for user");
    }

    #[test]
    fn test_syslog_parser_rfc3164() {
        let parser = SyslogParser::new().unwrap();
        let line = "Oct 11 22:14:15 server01 sshd[1234]: Failed password for user from 192.168.1.100";
        let result = EventParser::parse(&parser, line).unwrap();

        // Check fields
        assert_eq!(result.fields.get("timestamp").unwrap().clone().into_string().unwrap(), "Oct 11 22:14:15");
        assert_eq!(result.fields.get("host").unwrap().clone().into_string().unwrap(), "server01");
        assert_eq!(result.fields.get("prog").unwrap().clone().into_string().unwrap(), "sshd");
        assert_eq!(result.fields.get("pid").unwrap().as_int().unwrap(), 1234);
        assert_eq!(result.fields.get("msg").unwrap().clone().into_string().unwrap(), "Failed password for user from 192.168.1.100");
    }

    #[test]
    fn test_syslog_parser_rfc3164_no_pid() {
        let parser = SyslogParser::new().unwrap();
        let line = "Oct 11 22:14:15 server01 kernel: CPU0: Core temperature above threshold";
        let result = EventParser::parse(&parser, line).unwrap();

        // Check fields
        assert_eq!(result.fields.get("timestamp").unwrap().clone().into_string().unwrap(), "Oct 11 22:14:15");
        assert_eq!(result.fields.get("host").unwrap().clone().into_string().unwrap(), "server01");
        assert_eq!(result.fields.get("prog").unwrap().clone().into_string().unwrap(), "kernel");
        assert!(result.fields.get("pid").is_none()); // No PID in this format
        assert_eq!(result.fields.get("msg").unwrap().clone().into_string().unwrap(), "CPU0: Core temperature above threshold");
    }

    #[test]
    fn test_syslog_parser_rfc3164_with_priority() {
        let parser = SyslogParser::new().unwrap();
        let line = "<34>Oct 11 22:14:15 webserver nginx: 192.168.1.10 - - [11/Oct/2023:22:14:15 +0000] \"GET /index.html HTTP/1.1\" 200 612";
        let result = EventParser::parse(&parser, line).unwrap();

        // Check priority fields
        assert_eq!(result.fields.get("pri").unwrap().as_int().unwrap(), 34);
        assert_eq!(result.fields.get("facility").unwrap().as_int().unwrap(), 4); // 34 >> 3
        assert_eq!(result.fields.get("severity").unwrap().as_int().unwrap(), 2); // 34 & 7
        
        // Check other fields
        assert_eq!(result.fields.get("timestamp").unwrap().clone().into_string().unwrap(), "Oct 11 22:14:15");
        assert_eq!(result.fields.get("host").unwrap().clone().into_string().unwrap(), "webserver");
        assert_eq!(result.fields.get("prog").unwrap().clone().into_string().unwrap(), "nginx");
        assert!(result.fields.get("pid").is_none()); // No PID in this format
        assert_eq!(result.fields.get("msg").unwrap().clone().into_string().unwrap(), "192.168.1.10 - - [11/Oct/2023:22:14:15 +0000] \"GET /index.html HTTP/1.1\" 200 612");
    }

    #[test]
    fn test_syslog_parser_priority_calculation() {
        let parser = SyslogParser::new().unwrap();
        
        // Test different priority values
        let test_cases = [
            (0, 0, 0),    // kern.emerg
            (33, 4, 1),   // auth.alert  
            (165, 20, 5), // local4.notice
            (191, 23, 7), // local7.debug
        ];
        
        for (priority, expected_facility, expected_severity) in test_cases {
            let line = format!("<{}>1 2023-10-11T22:14:15.003Z server01 test - - - Test message", priority);
            let result = EventParser::parse(&parser, &line).unwrap();
            
            assert_eq!(result.fields.get("pri").unwrap().as_int().unwrap(), priority as i64);
            assert_eq!(result.fields.get("facility").unwrap().as_int().unwrap(), expected_facility);
            assert_eq!(result.fields.get("severity").unwrap().as_int().unwrap(), expected_severity);
        }
    }

    #[test]
    fn test_syslog_parser_invalid_priority() {
        let parser = SyslogParser::new().unwrap();
        let line = "<999>1 2023-10-11T22:14:15.003Z server01 test - - - Test message";
        assert!(EventParser::parse(&parser, line).is_err());
    }

    #[test]
    fn test_syslog_parser_invalid_format() {
        let parser = SyslogParser::new().unwrap();
        let line = "This is not a syslog line";
        assert!(EventParser::parse(&parser, line).is_err());
    }

    #[test]
    fn test_syslog_parser_fallback_to_rfc3164() {
        let parser = SyslogParser::new().unwrap();
        
        // This should fail RFC5424 parsing and fall back to RFC3164
        let line = "Dec 25 14:09:07 server01 httpd: GET /index.html HTTP/1.1";
        let result = EventParser::parse(&parser, line).unwrap();

        assert_eq!(result.fields.get("timestamp").unwrap().clone().into_string().unwrap(), "Dec 25 14:09:07");
        assert_eq!(result.fields.get("host").unwrap().clone().into_string().unwrap(), "server01");
        assert_eq!(result.fields.get("prog").unwrap().clone().into_string().unwrap(), "httpd");
        assert_eq!(result.fields.get("msg").unwrap().clone().into_string().unwrap(), "GET /index.html HTTP/1.1");
    }
}