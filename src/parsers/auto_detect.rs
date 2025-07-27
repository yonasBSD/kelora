use crate::config::InputFormat as ConfigInputFormat;
use anyhow::Result;

/// Auto-detect the input format based on the first line of input.
/// Tries formats in order of specificity/commonality with 'line' as fallback.
///
/// Format detection priority:
/// 1. JSONL - starts with '{' and valid JSON
/// 2. CEF - starts with "CEF:"
/// 3. Syslog - matches RFC5424 or RFC3164 patterns
/// 4. Apache/Nginx - contains common log patterns
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

    // 1. JSONL detection - most specific
    if detect_jsonl(trimmed) {
        return Ok(ConfigInputFormat::Jsonl);
    }

    // 2. CEF detection - very specific prefix
    if detect_cef(trimmed) {
        return Ok(ConfigInputFormat::Cef);
    }

    // 3. Syslog detection - structured patterns
    if detect_syslog(trimmed) {
        return Ok(ConfigInputFormat::Syslog);
    }

    // 4. Apache/Nginx log detection
    if let Some(web_format) = detect_web_logs(trimmed) {
        return Ok(web_format);
    }

    // 5. Logfmt detection - key=value patterns
    if detect_logfmt(trimmed) {
        return Ok(ConfigInputFormat::Logfmt);
    }

    // 6. CSV/TSV detection
    if let Some(csv_format) = detect_csv_variants(trimmed) {
        return Ok(csv_format);
    }

    // 7. Fallback to line format
    Ok(ConfigInputFormat::Line)
}

/// Detect JSONL format - starts with '{' and is valid JSON
#[allow(dead_code)] // Used by detect_format function
fn detect_jsonl(line: &str) -> bool {
    if !line.starts_with('{') {
        return false;
    }

    // Try to parse as JSON - if it succeeds, it's likely JSONL
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

    false
}

/// Detect web server log formats (Apache, Nginx)
#[allow(dead_code)] // Used by detect_format function
fn detect_web_logs(line: &str) -> Option<ConfigInputFormat> {
    // Common patterns in web logs:
    // Apache: IP - - [timestamp] "REQUEST" status size "referer" "user-agent"
    // Nginx: Similar but may have different ordering

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
                    // Any quoted request
                    // Could be either Apache or Nginx, default to Apache (more common)
                    return Some(ConfigInputFormat::Apache);
                }
            }
        }
    }

    None
}

/// Check if a string looks like an IP address (v4 or v6, or hostname)
#[allow(dead_code)] // Used by detect_web_logs function
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
                return Some(ConfigInputFormat::Tsv);
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
                return Some(ConfigInputFormat::Csv);
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

    #[test]
    fn test_detect_jsonl() {
        assert_eq!(
            detect_format(r#"{"key": "value", "num": 42}"#).unwrap(),
            ConfigInputFormat::Jsonl
        );
        assert_eq!(
            detect_format(r#"{"timestamp": "2023-04-15T10:00:00Z"}"#).unwrap(),
            ConfigInputFormat::Jsonl
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
    }

    #[test]
    fn test_detect_apache() {
        assert_eq!(
            detect_format(
                r#"192.168.1.1 - - [15/Apr/2023:10:00:00 +0000] "GET /path HTTP/1.1" 200 1234"#
            )
            .unwrap(),
            ConfigInputFormat::Apache
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
        assert_eq!(
            detect_format("name,age,city").unwrap(),
            ConfigInputFormat::Csv
        );
        assert_eq!(detect_format("1,2,3").unwrap(), ConfigInputFormat::Csvnh);
        assert_eq!(
            detect_format("john\t25\tnyc").unwrap(),
            ConfigInputFormat::Tsv
        ); // "john" has letters, so it's treated as header
        assert_eq!(
            detect_format("name\tage\tcity").unwrap(),
            ConfigInputFormat::Tsv
        );
        assert_eq!(detect_format("1\t2\t3").unwrap(), ConfigInputFormat::Tsvnh);
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
}
