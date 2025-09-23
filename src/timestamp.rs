#![allow(dead_code)]
use chrono::{DateTime, Local, TimeZone, Utc};

/// Adaptive timestamp parser that dynamically reorders formats based on success
/// Each thread should have its own instance to avoid contention
pub struct AdaptiveTsParser {
    /// Format list with successful formats moved to front
    formats: Vec<String>,
}

impl AdaptiveTsParser {
    pub fn new() -> Self {
        Self {
            formats: get_initial_timestamp_formats(),
        }
    }

    /// Parse timestamp using adaptive format ordering
    /// Successful formats are moved to front of the list for faster future parsing
    pub fn parse_ts(&mut self, ts_str: &str) -> Option<DateTime<Utc>> {
        self.parse_ts_with_config(ts_str, None, None)
    }

    /// Parse timestamp with full configuration support
    pub fn parse_ts_with_config(
        &mut self,
        ts_str: &str,
        custom_format: Option<&str>,
        default_timezone: Option<&str>,
    ) -> Option<DateTime<Utc>> {
        let ts_str = ts_str.trim();

        // Try custom format first if provided
        if let Some(format) = custom_format {
            if let Some(parsed) = try_parse_with_format(ts_str, format, default_timezone) {
                return Some(parsed);
            }
        }

        // Handle special values (journalctl-compatible)
        match ts_str {
            "now" => return Some(Utc::now()),
            "today" => {
                let local_today = Local::now().date_naive();
                if let Some(naive_datetime) = local_today.and_hms_opt(0, 0, 0) {
                    return Some(naive_datetime.and_utc());
                }
            }
            "yesterday" => {
                let local_yesterday = Local::now().date_naive() - chrono::Duration::days(1);
                if let Some(naive_datetime) = local_yesterday.and_hms_opt(0, 0, 0) {
                    return Some(naive_datetime.and_utc());
                }
            }
            "tomorrow" => {
                let local_tomorrow = Local::now().date_naive() + chrono::Duration::days(1);
                if let Some(naive_datetime) = local_tomorrow.and_hms_opt(0, 0, 0) {
                    return Some(naive_datetime.and_utc());
                }
            }
            _ => {}
        }

        // Handle relative times (e.g., "-1h", "+30m", "-2d", "1h", "30m")
        if ts_str.starts_with('-') || ts_str.starts_with('+') || looks_like_relative_time(ts_str) {
            if let Ok(dt) = parse_relative_time(ts_str) {
                return Some(dt);
            }
        }

        // Try Unix timestamp parsing for numeric-only strings
        if ts_str.chars().all(|c| c.is_ascii_digit()) {
            if let Some(parsed) = try_parse_unix_timestamp(ts_str) {
                return Some(parsed);
            }
        }

        // Try standard RFC formats (these are very fast)
        if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
            return Some(dt.with_timezone(&Utc));
        }
        if let Ok(dt) = DateTime::parse_from_rfc2822(ts_str) {
            return Some(dt.with_timezone(&Utc));
        }

        // Try date-only formats and assume 00:00:00
        if let Some(dt) = parse_date_only(ts_str) {
            return Some(dt);
        }

        // Try time-only formats and assume today's date
        if let Some(dt) = parse_time_only(ts_str) {
            return Some(dt);
        }

        // Try format list with adaptive reordering
        self.try_formats_with_reordering(ts_str, default_timezone)
    }

    /// Try formats with timezone configuration and move successful ones to front
    fn try_formats_with_reordering(
        &mut self,
        ts_str: &str,
        default_timezone: Option<&str>,
    ) -> Option<DateTime<Utc>> {
        for (index, format) in self.formats.iter().enumerate() {
            if let Some(parsed) = try_parse_with_format(ts_str, format, default_timezone) {
                // Move successful format to front if it's not already there
                if index > 0 {
                    let successful_format = self.formats.remove(index);
                    self.formats.insert(0, successful_format);
                }
                return Some(parsed);
            }
        }

        None
    }

    /// Reset the format ordering to initial state (for testing/debugging)
    #[allow(dead_code)]
    pub fn reset_ordering(&mut self) {
        self.formats = get_initial_timestamp_formats();
    }

    /// Get current format ordering (for debugging/monitoring)
    #[allow(dead_code)]
    pub fn get_format_ordering(&self) -> Vec<String> {
        self.formats.clone()
    }
}

