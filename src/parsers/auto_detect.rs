use crate::config::InputFormat as ConfigInputFormat;
use anyhow::Result;

/// Auto-detect the input format based on the first line of input.
/// Tries formats in order of specificity/commonality with 'line' as fallback.
///
/// Format detection priority:
/// 1. JSON - starts with '{' and valid JSON
/// 2. CEF - starts with "CEF:"
/// 3. Syslog - matches RFC5424 or RFC3164 patterns
/// 4. Combined - contains common Apache/Nginx log patterns
/// 5. Logfmt - contains key=value pairs
/// 6. CSV/TSV - contains delimiters with reasonable structure
/// 7. Line - fallback for everything else
#[allow(dead_code)] // Used by lib.rs for format auto-detection
pub fn detect_format(sample_line: &str) -> Result<ConfigInputFormat> {
    let trimmed = sample_line.trim();

    // Empty line detection - default to line format
    if trimmed.is_empty() {
        return Ok(ConfigInputFormat::Line);
    }

    // 1. JSON detection - most specific
    if detect_json(trimmed) {
        return Ok(ConfigInputFormat::Json);
    }

    // 2. CEF detection - very specific prefix
    if detect_cef(trimmed) {
        return Ok(ConfigInputFormat::Cef);
    }

    // 3. Syslog detection - structured patterns
    if detect_syslog(trimmed) {
        return Ok(ConfigInputFormat::Syslog);
    }

    // 4. Combined log format detection (Apache/Nginx)
    if detect_combined_logs(trimmed) {
        return Ok(ConfigInputFormat::Combined);
    }

    // 6. Logfmt detection - key=value patterns
    if detect_logfmt(trimmed) {
        return Ok(ConfigInputFormat::Logfmt);
    }

    // 7. CSV/TSV detection
    if let Some(csv_format) = detect_csv_variants(trimmed) {
        return Ok(csv_format);
    }

    // 8. Fallback to line format
    Ok(ConfigInputFormat::Line)
}

/// Detect JSON format - starts with '{' and is valid JSON
#[allow(dead_code)] // Used by detect_format function
fn detect_json(line: &str) -> bool {
    if !line.starts_with('{') {
        return false;
    }

    // Try to parse as JSON - if it succeeds, it's likely JSON
    serde_json::from_str::<serde_json::Value>(line).is_ok()
}

/// Detect CEF format - starts with "CEF:"
#[allow(dead_code)] // Used by detect_format function
fn detect_cef(line: &str) -> bool {
    line.starts_with("CEF:")
}

