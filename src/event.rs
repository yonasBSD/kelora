use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use indexmap::IndexMap;
use rhai::Dynamic;

/// Core field name constants to ensure consistency across the codebase
pub const TIMESTAMP_FIELD_NAMES: &[&str] = &[
    "ts", "_ts", "timestamp", "at", "time", "@timestamp",
    "log_timestamp", "event_time", "datetime", "date_time",
    "created_at", "logged_at", "_t", "@t", "t"
];

pub const LEVEL_FIELD_NAMES: &[&str] = &[
    "level", "lvl", "severity", "log_level", "loglevel",
    "priority", "sev", "@level", "log_severity", "error_level",
    "event_level", "_level", "@l"
];

pub const MESSAGE_FIELD_NAMES: &[&str] = &[
    "msg", "message", "content", "data", "log", "text",
    "description", "details", "body", "payload", "event_message",
    "log_message", "_message", "@message", "@m"
];

#[derive(Debug, Clone, Default)]
pub struct Event {
    pub timestamp: Option<DateTime<Utc>>,
    pub level: Option<String>,
    pub message: Option<String>,
    pub fields: IndexMap<String, Dynamic>,
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
            fields: IndexMap::with_capacity(capacity),
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

    pub fn set_field(&mut self, key: String, value: Dynamic) {
        self.fields.insert(key, value);
    }

    pub fn set_metadata(&mut self, line_number: usize, filename: Option<String>) {
        self.line_number = Some(line_number);
        self.filename = filename;
    }

    /// Filter to only show specified keys, keeping only fields that actually exist
    pub fn filter_keys(&mut self, keys: &[String]) {
        let mut new_fields = IndexMap::with_capacity(keys.len());

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
        // Extract and parse timestamp with more comprehensive field recognition
        if self.timestamp.is_none() {
            for ts_key in TIMESTAMP_FIELD_NAMES {
                if let Some(value) = self.fields.get(*ts_key) {
                    if let Ok(ts_str) = value.clone().into_string() {
                        if let Ok(ts) = parse_timestamp(&ts_str) {
                            self.timestamp = Some(ts);
                            break;
                        }
                    }
                }
            }
        }

        // Extract level with comprehensive field recognition
        if self.level.is_none() {
            for level_key in LEVEL_FIELD_NAMES {
                if let Some(value) = self.fields.get(*level_key) {
                    if let Ok(level_str) = value.clone().into_string() {
                        self.level = Some(level_str);
                        break;
                    }
                }
            }
        }

        // Extract message with comprehensive field recognition
        if self.message.is_none() {
            for msg_key in MESSAGE_FIELD_NAMES {
                if let Some(value) = self.fields.get(*msg_key) {
                    if let Ok(msg_str) = value.clone().into_string() {
                        self.message = Some(msg_str);
                        break;
                    }
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