impl Default for AdaptiveTsParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Try to parse a timestamp with a specific format and timezone configuration
fn try_parse_with_format(
    ts_str: &str,
    format: &str,
    default_timezone: Option<&str>,
) -> Option<DateTime<Utc>> {
    use chrono_tz::Tz;

    // Handle comma-separated fractional seconds (Python logging format)
    let (processed_ts_str, processed_format) = if format.contains(",%f") {
        // Convert comma to dot for chrono compatibility and handle milliseconds properly
        if let Some(comma_pos) = ts_str.rfind(',') {
            let base_part = &ts_str[..comma_pos];
            let frac_part = &ts_str[comma_pos + 1..];

            // Pad or truncate fractional part to 9 digits (nanoseconds)
            let frac_nanos = if frac_part.len() <= 3 {
                // Treat as milliseconds, pad to nanoseconds
                format!("{:0<9}", format!("{:0<3}", frac_part))
            } else if frac_part.len() <= 6 {
                // Treat as microseconds, pad to nanoseconds
                format!("{:0<9}", format!("{:0<6}", frac_part))
            } else {
                // Truncate to nanoseconds
                frac_part[..9].to_string()
            };

            let new_ts_str = format!("{}.{}", base_part, frac_nanos);
            let new_format = format.replace(",%f", ".%f");
            (new_ts_str, new_format)
        } else {
            (ts_str.to_string(), format.to_string())
        }
    } else {
        (ts_str.to_string(), format.to_string())
    };

    // Try timezone-aware parsing first
    if let Ok(dt) = DateTime::parse_from_str(&processed_ts_str, &processed_format) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try naive parsing
    if let Ok(naive_dt) =
        chrono::NaiveDateTime::parse_from_str(&processed_ts_str, &processed_format)
    {
        // Apply timezone configuration
        match default_timezone {
            Some("UTC") => {
                return Some(naive_dt.and_utc());
            }
            Some(tz_str) => {
                // Try to parse as named timezone
                if let Ok(tz) = tz_str.parse::<Tz>() {
                    if let Some(dt) = tz.from_local_datetime(&naive_dt).single() {
                        return Some(dt.with_timezone(&Utc));
                    }
                }
                // Fall back to local time if timezone parsing fails
                if let Some(local_dt) = chrono::Local.from_local_datetime(&naive_dt).single() {
                    return Some(local_dt.with_timezone(&Utc));
                }
            }
            None => {
                // Default: interpret as local time
                if let Some(local_dt) = chrono::Local.from_local_datetime(&naive_dt).single() {
                    return Some(local_dt.with_timezone(&Utc));
                }
            }
        }
    }

    None
}

/// Try to parse Unix timestamp based on string length
fn try_parse_unix_timestamp(ts_str: &str) -> Option<DateTime<Utc>> {
    let timestamp_int = ts_str.parse::<i64>().ok()?;

    // Detect Unix timestamp precision by string length
    let dt = match ts_str.len() {
        10 => {
            // Seconds (1735566123)
            DateTime::from_timestamp(timestamp_int, 0)
        }
        13 => {
            // Milliseconds (1735566123000)
            DateTime::from_timestamp(
                timestamp_int / 1000,
                (timestamp_int % 1000) as u32 * 1_000_000,
            )
        }
        16 => {
            // Microseconds (1735566123000000)
            DateTime::from_timestamp(
                timestamp_int / 1_000_000,
                (timestamp_int % 1_000_000) as u32 * 1_000,
            )
        }
        19 => {
            // Nanoseconds (1735566123000000000)
            DateTime::from_timestamp(
                timestamp_int / 1_000_000_000,
                (timestamp_int % 1_000_000_000) as u32,
            )
        }
        _ => None,
    };

    dt.map(|dt| dt.with_timezone(&Utc))
}