/// Detect Syslog format using patterns similar to SyslogParser
#[allow(dead_code)] // Used by detect_format function
fn detect_syslog(line: &str) -> bool {
    // RFC5424 pattern: <priority>version timestamp hostname app-name procid msgid structured-data message
    // Example: <34>1 2023-04-15T10:00:00.000Z hostname app-name - - - message
    if line.starts_with('<') {
        if let Some(end_bracket) = line.find('>') {
            if end_bracket < 10 {
                // Reasonable priority field length
                let after_priority = &line[end_bracket + 1..];
                // RFC5424 has version number after priority
                if after_priority.starts_with('1') && after_priority.len() > 2 {
                    let next_char = after_priority.chars().nth(1);
                    if next_char == Some(' ') || next_char == Some('\t') {
                        return true;
                    }
                }
                // RFC3164 pattern: <priority>timestamp hostname program: message
                // Timestamp typically starts with month name
                if after_priority.len() > 3 {
                    let timestamp_part = &after_priority[..3];
                    if matches!(
                        timestamp_part,
                        "Jan"
                            | "Feb"
                            | "Mar"
                            | "Apr"
                            | "May"
                            | "Jun"
                            | "Jul"
                            | "Aug"
                            | "Sep"
                            | "Oct"
                            | "Nov"
                            | "Dec"
                    ) {
                        return true;
                    }
                }
            }
        }
    }

    // RFC3164 pattern without priority: timestamp hostname program: message
    // Example: Jan 15 10:30:45 server1 sshd[1234]: Accepted publickey for user
    if line.len() > 15 {
        let month_part = &line[..3];
        if matches!(
            month_part,
            "Jan"
                | "Feb"
                | "Mar"
                | "Apr"
                | "May"
                | "Jun"
                | "Jul"
                | "Aug"
                | "Sep"
                | "Oct"
                | "Nov"
                | "Dec"
        ) {
            // Check for typical syslog timestamp pattern: "MMM dd HH:MM:SS"
            // Look for space after month, then day (1-2 digits), then space, then time pattern
            if let Some(space1) = line[3..].find(' ') {
                let after_month = &line[3 + space1 + 1..];
                if let Some(space2) = after_month.find(' ') {
                    let day_part = &after_month[..space2];
                    // Day should be 1-2 digits
                    if day_part.len() <= 2 && day_part.chars().all(|c| c.is_ascii_digit()) {
                        let after_day = &after_month[space2 + 1..];
                        // Check for time pattern: HH:MM:SS
                        if after_day.len() >= 8 {
                            let time_part = &after_day[..8];
                            if time_part.matches(':').count() == 2 {
                                // Look for hostname and program pattern after timestamp
                                if let Some(space3) = after_day[8..].find(' ') {
                                    let after_time = &after_day[8 + space3 + 1..];
                                    // Look for program: pattern (hostname program: or hostname program[pid]:)
                                    if after_time.contains(':') {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    false
}

/// Detect combined log formats (Apache/Nginx compatible)
#[allow(dead_code)] // Used by detect_format function
fn detect_combined_logs(line: &str) -> bool {
    // Common patterns in web logs:
    // Combined: IP - - [timestamp] "REQUEST" status size "referer" "user-agent" [request_time]
    // Common: IP - - [timestamp] "REQUEST" status size

    // Look for IP address at start
    if let Some(first_space) = line.find(' ') {
        let potential_ip = &line[..first_space];
        if is_likely_ip_address(potential_ip) {
            // Look for timestamp in brackets [dd/Mon/yyyy:hh:mm:ss +offset]
            if line.contains('[') && line.contains(']') && line.contains(':') {
                // Check for quoted strings that suggest HTTP requests
                if line.contains("\"GET ")
                    || line.contains("\"POST ")
                    || line.contains("\"PUT ")
                    || line.contains("\"DELETE ")
                    || line.contains("\" ")
                {
                    // Any quoted request - fits combined log format pattern
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a string looks like an IP address (v4 or v6, or hostname)
#[allow(dead_code)] // Used by detect_combined_logs function
fn is_likely_ip_address(s: &str) -> bool {
    // IPv4 pattern (rough check)
    if s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.contains('.') {
        return true;
    }

    // IPv6 pattern (rough check)
    if s.contains(':') && s.chars().all(|c| c.is_ascii_hexdigit() || c == ':') {
        return true;
    }

    // Hostname pattern - contains letters and possibly dots/hyphens
    if s.chars().any(|c| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return true;
    }

    false
}

/// Detect logfmt format - contains key=value pairs
#[allow(dead_code)] // Used by detect_format function
fn detect_logfmt(line: &str) -> bool {
    // Look for patterns like key=value
    let mut has_equals = false;
    let mut potential_pairs = 0;

    for part in line.split_whitespace() {
        if part.contains('=') {
            has_equals = true;
            // Check if it looks like a valid key=value pair
            if let Some(eq_pos) = part.find('=') {
                let key = &part[..eq_pos];
                let value = &part[eq_pos + 1..];

                // Key should be reasonable (letters/numbers/underscore)
                if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    // Value can be anything, but if it's there, it's a good sign
                    if !value.is_empty() {
                        potential_pairs += 1;
                    }
                }
            }
        }
    }

    // Require at least one equal sign and at least one valid-looking pair
    has_equals && potential_pairs > 0
}

/// Detect CSV/TSV variants
#[allow(dead_code)] // Used by detect_format function
fn detect_csv_variants(line: &str) -> Option<ConfigInputFormat> {
    let comma_count = line.matches(',').count();
    let tab_count = line.matches('\t').count();

    // Require multiple delimiters to distinguish from random commas/tabs in text
    if tab_count >= 2 {
        // Check if it could have headers vs no headers
        // If first field looks like a column name (letters), assume headers
        if let Some(first_field) = line.split('\t').next() {
            if first_field.chars().any(|c| c.is_ascii_alphabetic())
                && !first_field.chars().all(|c| c.is_ascii_digit())
            {
                return Some(ConfigInputFormat::Tsv(None));
            } else {
                return Some(ConfigInputFormat::Tsvnh);
            }
        }
    }

    if comma_count >= 2 {
        // Similar logic for CSV
        if let Some(first_field) = line.split(',').next() {
            let trimmed_field = first_field.trim_matches('"').trim();
            if trimmed_field.chars().any(|c| c.is_ascii_alphabetic())
                && !trimmed_field.chars().all(|c| c.is_ascii_digit())
            {
                return Some(ConfigInputFormat::Csv(None));
            } else {
                return Some(ConfigInputFormat::Csvnh);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest::strategy::{BoxedStrategy, Strategy};

    #[test]
    fn test_detect_json() {
        assert_eq!(
            detect_format(r#"{"key": "value", "num": 42}"#).unwrap(),
            ConfigInputFormat::Json
        );
        assert_eq!(
            detect_format(r#"{"timestamp": "2023-04-15T10:00:00Z"}"#).unwrap(),
            ConfigInputFormat::Json
        );
    }

    #[test]
    fn test_detect_cef() {
        assert_eq!(
            detect_format("CEF:0|Vendor|Product|Version|EventID|Name|Severity|Extension").unwrap(),
            ConfigInputFormat::Cef
        );
    }

    #[test]
    fn test_detect_syslog() {
        assert_eq!(
            detect_format("<34>1 2023-04-15T10:00:00.000Z hostname app - - - message").unwrap(),
            ConfigInputFormat::Syslog
        );
        assert_eq!(
            detect_format("<13>Apr 15 10:00:00 hostname program: message").unwrap(),
            ConfigInputFormat::Syslog
        );
        // Test syslog format without priority field (common in processed logs)
        assert_eq!(
            detect_format("Jan 15 10:30:45 server1 sshd[1234]: Accepted publickey for user")
                .unwrap(),
            ConfigInputFormat::Syslog
        );
        assert_eq!(
            detect_format("Dec 25 23:59:59 hostname kernel: USB disconnect").unwrap(),
            ConfigInputFormat::Syslog
        );
    }

    #[test]
    fn test_detect_combined() {
        assert_eq!(
            detect_format(
                r#"192.168.1.1 - - [15/Apr/2023:10:00:00 +0000] "GET /path HTTP/1.1" 200 1234"#
            )
            .unwrap(),
            ConfigInputFormat::Combined
        );
    }

    #[test]
    fn test_detect_logfmt() {
        assert_eq!(
            detect_format("time=2023-04-15T10:00:00Z level=info msg=test").unwrap(),
            ConfigInputFormat::Logfmt
        );
        assert_eq!(
            detect_format("key1=value1 key2=value2 key3=value3").unwrap(),
            ConfigInputFormat::Logfmt
        );
    }

    #[test]
    fn test_detect_csv() {
        assert!(matches!(
            detect_format("name,age,city").unwrap(),
            ConfigInputFormat::Csv(_)
        ));
        assert!(matches!(
            detect_format("1,2,3").unwrap(),
            ConfigInputFormat::Csvnh
        ));
        assert!(matches!(
            detect_format("john\t25\tnyc").unwrap(),
            ConfigInputFormat::Tsv(_)
        )); // "john" has letters, so it's treated as header
        assert!(matches!(
            detect_format("name\tage\tcity").unwrap(),
            ConfigInputFormat::Tsv(_)
        ));
        assert!(matches!(
            detect_format("1\t2\t3").unwrap(),
            ConfigInputFormat::Tsvnh
        ));
        // All numeric, no headers
    }

    #[test]
    fn test_detect_line_fallback() {
        assert_eq!(
            detect_format("just some random text").unwrap(),
            ConfigInputFormat::Line
        );
        assert_eq!(detect_format("").unwrap(), ConfigInputFormat::Line);
        assert_eq!(
            detect_format("a single word").unwrap(),
            ConfigInputFormat::Line
        );
    }

    fn lower_ascii(len: std::ops::RangeInclusive<usize>) -> BoxedStrategy<String> {
        prop::collection::vec(proptest::char::range('a', 'z'), len)
            .prop_map(|chars| chars.into_iter().collect())
            .boxed()
    }

    fn identifier() -> BoxedStrategy<String> {
        lower_ascii(1..=8)
    }

    fn short_ascii_text() -> BoxedStrategy<String> {
        lower_ascii(3..=12)
    }

    fn json_value() -> BoxedStrategy<serde_json::Value> {
        let string_val = lower_ascii(0..=8)
            .prop_map(serde_json::Value::String)
            .boxed();

        let number_val = any::<i64>()
            .prop_map(|v| serde_json::Value::Number(serde_json::Number::from(v)))
            .boxed();

        let bool_val = any::<bool>()
            .prop_map(serde_json::Value::Bool)
            .boxed();

        prop_oneof![string_val, number_val, bool_val].boxed()
    }

    fn json_line() -> BoxedStrategy<String> {
        prop::collection::vec((identifier(), json_value()), 1..=4)
            .prop_map(|entries| {
                let mut map = serde_json::Map::new();
                for (k, v) in entries {
                    map.insert(k, v);
                }
                serde_json::Value::Object(map).to_string()
            })
            .boxed()
    }

    fn cef_line() -> BoxedStrategy<String> {
        (
            identifier(),
            identifier(),
            identifier(),
            identifier(),
            identifier(),
            0u8..=10,
            identifier(),
            identifier(),
        )
            .prop_map(|(vendor, product, version, signature, name, severity, ext_key, ext_value)| {
                format!(
                    "CEF:0|{vendor}|{product}|{version}|{signature}|{name}|{severity}|{ext_key}={ext_value}"
                )
            })
            .boxed()
    }

    fn month_abbrev() -> BoxedStrategy<&'static str> {
        proptest::sample::select(&[
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ])
        .boxed()
    }

    fn syslog_rfc5424_line() -> BoxedStrategy<String> {
        (
            0u32..=191,
            identifier(),
            identifier(),
            0u32..=65535,
            identifier(),
            short_ascii_text(),
        )
            .prop_map(|(priority, host, app, pid, msgid, msg)| {
                format!(
                    "<{priority}>1 2024-01-02T03:04:05.006Z {host} {app} {pid} {msgid} - {msg}"
                )
            })
            .boxed()
    }

    fn syslog_rfc3164_line() -> BoxedStrategy<String> {
        (
            month_abbrev(),
            1u8..=28,
            0u8..=23,
            0u8..=59,
            0u8..=59,
            identifier(),
            identifier(),
            0u16..=9999,
            short_ascii_text(),
        )
            .prop_map(|(month, day, hour, minute, second, host, program, pid, message)| {
                let day_formatted = format!("{:>2}", day);
                format!(
                    "{month} {day_formatted} {hour:02}:{minute:02}:{second:02} {host} {program}[{pid}]: {message}"
                )
            })
            .boxed()
    }

    fn syslog_line() -> BoxedStrategy<String> {
        prop_oneof![syslog_rfc5424_line(), syslog_rfc3164_line()].boxed()
    }

    fn ip_octet() -> BoxedStrategy<u8> {
        (1u8..=255).boxed()
    }

    fn combined_line() -> BoxedStrategy<String> {
        (
            (ip_octet(), ip_octet(), ip_octet(), ip_octet()),
            1u8..=28,
            month_abbrev(),
            0u8..=23,
            0u8..=59,
            0u8..=59,
            proptest::sample::select(&["GET", "POST", "PUT", "DELETE"]),
            identifier(),
            100u16..=599,
            0u32..=10_000,
        )
            .prop_map(|((a, b, c, d), day, month, hour, minute, second, method, path, status, size)| {
                let ip = format!("{a}.{b}.{c}.{d}");
                let timestamp = format!("{day:02}/{month}/2024:{hour:02}:{minute:02}:{second:02} +0000");
                format!(
                    "{ip} - - [{timestamp}] \"{method} /{path} HTTP/1.1\" {status} {size} \"-\" \"Mozilla/5.0\""
                )
            })
            .boxed()
    }

    fn logfmt_value() -> BoxedStrategy<String> {
        prop_oneof![
            identifier(),
            any::<i64>().prop_map(|v| v.to_string()).boxed(),
            short_ascii_text(),
        ]
        .boxed()
    }

    fn logfmt_line() -> BoxedStrategy<String> {
        prop::collection::vec((identifier(), logfmt_value()), 2..=4)
            .prop_map(|pairs| {
                pairs
                    .into_iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .boxed()
    }

    fn csv_with_headers() -> BoxedStrategy<String> {
        prop::collection::vec(identifier(), 3..=5)
            .prop_map(|fields| fields.join(","))
            .boxed()
    }

    fn csv_without_headers() -> BoxedStrategy<String> {
        prop::collection::vec(0u16..=999, 3..=5)
            .prop_map(|nums| nums.into_iter().map(|n| n.to_string()).collect::<Vec<_>>().join(","))
            .boxed()
    }

    fn tsv_with_headers() -> BoxedStrategy<String> {
        prop::collection::vec(identifier(), 3..=5)
            .prop_map(|fields| fields.join("\t"))
            .boxed()
    }

    fn tsv_without_headers() -> BoxedStrategy<String> {
        prop::collection::vec(0u16..=999, 3..=5)
            .prop_map(|nums| nums.into_iter().map(|n| n.to_string()).collect::<Vec<_>>().join("\t"))
            .boxed()
    }

    fn plain_line() -> BoxedStrategy<String> {
        lower_ascii(5..=30)
    }

    proptest! {
        #[test]
        fn prop_detects_json(line in json_line()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Json);
        }

        #[test]
        fn prop_detects_cef(line in cef_line()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Cef);
        }

        #[test]
        fn prop_detects_syslog(line in syslog_line()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Syslog);
        }

        #[test]
        fn prop_detects_combined(line in combined_line()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Combined);
        }

        #[test]
        fn prop_detects_logfmt(line in logfmt_line()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Logfmt);
        }

        #[test]
        fn prop_detects_csv_headers(line in csv_with_headers()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Csv(None));
        }

        #[test]
        fn prop_detects_csv_no_headers(line in csv_without_headers()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Csvnh);
        }

        #[test]
        fn prop_detects_tsv_headers(line in tsv_with_headers()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Tsv(None));
        }

        #[test]
        fn prop_detects_tsv_no_headers(line in tsv_without_headers()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Tsvnh);
        }

        #[test]
        fn prop_detects_line_fallback(line in plain_line()) {
            prop_assert_eq!(detect_format(&line).unwrap(), ConfigInputFormat::Line);
        }
    }
}
