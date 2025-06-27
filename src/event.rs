use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Event {
    pub timestamp: Option<DateTime<Utc>>,
    pub level: Option<String>,
    pub message: Option<String>,
    pub fields: HashMap<String, serde_json::Value>,
    pub original_line: String,
    pub line_number: Option<usize>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

impl Event {
    pub fn with_capacity(original_line: String, capacity: usize) -> Self {
        Self {
            timestamp: None,
            level: None,
            message: None,
            fields: HashMap::with_capacity(capacity),
            original_line,
            line_number: None,
            filename: None,
        }
    }

    pub fn default_with_line(line: String) -> Self {
        Self {
            original_line: line,
            ..Default::default()
        }
    }

    pub fn set_field(&mut self, key: String, value: serde_json::Value) {
        self.fields.insert(key, value);
    }

    pub fn set_metadata(&mut self, line_number: usize, filename: Option<String>) {
        self.line_number = Some(line_number);
        self.filename = filename;
    }

    /// Filter to only show specified keys, keeping only fields that actually exist
    pub fn filter_keys(&mut self, keys: &[String]) {
        let mut new_fields = HashMap::with_capacity(keys.len());

        // Only include fields that are both requested and exist
        for key in keys {
            if let Some(value) = self.fields.get(key) {
                new_fields.insert(key.clone(), value.clone());
            }
        }

        self.fields = new_fields;
    }

    /// Try to parse and extract core fields from the fields map
    pub fn extract_core_fields(&mut self) {
        // Extract timestamp
        for ts_key in &["timestamp", "ts", "time", "at", "_t", "@t", "t"] {
            if let Some(serde_json::Value::String(ts_str)) = self.fields.get(*ts_key) {
                if let Ok(ts) = parse_timestamp(ts_str) {
                    self.timestamp = Some(ts);
                    break;
                }
            }
        }

        // Extract level
        for level_key in &["level", "log_level", "loglevel", "lvl", "severity", "@l"] {
            if let Some(value) = self.fields.get(*level_key) {
                if let Some(level_str) = value.as_str() {
                    self.level = Some(level_str.to_string());
                    break;
                }
            }
        }

        // Extract message
        for msg_key in &["message", "msg", "@m"] {
            if let Some(value) = self.fields.get(*msg_key) {
                if let Some(msg_str) = value.as_str() {
                    self.message = Some(msg_str.to_string());
                    break;
                }
            }
        }
    }

}

impl std::fmt::Display for FieldValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldValue::String(s) => write!(f, "{}", s),
            FieldValue::Number(n) => write!(f, "{}", n),
            FieldValue::Boolean(b) => write!(f, "{}", b),
            FieldValue::Null => write!(f, "null"),
        }
    }
}

fn parse_timestamp(ts_str: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    // Try common timestamp formats in order of likelihood
    let formats = [
        "%Y-%m-%dT%H:%M:%S%.fZ",   // ISO 8601 with subseconds
        "%Y-%m-%dT%H:%M:%SZ",      // ISO 8601
        "%Y-%m-%dT%H:%M:%S%.f%:z", // ISO 8601 with timezone
        "%Y-%m-%dT%H:%M:%S%:z",    // ISO 8601 with timezone
        "%Y-%m-%d %H:%M:%S%.f",    // Common log format with subseconds
        "%Y-%m-%d %H:%M:%S",       // Common log format
        "%b %d %H:%M:%S",          // Syslog format
    ];

    for format in &formats {
        if let Ok(dt) = DateTime::parse_from_str(ts_str, format) {
            return Ok(dt.with_timezone(&Utc));
        }
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts_str, format) {
            return Ok(dt.and_utc());
        }
    }

    // Return a proper chrono parse error
    chrono::NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%dT%H:%M:%SZ").map(|dt| dt.and_utc())
}