/// Get initial list of timestamp formats
/// Ordered by likelihood/performance, with most common formats first
fn get_initial_timestamp_formats() -> Vec<String> {
    vec![
        // ISO 8601 variants (most common in logs)
        "%Y-%m-%dT%H:%M:%S%.fZ".to_string(), // ISO 8601 with subseconds
        "%Y-%m-%dT%H:%M:%SZ".to_string(),    // ISO 8601
        "%Y-%m-%dT%H:%M:%S%.f%:z".to_string(), // ISO 8601 with timezone
        "%Y-%m-%dT%H:%M:%S%:z".to_string(),  // ISO 8601 with timezone
        "%Y-%m-%dT%H:%M:%S%.f".to_string(),  // ISO 8601 without timezone (with subseconds)
        "%Y-%m-%dT%H:%M:%S".to_string(),     // ISO 8601 without timezone
        // Space-separated ISO variants (common in many log formats)
        "%Y-%m-%d %H:%M:%S%.f".to_string(), // Common log format with subseconds
        "%Y-%m-%d %H:%M:%S".to_string(),    // Common log format
        "%Y-%m-%d %H:%M:%S%.fZ".to_string(), // Space-separated with Z
        "%Y-%m-%d %H:%M:%SZ".to_string(),   // Space-separated with Z
        "%Y-%m-%d %H:%M:%S%z".to_string(),  // Space-separated with timezone
        "%Y-%m-%d %H:%M:%S%.f%z".to_string(), // Space-separated with fractional + timezone
        // Syslog and server log formats
        "%b %d %H:%M:%S".to_string(),       // Syslog format
        "%b %d %Y %H:%M:%S".to_string(),    // BSD syslog with year
        "%d/%b/%Y:%H:%M:%S %z".to_string(), // Apache log format
        // Application-specific formats
        "%Y-%m-%d %H:%M:%S,%f".to_string(), // Python logging format
        "%Y/%m/%d %H:%M:%S".to_string(),    // Nginx error log format
        "%d.%m.%Y %H:%M:%S".to_string(),    // German format
        "%y%m%d %H:%M:%S".to_string(),      // MySQL legacy format
        // Less common but valid formats
        "%a %b %d %H:%M:%S %Y".to_string(), // Classic Unix timestamp
        "%d-%b-%y %I:%M:%S.%f %p".to_string(), // Oracle format
        "%b %d, %Y %I:%M:%S %p".to_string(), // Java SimpleDateFormat
    ]
}

/// Configuration for timestamp field identification and parsing
#[derive(Debug, Clone)]
pub struct TsConfig {
    /// Custom timestamp field name (overrides auto-detection)
    pub custom_field: Option<String>,
    /// Custom timestamp format string
    pub custom_format: Option<String>,
    /// Default timezone for naive timestamps (None = local time)
    pub default_timezone: Option<String>,
    /// Whether to automatically parse timestamps from events (reserved for future features)
    #[allow(dead_code)]
    pub auto_parse: bool,
}

impl Default for TsConfig {
    fn default() -> Self {
        Self {
            custom_field: None,
            custom_format: None,
            default_timezone: None,
            auto_parse: true,
        }
    }
}

/// Identify and extract timestamp from event fields
pub fn identify_timestamp_field(
    fields: &indexmap::IndexMap<String, rhai::Dynamic>,
    config: &TsConfig,
) -> Option<(String, String)> {
    // If custom field is specified, try that first
    if let Some(ref custom_field) = config.custom_field {
        if let Some(value) = fields.get(custom_field) {
            if let Ok(ts_str) = value.clone().into_string() {
                return Some((custom_field.clone(), ts_str));
            }
        }
    }

    // Otherwise, try the standard timestamp field names
    for ts_key in crate::event::TIMESTAMP_FIELD_NAMES {
        if let Some(value) = fields.get(*ts_key) {
            if let Ok(ts_str) = value.clone().into_string() {
                return Some((ts_key.to_string(), ts_str));
            }
        }
    }

    None
}

/// Parse timestamp arguments (--since, --until) in journalctl-compatible format
/// Uses the enhanced adaptive parser with journalctl support
pub fn parse_timestamp_arg_with_timezone(
    arg: &str,
    default_timezone: Option<&str>,
) -> Result<DateTime<Utc>, String> {
    let mut parser = AdaptiveTsParser::new();
    parser
        .parse_ts_with_config(arg, None, default_timezone)
        .ok_or_else(|| format!("Could not parse timestamp: {}", arg))
}

