#![allow(dead_code)]
use crate::colors::ColorScheme;
use crate::event::Event;
use crate::pipeline;
use rhai::Dynamic;
use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;

/// Global header tracking registry for CSV formatters in parallel mode
/// Key format: "{delimiter}_{keys_hash}" for uniqueness across different CSV configurations
static CSV_FORMATTER_HEADER_REGISTRY: Lazy<Mutex<HashMap<String, bool>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[cfg(test)]
use crate::pipeline::Formatter;

/// Utility function for logfmt-compliant string escaping
/// Escapes quotes, backslashes, newlines, tabs, and carriage returns
fn escape_logfmt_string(input: &str) -> String {
    let mut output = String::with_capacity(input.len() + 10); // Some extra space for escapes

    for ch in input.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\t' => output.push_str("\\t"),
            '\r' => output.push_str("\\r"),
            _ => output.push(ch),
        }
    }

    output
}

/// Check if a string value needs to be quoted per logfmt rules
fn needs_logfmt_quoting(value: &str) -> bool {
    // Quote values that contain spaces, tabs, newlines, quotes, equals, or are empty
    value.is_empty()
        || value.contains(' ')
        || value.contains('\t')
        || value.contains('\n')
        || value.contains('\r')
        || value.contains('"')
        || value.contains('=')
}

/// Format a Dynamic value as a string representation suitable for output
/// Returns (string_value, needs_quotes) - the second bool indicates if it should be quoted
fn format_dynamic_value(value: &Dynamic) -> (String, bool) {
    if value.is_string() {
        if let Ok(s) = value.clone().into_string() {
            (s, true) // Strings can potentially need quotes
        } else {
            (value.to_string(), false)
        }
    } else {
        // Numbers, booleans, etc. - never need quotes
        (value.to_string(), false)
    }
}

/// Utility to format a quoted logfmt value into a buffer
fn format_quoted_logfmt_value(value: &str, output: &mut String) {
    if needs_logfmt_quoting(value) {
        output.push('"');
        output.push_str(&escape_logfmt_string(value));
        output.push('"');
    } else {
        output.push_str(value);
    }
}

/// Convert rhai::Dynamic to serde_json::Value recursively
fn dynamic_to_json(value: &Dynamic) -> serde_json::Value {
    if value.is_string() {
        if let Ok(s) = value.clone().into_string() {
            serde_json::Value::String(s)
        } else {
            serde_json::Value::Null
        }
    } else if value.is_int() {
        if let Ok(i) = value.as_int() {
            serde_json::Value::Number(serde_json::Number::from(i))
        } else {
            serde_json::Value::Null
        }
    } else if value.is_float() {
        if let Ok(f) = value.as_float() {
            serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        }
    } else if value.is_bool() {
        if let Ok(b) = value.as_bool() {
            serde_json::Value::Bool(b)
        } else {
            serde_json::Value::Null
        }
    } else if value.is_unit() {
        serde_json::Value::Null
    } else if let Some(arr) = value.clone().try_cast::<rhai::Array>() {
        // Convert Rhai array to JSON array recursively
        let json_array: Vec<serde_json::Value> = arr.iter()
            .map(dynamic_to_json)
            .collect();
        serde_json::Value::Array(json_array)
    } else if let Some(map) = value.clone().try_cast::<rhai::Map>() {
        // Convert Rhai map to JSON object recursively
        let mut json_obj = serde_json::Map::new();
        for (key, val) in map {
            json_obj.insert(key.to_string(), dynamic_to_json(&val));
        }
        serde_json::Value::Object(json_obj)
    } else {
        // For any remaining types, convert to string
        serde_json::Value::String(value.to_string())
    }
}

/// Check if a CSV value needs quoting
fn needs_csv_quoting(value: &str, delimiter: char) -> bool {
    value.is_empty()
        || value.contains(delimiter)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r')
        || value.starts_with(' ')
        || value.ends_with(' ')
}

/// Escape CSV value with proper quoting
fn escape_csv_value(value: &str, delimiter: char) -> String {
    if needs_csv_quoting(value, delimiter) {
        let escaped = value.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        value.to_string()
    }
}

// JSON formatter
pub struct JsonFormatter;

