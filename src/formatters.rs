use crate::event::Event;
use crate::colors::ColorScheme;
use rhai::Dynamic;

/// Convert rhai::Dynamic to serde_json::Value
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
    } else {
        // For other types, convert to string
        serde_json::Value::String(value.to_string())
    }
}

pub trait Formatter {
    fn format(&self, event: &Event) -> String;
}

// JSON formatter
pub struct JsonFormatter;

impl JsonFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Formatter for JsonFormatter {
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

// Default formatter (logfmt-style with colors and plain mode)
pub struct DefaultFormatter {
    colors: ColorScheme,
    level_keys: Vec<&'static str>,
    plain: bool,
}

impl DefaultFormatter {
    pub fn new(use_colors: bool, plain: bool) -> Self {
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
            plain,
        }
    }

    /// Format a Dynamic value directly into buffer for performance (zero-allocation when possible)
    fn format_dynamic_value_into(&self, key: &str, value: &Dynamic, output: &mut String) {
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

        // Format value based on type - quote strings, leave numbers/bools unquoted
        if value.is_string() {
            if let Ok(s) = value.clone().into_string() {
                output.push('"');
                // Inline escape quotes to avoid allocation
                for ch in s.chars() {
                    if ch == '"' {
                        output.push_str("\\\"");
                    } else if ch == '\\' {
                        output.push_str("\\\\");
                    } else {
                        output.push(ch);
                    }
                }
                output.push('"');
            }
        } else {
            // Numbers, booleans, etc. - no quotes, direct output
            output.push_str(&value.to_string());
        }

        // Reset color
        if !color.is_empty() {
            output.push_str(self.colors.reset);
        }
    }

    /// Format a Dynamic value for plain mode (no quotes, just the value with colors)
    fn format_dynamic_value_plain_into(&self, key: &str, value: &Dynamic, output: &mut String) {
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

        // In plain mode, output raw value (no quotes even for strings)
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

}

impl Formatter for DefaultFormatter {
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

            if self.plain {
                // Plain mode: only values (no keys, no quotes)
                self.format_dynamic_value_plain_into(key, value, &mut output);
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

// Logfmt formatter - dedicated logfmt output formatter with proper quoting
pub struct LogfmtFormatter {
    colors: ColorScheme,
    level_keys: Vec<&'static str>,
    plain: bool,
}

impl LogfmtFormatter {
    pub fn new(use_colors: bool, plain: bool) -> Self {
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
            plain,
        }
    }

    /// Check if value needs to be quoted per logfmt rules
    fn needs_quoting(&self, value: &str) -> bool {
        // Quote values that contain spaces, tabs, newlines, quotes, or equals, or are empty
        value.is_empty()
            || value.contains(' ')
            || value.contains('\t')
            || value.contains('\n')
            || value.contains('"')
            || value.contains('=')
    }

    /// Format a Dynamic value directly into buffer for performance (zero-allocation when possible)
    fn format_dynamic_value_into(&self, key: &str, value: &Dynamic, output: &mut String) {
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

        // Format value based on type - quote strings if needed, leave numbers/bools unquoted
        if value.is_string() {
            if let Ok(s) = value.clone().into_string() {
                if self.needs_quoting(&s) {
                    output.push('"');
                    // Inline escape quotes to avoid allocation
                    for ch in s.chars() {
                        if ch == '"' {
                            output.push_str("\\\"");
                        } else if ch == '\\' {
                            output.push_str("\\\\");
                        } else {
                            output.push(ch);
                        }
                    }
                    output.push('"');
                } else {
                    output.push_str(&s);
                }
            }
        } else {
            // Numbers, booleans, etc. - no quotes, direct output
            output.push_str(&value.to_string());
        }

        // Reset color
        if !color.is_empty() {
            output.push_str(self.colors.reset);
        }
    }

    /// Format a Dynamic value for plain mode (no quotes, just the value with colors)
    fn format_dynamic_value_plain_into(&self, key: &str, value: &Dynamic, output: &mut String) {
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

        // In plain mode, output raw value (no quotes even for strings)
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
}

impl Formatter for LogfmtFormatter {
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

            if self.plain {
                // Plain mode: only values (no keys, no quotes)
                self.format_dynamic_value_plain_into(key, value, &mut output);
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

                // Add formatted value (with proper logfmt quoting and colors)
                self.format_dynamic_value_into(key, value, &mut output);
            }
        }

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
        event.set_field("message".to_string(), Dynamic::from("Test message".to_string()));
        event.set_field("user".to_string(), Dynamic::from("alice".to_string()));
        event.set_field("status".to_string(), Dynamic::from(200i64));

        let formatter = JsonFormatter::new();
        let result = formatter.format(&event);

        assert!(result.contains("\"level\":\"INFO\""));
        assert!(result.contains("\"message\":\"Test message\""));
        assert!(result.contains("\"user\":\"alice\""));
        assert!(result.contains("\"status\":200"));
    }