/// Check if a string looks like a relative time expression (e.g., "1h", "30m", "2d", "1 hour")
fn looks_like_relative_time(arg: &str) -> bool {
    // Must start with a digit
    if !arg.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }

    // Find where numbers end (may have spaces before unit)
    let num_end = arg.find(|c: char| !c.is_ascii_digit()).unwrap_or(arg.len());
    let remainder = &arg[num_end..].trim_start();

    if remainder.is_empty() {
        return false;
    }

    // Check if it's a valid time unit
    matches!(
        *remainder,
        "s" | "sec"
            | "secs"
            | "second"
            | "seconds"
            | "m"
            | "min"
            | "mins"
            | "minute"
            | "minutes"
            | "h"
            | "hour"
            | "hours"
            | "d"
            | "day"
            | "days"
            | "w"
            | "week"
            | "weeks"
    )
}

/// Parse relative time expressions like "-1h", "+30m", "-2d", "1h", "30m"
/// Unsigned times default to past (e.g., "1h" means "1 hour ago")
fn parse_relative_time(arg: &str) -> Result<DateTime<Utc>, String> {
    let (sign, rest) = if let Some(stripped) = arg.strip_prefix('-') {
        (-1, stripped)
    } else if let Some(stripped) = arg.strip_prefix('+') {
        (1, stripped)
    } else {
        // Unsigned times default to past (e.g., "1h" means "1 hour ago")
        (-1, arg)
    };

    if rest.is_empty() {
        return Err("Empty relative time".to_string());
    }

    // Parse number and unit (handle spaces between them)
    let (num_str, unit) = if let Some(pos) = rest.find(|c: char| !c.is_ascii_digit()) {
        let num_part = &rest[..pos];
        let unit_part = rest[pos..].trim_start();
        if unit_part.is_empty() || !unit_part.chars().next().unwrap().is_alphabetic() {
            return Err("Relative time must have a valid unit (h, m, d, etc.)".to_string());
        }
        (num_part, unit_part)
    } else {
        return Err("Relative time must have a unit (h, m, d, etc.)".to_string());
    };

    let num: i64 = num_str
        .parse()
        .map_err(|_| "Invalid number in relative time")?;
    let signed_num = sign * num;

    let duration = match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => chrono::Duration::seconds(signed_num),
        "m" | "min" | "mins" | "minute" | "minutes" => chrono::Duration::minutes(signed_num),
        "h" | "hour" | "hours" => chrono::Duration::hours(signed_num),
        "d" | "day" | "days" => chrono::Duration::days(signed_num),
        "w" | "week" | "weeks" => chrono::Duration::weeks(signed_num),
        _ => return Err(format!("Unknown time unit: {}", unit)),
    };

    Ok(Utc::now() + duration)
}

/// Parse date-only strings and assume 00:00:00
fn parse_date_only(arg: &str) -> Option<DateTime<Utc>> {
    // Try YYYY-MM-DD format
    if let Ok(date) = chrono::NaiveDate::parse_from_str(arg, "%Y-%m-%d") {
        if let Some(naive_dt) = date.and_hms_opt(0, 0, 0) {
            return Some(naive_dt.and_utc());
        }
    }

    // Try MM/DD/YYYY format
    if let Ok(date) = chrono::NaiveDate::parse_from_str(arg, "%m/%d/%Y") {
        if let Some(naive_dt) = date.and_hms_opt(0, 0, 0) {
            return Some(naive_dt.and_utc());
        }
    }

    // Try DD.MM.YYYY format
    if let Ok(date) = chrono::NaiveDate::parse_from_str(arg, "%d.%m.%Y") {
        if let Some(naive_dt) = date.and_hms_opt(0, 0, 0) {
            return Some(naive_dt.and_utc());
        }
    }

    None
}