impl JsonFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl pipeline::Formatter for JsonFormatter {
    fn format(&self, event: &Event) -> String {
        // Convert Dynamic values to JSON manually
        let mut json_obj = serde_json::Map::new();

        for (key, value) in &event.fields {
            let json_value = dynamic_to_json(value);
            json_obj.insert(key.clone(), json_value);
        }

        serde_json::to_string(&serde_json::Value::Object(json_obj))
            .unwrap_or_else(|_| "{}".to_string())
    }
}

// Default formatter (logfmt-style with colors and brief mode)
pub struct DefaultFormatter {
    colors: ColorScheme,
    level_keys: Vec<&'static str>,
    brief: bool,
    timestamp_formatting: crate::config::TimestampFormatConfig,
}

impl DefaultFormatter {
    pub fn new(
        use_colors: bool,
        brief: bool,
        timestamp_formatting: crate::config::TimestampFormatConfig,
    ) -> Self {
        Self {
            colors: ColorScheme::new(use_colors),
            level_keys: vec![
                "level",
                "loglevel",
                "log_level",
                "lvl",
                "severity",
                "levelname",
                "@l",
            ],
            brief,
            timestamp_formatting,
        }
    }

    /// Format a Dynamic value directly into buffer for performance (zero-allocation when possible)
    fn format_dynamic_value_into(&self, key: &str, value: &Dynamic, output: &mut String) {
        // Check if this field should be formatted as a timestamp
        if self.should_format_as_timestamp(key) {
            if let Some(formatted_ts) = self.try_format_timestamp(value) {
                // Use timestamp formatting
                if !self.colors.string.is_empty() {
                    output.push_str(self.colors.string);
                }
                output.push('"');
                output.push_str(&escape_logfmt_string(&formatted_ts));
                output.push('"');
                if !self.colors.string.is_empty() {
                    output.push_str(self.colors.reset);
                }
                return;
            }
        }

        // Choose color based on field type and value content
        let color = if self.is_level_field(key) {
            if let Ok(level_str) = value.clone().into_string() {
                self.level_color(&level_str)
            } else {
                self.colors.string
            }
        } else {
            self.colors.string
        };

        // Format value based on type - always quote strings for default formatter
        let (string_val, is_string) = format_dynamic_value(value);
        if is_string {
            // Add opening quote (uncolored)
            output.push('"');
            // Apply color to content only
            if !color.is_empty() {
                output.push_str(color);
            }
            output.push_str(&escape_logfmt_string(&string_val));
            // Reset color before closing quote
            if !color.is_empty() {
                output.push_str(self.colors.reset);
            }
            // Add closing quote (uncolored)
            output.push('"');
        } else {
            // For non-strings, color the entire value
            if !color.is_empty() {
                output.push_str(color);
            }
            output.push_str(&string_val);
            if !color.is_empty() {
                output.push_str(self.colors.reset);
            }
        }
    }

    /// Format a Dynamic value for brief mode (no quotes, just the value with colors)
    fn format_dynamic_value_brief_into(&self, key: &str, value: &Dynamic, output: &mut String) {
        // Check if this field should be formatted as a timestamp
        if self.should_format_as_timestamp(key) {
            if let Some(formatted_ts) = self.try_format_timestamp(value) {
                // Use timestamp formatting (no quotes in brief mode)
                if !self.colors.string.is_empty() {
                    output.push_str(self.colors.string);
                }
                output.push_str(&formatted_ts);
                if !self.colors.string.is_empty() {
                    output.push_str(self.colors.reset);
                }
                return;
            }
        }

        // Choose color based on field type and value content
        let color = if self.is_level_field(key) {
            if let Ok(level_str) = value.clone().into_string() {
                self.level_color(&level_str)
            } else {
                self.colors.string
            }
        } else {
            self.colors.string
        };

        // Apply color
        if !color.is_empty() {
            output.push_str(color);
        }

        // In brief mode, output raw value (no quotes even for strings)
        if let Ok(s) = value.clone().into_string() {
            output.push_str(&s);
        } else {
            output.push_str(&value.to_string());
        }

        // Reset color
        if !color.is_empty() {
            output.push_str(self.colors.reset);
        }
    }