    #[test]
    fn test_default_formatter() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("INFO".to_string()));
        event.set_field("user".to_string(), Dynamic::from("alice".to_string()));
        event.set_field("count".to_string(), Dynamic::from(42i64));

        let formatter = DefaultFormatter::new(false, false); // No colors, no plain mode
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
    fn test_default_formatter_plain_mode() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("info".to_string()));
        event.set_field("message".to_string(), Dynamic::from("test message".to_string()));

        let formatter = DefaultFormatter::new(false, true); // No colors, plain mode
        let result = formatter.format(&event);

        // Plain mode should output only values, space-separated
        assert_eq!(result, "info test message");
    }

    #[test]
    fn test_logfmt_formatter_basic() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("INFO".to_string()));
        event.set_field("message".to_string(), Dynamic::from("Test message".to_string()));
        event.set_field("user".to_string(), Dynamic::from("alice".to_string()));
        event.set_field("status".to_string(), Dynamic::from(200i64));

        let formatter = LogfmtFormatter::new(false, false); // No colors, no plain mode
        let result = formatter.format(&event);

        // Should properly quote strings with spaces, leave numbers unquoted
        assert!(result.contains("level=INFO"));
        assert!(result.contains("message=\"Test message\""));
        assert!(result.contains("user=alice"));
        assert!(result.contains("status=200"));
        // Fields should be space-separated
        assert!(result.contains(" "));
    }

    #[test]
    fn test_logfmt_formatter_quoting() {
        let mut event = Event::default();
        event.set_field("simple".to_string(), Dynamic::from("value".to_string()));
        event.set_field("spaced".to_string(), Dynamic::from("has spaces".to_string()));
        event.set_field("empty".to_string(), Dynamic::from("".to_string()));
        event.set_field("quoted".to_string(), Dynamic::from("has\"quotes".to_string()));
        event.set_field("equals".to_string(), Dynamic::from("has=sign".to_string()));

        let formatter = LogfmtFormatter::new(false, false);
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

        let formatter = LogfmtFormatter::new(false, false);
        let result = formatter.format(&event);

        // Numbers and booleans should not be quoted
        assert!(result.contains("string=hello"));
        assert!(result.contains("integer=42"));
        assert!(result.contains("float=3.14"));
        assert!(result.contains("bool_true=true"));
        assert!(result.contains("bool_false=false"));
    }

    #[test]
    fn test_logfmt_formatter_plain_mode() {
        let mut event = Event::default();
        event.set_field("level".to_string(), Dynamic::from("info".to_string()));
        event.set_field("message".to_string(), Dynamic::from("test message".to_string()));

        let formatter = LogfmtFormatter::new(false, true); // No colors, plain mode
        let result = formatter.format(&event);

        // Plain mode should output only values, space-separated
        assert_eq!(result, "info test message");
    }

    #[test]
    fn test_logfmt_formatter_empty_event() {
        let event = Event::default();
        let formatter = LogfmtFormatter::new(false, false);
        let result = formatter.format(&event);
        assert_eq!(result, "");
    }

}