/// Parse time-only strings and assume today's date
fn parse_time_only(arg: &str) -> Option<DateTime<Utc>> {
    let today = Local::now().date_naive();

    // Try HH:MM:SS format
    if let Ok(time) = chrono::NaiveTime::parse_from_str(arg, "%H:%M:%S") {
        let naive_dt = today.and_time(time);
        return Some(naive_dt.and_utc());
    }

    // Try HH:MM format (assume :00 seconds)
    if let Ok(time) = chrono::NaiveTime::parse_from_str(arg, "%H:%M") {
        let naive_dt = today.and_time(time);
        return Some(naive_dt.and_utc());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn test_adaptive_parser_basic() {
        let mut parser = AdaptiveTsParser::new();

        // Test ISO 8601 parsing
        let result = parser.parse_ts("2023-07-04T12:34:56Z");
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2023);
        assert_eq!(dt.month(), 7);
        assert_eq!(dt.day(), 4);
    }

    #[test]
    fn test_format_reordering() {
        let mut parser = AdaptiveTsParser::new();

        // Parse first timestamp - should move successful format to front
        let result1 = parser.parse_ts("2023-07-04 12:34:56");
        assert!(result1.is_some());

        // Check that format was moved to front
        let formats = parser.get_format_ordering();
        assert!(!formats.is_empty());
        // The successful format should now be at index 0
        assert_eq!(formats[0], "%Y-%m-%d %H:%M:%S%.f");

        // Parse second timestamp with same format - should be faster (first try)
        let result2 = parser.parse_ts("2023-07-05 13:45:07");
        assert!(result2.is_some());
    }

    #[test]
    fn test_unix_timestamp_parsing() {
        let mut parser = AdaptiveTsParser::new();

        // Test seconds precision
        let result = parser.parse_ts("1735566123");
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);

        // Test milliseconds precision
        let result = parser.parse_ts("1735566123000");
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);
    }

    #[test]
    fn test_timestamp_field_identification() {
        use indexmap::IndexMap;
        use rhai::Dynamic;

        let mut fields = IndexMap::new();
        fields.insert("ts".to_string(), Dynamic::from("2023-07-04T12:34:56Z"));
        fields.insert("message".to_string(), Dynamic::from("test message"));

        let config = TsConfig::default();
        let result = identify_timestamp_field(&fields, &config);

        assert!(result.is_some());
        let (field_name, value) = result.unwrap();
        assert_eq!(field_name, "ts");
        assert_eq!(value, "2023-07-04T12:34:56Z");
    }

    #[test]
    fn test_custom_timestamp_field() {
        use indexmap::IndexMap;
        use rhai::Dynamic;

        let mut fields = IndexMap::new();
        fields.insert(
            "custom_time".to_string(),
            Dynamic::from("2023-07-04T12:34:56Z"),
        );
        fields.insert("ts".to_string(), Dynamic::from("other_timestamp"));

        let config = TsConfig {
            custom_field: Some("custom_time".to_string()),
            custom_format: None,
            default_timezone: None,
            auto_parse: true,
        };

        let result = identify_timestamp_field(&fields, &config);

        assert!(result.is_some());
        let (field_name, value) = result.unwrap();
        assert_eq!(field_name, "custom_time");
        assert_eq!(value, "2023-07-04T12:34:56Z");
    }

    #[test]
    fn test_ordering_reset() {
        let mut parser = AdaptiveTsParser::new();

        // Parse to change ordering
        parser.parse_ts("2023-07-04 12:34:56");
        let formats_after = parser.get_format_ordering();
        assert_eq!(formats_after[0], "%Y-%m-%d %H:%M:%S%.f");

        // Reset ordering
        parser.reset_ordering();
        let formats_reset = parser.get_format_ordering();
        // Should be back to initial order
        assert_eq!(formats_reset[0], "%Y-%m-%dT%H:%M:%S%.fZ");
    }

    #[test]
    fn test_invalid_timestamp() {
        let mut parser = AdaptiveTsParser::new();

        let result = parser.parse_ts("not-a-timestamp");
        assert!(result.is_none());

        let result = parser.parse_ts("2023-13-45T25:70:70Z");
        assert!(result.is_none());
    }

    #[test]
    fn test_multiple_format_reordering() {
        let mut parser = AdaptiveTsParser::new();

        // Parse different formats to test reordering
        parser.parse_ts("2023-07-04 12:34:56"); // Common format
        parser.parse_ts("2023-07-05T13:45:07Z"); // ISO format
        parser.parse_ts("2023-07-06 14:56:08"); // Common format again

        let formats = parser.get_format_ordering();
        // Most recently used format should be at front
        assert_eq!(formats[0], "%Y-%m-%d %H:%M:%S%.f");
    }

    #[test]
    fn test_journalctl_compatible_formats() {
        let mut parser = AdaptiveTsParser::new();

        // Test special values
        let now = parser.parse_ts("now");
        assert!(now.is_some());

        let today = parser.parse_ts("today");
        assert!(today.is_some());

        let yesterday = parser.parse_ts("yesterday");
        assert!(yesterday.is_some());

        let tomorrow = parser.parse_ts("tomorrow");
        assert!(tomorrow.is_some());

        // Test relative times
        let one_hour_ago = parser.parse_ts("-1h");
        assert!(one_hour_ago.is_some());

        let thirty_min_from_now = parser.parse_ts("+30m");
        assert!(thirty_min_from_now.is_some());

        // Test date-only formats
        let date_only = parser.parse_ts("2023-07-04");
        assert!(date_only.is_some());

        // Test time-only formats
        let time_only = parser.parse_ts("15:30:45");
        assert!(time_only.is_some());
    }

    #[test]
    fn test_parse_timestamp_arg_with_timezone() {
        // Test that the wrapper function works
        let result = parse_timestamp_arg_with_timezone("2023-07-04T12:34:56Z", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("now", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("-1h", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("invalid-timestamp", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_relative_time_parsing() {
        // Test various relative time formats
        let result = parse_timestamp_arg_with_timezone("-30m", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("+2h", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("-1d", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("-2w", None);
        assert!(result.is_ok());

        // Test invalid relative times
        let result = parse_timestamp_arg_with_timezone("-", None);
        assert!(result.is_err());

        let result = parse_timestamp_arg_with_timezone("-1x", None);
        assert!(result.is_err());

        let result = parse_timestamp_arg_with_timezone("1h", None); // Unsigned defaults to past
        assert!(result.is_ok());
    }

    #[test]
    fn test_date_only_parsing() {
        let result = parse_timestamp_arg_with_timezone("2023-07-04", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("07/04/2023", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("04.07.2023", None);
        assert!(result.is_ok());

        // Invalid date formats
        let result = parse_timestamp_arg_with_timezone("2023-13-45", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_time_only_parsing() {
        let result = parse_timestamp_arg_with_timezone("15:30:45", None);
        assert!(result.is_ok());

        let result = parse_timestamp_arg_with_timezone("09:15", None);
        assert!(result.is_ok());

        // Invalid time formats
        let result = parse_timestamp_arg_with_timezone("25:70:80", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_comma_separated_milliseconds() {
        let mut parser = AdaptiveTsParser::new();

        // Test 3-digit milliseconds (most common) - use UTC timezone to avoid local time conversion
        let result = parser.parse_ts_with_config("2010-04-24 07:52:09,487", None, Some("UTC"));
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2010);
        assert_eq!(dt.month(), 4);
        assert_eq!(dt.day(), 24);
        assert_eq!(dt.hour(), 7);
        assert_eq!(dt.minute(), 52);
        assert_eq!(dt.second(), 9);
        // 487 milliseconds should equal 487,000,000 nanoseconds
        assert_eq!(
            dt.timestamp_nanos_opt().unwrap() % 1_000_000_000,
            487_000_000
        );

        // Test 2-digit centiseconds
        let result = parser.parse_ts_with_config("2010-04-24 07:52:09,12", None, Some("UTC"));
        assert!(result.is_some());
        let dt = result.unwrap();
        // 12 centiseconds = 120 milliseconds = 120,000,000 nanoseconds
        assert_eq!(
            dt.timestamp_nanos_opt().unwrap() % 1_000_000_000,
            120_000_000
        );

        // Test 1-digit deciseconds
        let result = parser.parse_ts_with_config("2010-04-24 07:52:09,5", None, Some("UTC"));
        assert!(result.is_some());
        let dt = result.unwrap();
        // 5 deciseconds = 500 milliseconds = 500,000,000 nanoseconds
        assert_eq!(
            dt.timestamp_nanos_opt().unwrap() % 1_000_000_000,
            500_000_000
        );
    }

    #[test]
    fn test_unix_timestamp_parsing_enhanced() {
        let mut parser = AdaptiveTsParser::new();

        // Test various Unix timestamp precisions
        let result = parser.parse_ts("1735566123"); // seconds
        assert!(result.is_some());

        let result = parser.parse_ts("1735566123000"); // milliseconds
        assert!(result.is_some());

        let result = parser.parse_ts("1735566123000000"); // microseconds
        assert!(result.is_some());

        let result = parser.parse_ts("1735566123000000000"); // nanoseconds
        assert!(result.is_some());

        // Test invalid Unix timestamps
        let result = parser.parse_ts("123"); // too short
        assert!(result.is_none());

        let result = parser.parse_ts("12345678901234567890123"); // too long
        assert!(result.is_none());
    }
}