    /// Get appropriate color for log level values
    fn level_color(&self, level: &str) -> &str {
        match level.to_lowercase().as_str() {
            // Bright red for error levels
            "error" | "err" | "fatal" | "panic" | "alert" | "crit" | "critical" | "emerg"
            | "emergency" | "severe" => self.colors.level_error,
            // Bright yellow for warning levels
            "warn" | "warning" => self.colors.level_warn,
            // Bright green for info levels
            "info" | "informational" | "notice" => self.colors.level_info,
            // Bright cyan for debug levels
            "debug" | "finer" | "config" => self.colors.level_debug,
            // Cyan for trace levels
            "trace" | "finest" => self.colors.level_trace,
            // Default to no color for unknown levels
            _ => "",
        }
    }

    /// Check if key is likely a log level field
    fn is_level_field(&self, key: &str) -> bool {
        self.level_keys.iter().any(|&lk| lk == key)
    }

    /// Check if a field should be formatted as a timestamp
    fn should_format_as_timestamp(&self, key: &str) -> bool {
        // Check if this field is explicitly listed in format_fields
        if self
            .timestamp_formatting
            .format_fields
            .contains(&key.to_string())
        {
            return true;
        }

        // Check if auto-formatting is enabled and this is a known timestamp field
        if self.timestamp_formatting.auto_format_all {
            return crate::event::TIMESTAMP_FIELD_NAMES.contains(&key);
        }

        false
    }

    /// Try to format a value as a timestamp, returning formatted string if successful
    fn try_format_timestamp(&self, value: &Dynamic) -> Option<String> {
        use chrono::{DateTime, Local, Utc};

        // First, try if it's already a DateTime value
        if let Some(dt) = value.clone().try_cast::<DateTime<Utc>>() {
            return Some(if self.timestamp_formatting.format_as_utc {
                dt.to_rfc3339()
            } else {
                dt.with_timezone(&Local).to_rfc3339()
            });
        }

        // Otherwise, try to parse it as a string timestamp
        if let Ok(ts_str) = value.clone().into_string() {
            let mut parser = crate::timestamp::AdaptiveTsParser::new();
            if let Some(parsed_dt) = parser.parse_ts(&ts_str) {
                return Some(if self.timestamp_formatting.format_as_utc {
                    parsed_dt.to_rfc3339()
                } else {
                    parsed_dt.with_timezone(&Local).to_rfc3339()
                });
            }
        }

        None
    }
}

impl pipeline::Formatter for DefaultFormatter {
    fn format(&self, event: &Event) -> String {
        if event.fields.is_empty() {
            return String::new();
        }

        // Pre-allocate buffer with estimated capacity
        let estimated_capacity = event.fields.len() * 32;
        let mut output = String::with_capacity(estimated_capacity);
        let mut first = true;

        for (key, value) in &event.fields {
            if !first {
                output.push(' ');
            }
            first = false;

            if self.brief {
                // Brief mode: only values (no keys, no quotes)
                self.format_dynamic_value_brief_into(key, value, &mut output);
            } else {
                // Normal mode: key=value pairs
                // Format key with color
                if !self.colors.key.is_empty() {
                    output.push_str(self.colors.key);
                }
                output.push_str(key);
                if !self.colors.key.is_empty() {
                    output.push_str(self.colors.reset);
                }

                // Add equals sign
                if !self.colors.equals.is_empty() {
                    output.push_str(self.colors.equals);
                }
                output.push('=');
                if !self.colors.equals.is_empty() {
                    output.push_str(self.colors.reset);
                }

                // Add formatted value (with proper quoting and colors)
                self.format_dynamic_value_into(key, value, &mut output);
            }
        }

        output
    }
}

// Hide formatter - suppresses all event output
pub struct HideFormatter;

impl HideFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl pipeline::Formatter for HideFormatter {
    fn format(&self, _event: &Event) -> String {
        String::new()
    }
}

/// Sanitize a field key to ensure logfmt compliance
///
/// The logfmt specification requires keys to:
/// - Not contain whitespace (spaces, tabs, newlines, carriage returns)
/// - Not contain equals signs (=) as they delimit key-value pairs
///
/// This function replaces problematic characters with underscores to maintain
/// field information while ensuring valid logfmt output. This approach:
/// - Preserves all field data (no information loss)
/// - Produces parseable logfmt output
/// - Maintains deterministic key mapping
/// - Handles edge cases gracefully
///
/// # Examples
/// - sanitize_logfmt_key("field with spaces") -> "field_with_spaces"
/// - sanitize_logfmt_key("field=with=equals") -> "field_with_equals"  
/// - sanitize_logfmt_key("normal_field") -> "normal_field"
fn sanitize_logfmt_key(key: &str) -> String {
    key.chars()
        .map(|c| match c {
            ' ' | '\t' | '\n' | '\r' | '=' => '_',
            c => c,
        })
        .collect()
}

