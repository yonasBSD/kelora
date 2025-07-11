#![allow(dead_code)]
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rhai::Dynamic;
use serde::{Deserialize, Serialize};

/// Core field name constants to ensure consistency across the codebase
pub const TIMESTAMP_FIELD_NAMES: &[&str] = &[
    "ts",
    "_ts",
    "timestamp",
    "at",
    "time",
    "@timestamp",
    "log_timestamp",
    "event_time",
    "datetime",
    "date_time",
    "created_at",
    "logged_at",
    "_t",
    "@t",
    "t",
];

pub const LEVEL_FIELD_NAMES: &[&str] = &[
    "level",
    "lvl",
    "severity",
    "log_level",
    "loglevel",
    "priority",
    "sev",
    "@level",
    "log_severity",
    "error_level",
    "event_level",
    "_level",
    "@l",
];

pub const MESSAGE_FIELD_NAMES: &[&str] = &[
    "msg",
    "message",
    "content",
    "data",
    "log",
    "text",
    "description",
    "details",
    "body",
    "payload",
    "event_message",
    "log_message",
    "_message",
    "@message",
    "@m",
];

#[derive(Debug, Clone, Default)]
pub struct Event {
    pub fields: IndexMap<String, Dynamic>,
    pub original_line: String,
    pub line_number: Option<usize>,
    pub filename: Option<String>,
    /// Parsed timestamp field for efficient timestamp operations
    /// This is populated automatically when timestamps are extracted from fields
    pub parsed_ts: Option<DateTime<Utc>>,
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
            fields: IndexMap::with_capacity(capacity),
            original_line,
            line_number: None,
            filename: None,
            parsed_ts: None,
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

    /// Try to parse and extract timestamp from the fields map
    pub fn extract_timestamp(&mut self) {
        self.extract_timestamp_with_parser(None);
    }

    /// Try to parse and extract timestamp from the fields map with optional adaptive parser
    pub fn extract_timestamp_with_parser(
        &mut self,
        parser: Option<&mut crate::timestamp::AdaptiveTsParser>,
    ) {
        self.extract_timestamp_with_config(parser, &crate::timestamp::TsConfig::default());
    }

    /// Extract timestamp with configuration
    pub fn extract_timestamp_with_config(
        &mut self,
        parser: Option<&mut crate::timestamp::AdaptiveTsParser>,
        ts_config: &crate::timestamp::TsConfig,
    ) {
        // Extract and parse timestamp with comprehensive field recognition
        if self.parsed_ts.is_none() {
            if let Some((_field_name, ts_str)) =
                crate::timestamp::identify_timestamp_field(&self.fields, ts_config)
            {
                let parsed_ts = if let Some(parser) = parser {
                    parser.parse_ts(&ts_str)
                } else {
                    // Use the enhanced adaptive parser as default
                    let mut default_parser = crate::timestamp::AdaptiveTsParser::new();
                    default_parser.parse_ts(&ts_str)
                };

                if let Some(ts) = parsed_ts {
                    self.parsed_ts = Some(ts);
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