// Logfmt formatter - strict logfmt output formatter (no colors, no brief mode)
//
// This formatter produces logfmt-compliant output by:
// 1. Sanitizing field keys to replace invalid characters with underscores
// 2. Properly quoting string values that contain special characters
// 3. Leaving numeric and boolean values unquoted
//
// Key sanitization ensures that output can be parsed by standard logfmt parsers,
// including Kelora's own logfmt input parser.
pub struct LogfmtFormatter;

impl LogfmtFormatter {
    pub fn new() -> Self {
        Self
    }

    /// Format a Dynamic value directly into buffer for performance
    fn format_dynamic_value_into(&self, value: &Dynamic, output: &mut String) {
        let (string_val, is_string) = format_dynamic_value(value);
        if is_string {
            format_quoted_logfmt_value(&string_val, output);
        } else {
            output.push_str(&string_val);
        }
    }
}

impl pipeline::Formatter for LogfmtFormatter {
    fn format(&self, event: &Event) -> String {
        if event.fields.is_empty() {
            return String::new();
        }

        // Pre-allocate buffer with estimated capacity
        let estimated_capacity = event.fields.len() * 32;
        let mut output = String::with_capacity(estimated_capacity);
        let mut first = true;

        for (key, value) in &event.fields {
            if !first {
                output.push(' ');
            }
            first = false;

            // Strict logfmt: key=value pairs with sanitized keys
            // Keys are sanitized to ensure logfmt compliance (no spaces, equals signs, etc.)
            let sanitized_key = sanitize_logfmt_key(key);
            output.push_str(&sanitized_key);
            output.push('=');
            self.format_dynamic_value_into(value, &mut output);
        }

        output
    }
}

// CSV formatter - outputs CSV format with required field order
pub struct CsvFormatter {
    delimiter: char,
    keys: Vec<String>,
    include_header: bool,
    formatter_key: String,
    worker_mode: bool, // If true, never write headers (for parallel workers)
}

impl CsvFormatter {
    pub fn new(keys: Vec<String>) -> Self {
        let formatter_key = format!(",_{}", Self::keys_hash(&keys));
        Self {
            delimiter: ',',
            keys,
            include_header: true,
            formatter_key,
            worker_mode: false,
        }
    }

    pub fn new_tsv(keys: Vec<String>) -> Self {
        let formatter_key = format!("\t_{}", Self::keys_hash(&keys));
        Self {
            delimiter: '\t',
            keys,
            include_header: true,
            formatter_key,
            worker_mode: false,
        }
    }

    pub fn new_csv_no_header(keys: Vec<String>) -> Self {
        let formatter_key = format!(",_noheader_{}", Self::keys_hash(&keys));
        Self {
            delimiter: ',',
            keys,
            include_header: false,
            formatter_key,
            worker_mode: false,
        }
    }

    pub fn new_tsv_no_header(keys: Vec<String>) -> Self {
        let formatter_key = format!("\t_noheader_{}", Self::keys_hash(&keys));
        Self {
            delimiter: '\t',
            keys,
            include_header: false,
            formatter_key,
            worker_mode: false,
        }
    }

    /// Create worker-mode variants that never write headers
    pub fn new_worker(keys: Vec<String>) -> Self {
        let formatter_key = format!(",_worker_{}", Self::keys_hash(&keys));
        Self {
            delimiter: ',',
            keys,
            include_header: false, // Workers never write headers
            formatter_key,
            worker_mode: true,
        }
    }

    pub fn new_tsv_worker(keys: Vec<String>) -> Self {
        let formatter_key = format!("\t_worker_{}", Self::keys_hash(&keys));
        Self {
            delimiter: '\t',
            keys,
            include_header: false, // Workers never write headers
            formatter_key,
            worker_mode: true,
        }
    }

    pub fn new_csv_no_header_worker(keys: Vec<String>) -> Self {
        let formatter_key = format!(",_noheader_worker_{}", Self::keys_hash(&keys));
        Self {
            delimiter: ',',
            keys,
            include_header: false,
            formatter_key,
            worker_mode: true,
        }
    }

    pub fn new_tsv_no_header_worker(keys: Vec<String>) -> Self {
        let formatter_key = format!("\t_noheader_worker_{}", Self::keys_hash(&keys));
        Self {
            delimiter: '\t',
            keys,
            include_header: false,
            formatter_key,
            worker_mode: true,
        }
    }

    /// Create a simple hash of the keys for uniqueness
    fn keys_hash(keys: &[String]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        keys.hash(&mut hasher);
        hasher.finish()
    }

    /// Mark header as written globally for this formatter configuration
    /// Returns true if this call was the first to mark it (header should be written)
    fn mark_header_written_globally(&self) -> bool {
        let mut registry = CSV_FORMATTER_HEADER_REGISTRY.lock().unwrap();
        if registry.get(&self.formatter_key).copied().unwrap_or(false) {
            // Already marked by another thread
            false
        } else {
            // This is the first thread to mark it
            registry.insert(self.formatter_key.clone(), true);
            true
        }
    }

    /// Format the header row
    pub fn format_header(&self) -> String {
        self.keys
            .iter()
            .map(|key| escape_csv_value(key, self.delimiter))
            .collect::<Vec<_>>()
            .join(&self.delimiter.to_string())
    }

    /// Format a data row
    fn format_data_row(&self, event: &Event) -> String {
        self.keys
            .iter()
            .map(|key| {
                if let Some(value) = event.fields.get(key) {
                    escape_csv_value(&value.to_string(), self.delimiter)
                } else {
                    String::new() // Empty field for missing values
                }
            })
            .collect::<Vec<_>>()
            .join(&self.delimiter.to_string())
    }
}

impl pipeline::Formatter for CsvFormatter {
    fn format(&self, event: &Event) -> String {
        let mut output = String::new();

        // Write header row if needed (thread-safe, once only across all workers)
        // Workers in parallel mode never write headers
        if !self.worker_mode && self.include_header && self.mark_header_written_globally() {
            output.push_str(&self.format_header());
            output.push('\n');
        }

        // Write data row
        output.push_str(&self.format_data_row(event));
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_formatter_empty_event() {
        let event = Event::default();
        let formatter = JsonFormatter::new();
        let result = formatter.format(&event);
        assert!(result.starts_with('{') && result.ends_with('}'));
    }

    #[test]
    fn test_json_formatter_with_fields() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("INFO".to_string()));
        event.set_field("msg".to_string(), Dynamic::from("Test message".to_string()));
        event.set_field("user".to_string(), Dynamic::from("alice".to_string()));
        event.set_field("status".to_string(), Dynamic::from(200i64));

        let formatter = JsonFormatter::new();
        let result = formatter.format(&event);

        assert!(result.contains("\"level\":\"INFO\""));
        assert!(result.contains("\"msg\":\"Test message\""));
        assert!(result.contains("\"user\":\"alice\""));
        assert!(result.contains("\"status\":200"));
    }

    #[test]
    fn test_default_formatter() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("INFO".to_string()));
        event.set_field("user".to_string(), Dynamic::from("alice".to_string()));
        event.set_field("count".to_string(), Dynamic::from(42i64));

        let formatter = DefaultFormatter::new(
            false,
            false,
            crate::config::TimestampFormatConfig::default(),
        ); // No colors, no brief mode
        let result = formatter.format(&event);

        // Check that all fields are present with proper formatting
        // Strings should be quoted, numbers should not be
        assert!(result.contains("level=\"INFO\""));
        assert!(result.contains("user=\"alice\""));
        assert!(result.contains("count=42"));
        // Fields should be space-separated
        assert!(result.contains(" "));
    }

    #[test]
    fn test_default_formatter_brief_mode() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("info".to_string()));
        event.set_field("msg".to_string(), Dynamic::from("test message".to_string()));

        let formatter =
            DefaultFormatter::new(false, true, crate::config::TimestampFormatConfig::default()); // No colors, brief mode
        let result = formatter.format(&event);

        // Brief mode should output only values, space-separated
        assert_eq!(result, "info test message");
    }

    #[test]
    fn test_logfmt_formatter_basic() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("INFO".to_string()));
        event.set_field("msg".to_string(), Dynamic::from("Test message".to_string()));
        event.set_field("user".to_string(), Dynamic::from("alice".to_string()));
        event.set_field("status".to_string(), Dynamic::from(200i64));

        let formatter = LogfmtFormatter::new();
        let result = formatter.format(&event);

        // Should properly quote strings with spaces, leave numbers unquoted
        assert!(result.contains("level=INFO"));
        assert!(result.contains("msg=\"Test message\""));
        assert!(result.contains("user=alice"));
        assert!(result.contains("status=200"));
        // Fields should be space-separated
        assert!(result.contains(" "));
    }

    #[test]
    fn test_logfmt_formatter_quoting() {
        let mut event = Event::default();
        event.set_field("simple".to_string(), Dynamic::from("value".to_string()));
        event.set_field(
            "spaced".to_string(),
            Dynamic::from("has spaces".to_string()),
        );
        event.set_field("empty".to_string(), Dynamic::from("".to_string()));
        event.set_field(
            "quoted".to_string(),
            Dynamic::from("has\"quotes".to_string()),
        );
        event.set_field("equals".to_string(), Dynamic::from("has=sign".to_string()));

        let formatter = LogfmtFormatter::new();
        let result = formatter.format(&event);

        assert!(result.contains("simple=value")); // No quotes needed
        assert!(result.contains("spaced=\"has spaces\"")); // Quotes due to space
        assert!(result.contains("empty=\"\"")); // Quotes due to empty
        assert!(result.contains("quoted=\"has\\\"quotes\"")); // Escaped quotes
        assert!(result.contains("equals=\"has=sign\"")); // Quotes due to equals sign
    }

    #[test]
    fn test_logfmt_formatter_types() {
        let mut event = Event::default();
        event.set_field("string".to_string(), Dynamic::from("hello".to_string()));
        event.set_field("integer".to_string(), Dynamic::from(42i64));
        event.set_field("float".to_string(), Dynamic::from(3.14f64));
        event.set_field("bool_true".to_string(), Dynamic::from(true));
        event.set_field("bool_false".to_string(), Dynamic::from(false));

        let formatter = LogfmtFormatter::new();
        let result = formatter.format(&event);

        // Numbers and booleans should not be quoted
        assert!(result.contains("string=hello"));
        assert!(result.contains("integer=42"));
        assert!(result.contains("float=3.14"));
        assert!(result.contains("bool_true=true"));
        assert!(result.contains("bool_false=false"));
    }

    #[test]
    fn test_logfmt_formatter_empty_event() {
        let event = Event::default();
        let formatter = LogfmtFormatter::new();
        let result = formatter.format(&event);
        assert_eq!(result, "");
    }

    #[test]
    fn test_logfmt_formatter_key_sanitization() {
        let mut event = Event::default();
        // Test various problematic key characters
        event.set_field(
            "field with spaces".to_string(),
            Dynamic::from("value1".to_string()),
        );
        event.set_field(
            "field=with=equals".to_string(),
            Dynamic::from("value2".to_string()),
        );
        event.set_field(
            "field\twith\ttabs".to_string(),
            Dynamic::from("value3".to_string()),
        );
        event.set_field(
            "field\nwith\nnewlines".to_string(),
            Dynamic::from("value4".to_string()),
        );
        event.set_field(
            "field\rwith\rcarriage".to_string(),
            Dynamic::from("value5".to_string()),
        );
        event.set_field(
            "normal_field".to_string(),
            Dynamic::from("value6".to_string()),
        );
        event.set_field(
            "field-with-dashes".to_string(),
            Dynamic::from("value7".to_string()),
        );
        event.set_field(
            "field.with.dots".to_string(),
            Dynamic::from("value8".to_string()),
        );

        let formatter = LogfmtFormatter::new();
        let result = formatter.format(&event);

        // Keys should be sanitized by replacing problematic characters with underscores
        assert!(result.contains("field_with_spaces=value1"));
        assert!(result.contains("field_with_equals=value2"));
        assert!(result.contains("field_with_tabs=value3"));
        assert!(result.contains("field_with_newlines=value4"));
        assert!(result.contains("field_with_carriage=value5"));
        assert!(result.contains("normal_field=value6"));

        // Non-problematic characters should be preserved
        assert!(result.contains("field-with-dashes=value7"));
        assert!(result.contains("field.with.dots=value8"));

        // Ensure the result can be parsed by the logfmt parser
        let parser = crate::parsers::logfmt::LogfmtParser::new();
        let parsed = crate::pipeline::EventParser::parse(&parser, &result);
        assert!(
            parsed.is_ok(),
            "Sanitized logfmt output should be parseable: {}",
            result
        );

        let parsed_event = parsed.unwrap();
        // Verify that sanitized keys preserve the data
        assert_eq!(
            parsed_event
                .fields
                .get("field_with_spaces")
                .unwrap()
                .to_string(),
            "value1"
        );
        assert_eq!(
            parsed_event
                .fields
                .get("field_with_equals")
                .unwrap()
                .to_string(),
            "value2"
        );
        assert_eq!(
            parsed_event.fields.get("normal_field").unwrap().to_string(),
            "value6"
        );
    }

    #[test]
    fn test_sanitize_logfmt_key_function() {
        // Test the sanitize_logfmt_key function directly
        assert_eq!(sanitize_logfmt_key("normal_field"), "normal_field");
        assert_eq!(
            sanitize_logfmt_key("field with spaces"),
            "field_with_spaces"
        );
        assert_eq!(
            sanitize_logfmt_key("field=with=equals"),
            "field_with_equals"
        );
        assert_eq!(sanitize_logfmt_key("field\twith\ttabs"), "field_with_tabs");
        assert_eq!(
            sanitize_logfmt_key("field\nwith\nnewlines"),
            "field_with_newlines"
        );
        assert_eq!(
            sanitize_logfmt_key("field\rwith\rcarriage"),
            "field_with_carriage"
        );
        assert_eq!(
            sanitize_logfmt_key("field-with-dashes"),
            "field-with-dashes"
        );
        assert_eq!(sanitize_logfmt_key("field.with.dots"), "field.with.dots");
        assert_eq!(
            sanitize_logfmt_key("field_with_underscores"),
            "field_with_underscores"
        );
        assert_eq!(sanitize_logfmt_key(""), "");

        // Test edge cases
        assert_eq!(sanitize_logfmt_key("==="), "___");
        assert_eq!(sanitize_logfmt_key("   "), "___");
        assert_eq!(sanitize_logfmt_key(" = \t = \n = \r "), "_____________");
    }

    #[test]
    fn test_hide_formatter() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("INFO".to_string()));
        event.set_field("msg".to_string(), Dynamic::from("Test message".to_string()));
        event.set_field("user".to_string(), Dynamic::from("alice".to_string()));

        let formatter = HideFormatter::new();
        let result = formatter.format(&event);
        assert_eq!(result, "");
    }

    #[test]
    fn test_hide_formatter_empty_event() {
        let event = Event::default();
        let formatter = HideFormatter::new();
        let result = formatter.format(&event);
        assert_eq!(result, "");
    }

    #[test]
    fn test_null_formatter_behavior() {
        // Null format uses HideFormatter, so test that it produces empty strings
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("ERROR".to_string()));
        event.set_field(
            "msg".to_string(),
            Dynamic::from("Critical error".to_string()),
        );

        let formatter = HideFormatter::new(); // Null format uses HideFormatter
        let result = formatter.format(&event);
        assert_eq!(result, ""); // Should be empty for null format
    }

    #[test]
    fn test_shared_escaping_utilities() {
        // Test escape_logfmt_string
        assert_eq!(escape_logfmt_string("simple"), "simple");
        assert_eq!(escape_logfmt_string("with\"quotes"), "with\\\"quotes");
        assert_eq!(escape_logfmt_string("with\nnewline"), "with\\nnewline");
        assert_eq!(escape_logfmt_string("with\ttab"), "with\\ttab");
        assert_eq!(escape_logfmt_string("with\\backslash"), "with\\\\backslash");

        // Test needs_logfmt_quoting
        assert!(!needs_logfmt_quoting("simple"));
        assert!(needs_logfmt_quoting("with spaces"));
        assert!(needs_logfmt_quoting(""));
        assert!(needs_logfmt_quoting("with=equals"));
        assert!(needs_logfmt_quoting("with\"quotes"));
        assert!(needs_logfmt_quoting("with\ttab"));

        // Test format_dynamic_value
        assert_eq!(
            format_dynamic_value(&Dynamic::from("test")),
            ("test".to_string(), true)
        );
        assert_eq!(
            format_dynamic_value(&Dynamic::from(42i64)),
            ("42".to_string(), false)
        );
        assert_eq!(
            format_dynamic_value(&Dynamic::from(true)),
            ("true".to_string(), false)
        );
    }

    #[test]
    fn test_csv_formatter_basic() {
        let keys = vec!["name".to_string(), "age".to_string(), "city".to_string()];
        let formatter = CsvFormatter::new(keys);

        let mut event = Event::default();
        event.set_field("name".to_string(), Dynamic::from("Alice".to_string()));
        event.set_field("age".to_string(), Dynamic::from(25i64));
        event.set_field("city".to_string(), Dynamic::from("New York".to_string()));

        let result = formatter.format(&event);

        // Should include header and data
        assert!(result.contains("name,age,city"));
        assert!(result.contains("Alice,25,New York"));
    }

    #[test]
    fn test_csv_formatter_with_quoting() {
        let keys = vec!["name".to_string(), "msg".to_string()];
        let formatter = CsvFormatter::new(keys);

        let mut event = Event::default();
        event.set_field("name".to_string(), Dynamic::from("Smith, John".to_string()));
        event.set_field(
            "msg".to_string(),
            Dynamic::from("He said \"hello\"".to_string()),
        );

        let result = formatter.format(&event);

        // Should properly quote values with commas and quotes
        assert!(result.contains("\"Smith, John\""));
        assert!(result.contains("\"He said \"\"hello\"\"\""));
    }

    #[test]
    fn test_tsv_formatter_basic() {
        let keys = vec!["name".to_string(), "age".to_string()];
        let formatter = CsvFormatter::new_tsv(keys);

        let mut event = Event::default();
        event.set_field("name".to_string(), Dynamic::from("Alice".to_string()));
        event.set_field("age".to_string(), Dynamic::from(25i64));

        let result = formatter.format(&event);

        // Should use tab separator
        assert!(result.contains("name\tage"));
        assert!(result.contains("Alice\t25"));
    }

    #[test]
    fn test_csv_formatter_no_header() {
        let keys = vec!["name".to_string(), "age".to_string()];
        let formatter = CsvFormatter::new_csv_no_header(keys);

        let mut event = Event::default();
        event.set_field("name".to_string(), Dynamic::from("Alice".to_string()));
        event.set_field("age".to_string(), Dynamic::from(25i64));

        let result = formatter.format(&event);

        // Should not include header
        assert!(!result.contains("name,age"));
        assert_eq!(result, "Alice,25");
    }

    #[test]
    fn test_csv_formatter_missing_fields() {
        let keys = vec!["name".to_string(), "age".to_string(), "city".to_string()];
        let formatter = CsvFormatter::new_csv_no_header(keys);

        let mut event = Event::default();
        event.set_field("name".to_string(), Dynamic::from("Alice".to_string()));
        // age is missing
        event.set_field("city".to_string(), Dynamic::from("Boston".to_string()));

        let result = formatter.format(&event);

        // Should have empty field for missing age
        assert_eq!(result, "Alice,,Boston");
    }

    #[test]
    fn test_csv_escaping_utilities() {
        // Test needs_csv_quoting
        assert!(!needs_csv_quoting("simple", ','));
        assert!(needs_csv_quoting("with,comma", ','));
        assert!(needs_csv_quoting("with\"quote", ','));
        assert!(needs_csv_quoting("with\nnewline", ','));
        assert!(needs_csv_quoting("", ','));
        assert!(needs_csv_quoting(" leading", ','));
        assert!(needs_csv_quoting("trailing ", ','));

        // Test with tab delimiter
        assert!(!needs_csv_quoting("with,comma", '\t'));
        assert!(needs_csv_quoting("with\ttab", '\t'));

        // Test escape_csv_value
        assert_eq!(escape_csv_value("simple", ','), "simple");
        assert_eq!(escape_csv_value("with,comma", ','), "\"with,comma\"");
        assert_eq!(escape_csv_value("with\"quote", ','), "\"with\"\"quote\"");
        assert_eq!(escape_csv_value("", ','), "\"\"");
    }
}
