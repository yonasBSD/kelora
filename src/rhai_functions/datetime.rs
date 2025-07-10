use chrono::{DateTime, Datelike, Duration, Local, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use rhai::{Engine, EvalAltResult, Position};
use std::cell::RefCell;
use std::fmt;
use std::str::FromStr;

/// Wrapper for chrono::DateTime to provide Rhai integration
#[derive(Debug, Clone)]
pub struct DateTimeWrapper {
    pub inner: DateTime<Tz>,
}

impl DateTimeWrapper {
    pub fn new(dt: DateTime<Tz>) -> Self {
        Self { inner: dt }
    }

    pub fn from_utc(dt: DateTime<Utc>) -> Self {
        Self {
            inner: dt.with_timezone(&Tz::UTC),
        }
    }

    pub fn from_local(dt: DateTime<Local>) -> Self {
        Self {
            inner: dt.with_timezone(&Tz::UTC),
        }
    }
}

impl fmt::Display for DateTimeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner.to_rfc3339())
    }
}

/// Wrapper for chrono::Duration to provide Rhai integration
#[derive(Debug, Clone)]
pub struct DurationWrapper {
    pub inner: Duration,
}

impl DurationWrapper {
    pub fn new(dur: Duration) -> Self {
        // Ensure durations are always non-negative as per spec
        Self { inner: dur.abs() }
    }

    pub fn from_seconds(secs: i64) -> Self {
        Self::new(Duration::seconds(secs))
    }

    pub fn from_minutes(mins: i64) -> Self {
        Self::new(Duration::minutes(mins))
    }

    pub fn from_hours(hours: i64) -> Self {
        Self::new(Duration::hours(hours))
    }

    pub fn from_days(days: i64) -> Self {
        Self::new(Duration::days(days))
    }

    pub fn from_milliseconds(ms: i64) -> Self {
        Self::new(Duration::milliseconds(ms))
    }

    pub fn from_nanoseconds(ns: i64) -> Self {
        Self::new(Duration::nanoseconds(ns))
    }
}

impl fmt::Display for DurationWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total_seconds = self.inner.num_seconds();
        if total_seconds < 60 {
            write!(f, "{}s", total_seconds)
        } else if total_seconds < 3600 {
            let minutes = total_seconds / 60;
            let seconds = total_seconds % 60;
            if seconds == 0 {
                write!(f, "{}m", minutes)
            } else {
                write!(f, "{}m {}s", minutes, seconds)
            }
        } else if total_seconds < 86400 {
            let hours = total_seconds / 3600;
            let remaining = total_seconds % 3600;
            let minutes = remaining / 60;
            if minutes == 0 {
                write!(f, "{}h", hours)
            } else {
                write!(f, "{}h {}m", hours, minutes)
            }
        } else {
            let days = total_seconds / 86400;
            let remaining = total_seconds % 86400;
            let hours = remaining / 3600;
            if hours == 0 {
                write!(f, "{}d", days)
            } else {
                write!(f, "{}d {}h", days, hours)
            }
        }
    }
}

// Thread-local adaptive parser for Rhai timestamp parsing
thread_local! {
    static RHAI_TIMESTAMP_PARSER: RefCell<crate::timestamp::AdaptiveTimestampParser> = 
        RefCell::new(crate::timestamp::AdaptiveTimestampParser::new());
}

/// Parse timestamp with optional format and timezone
pub fn parse_timestamp(
    s: &str,
    format: Option<&str>,
    tz: Option<&str>,
) -> Result<DateTimeWrapper, Box<EvalAltResult>> {
    // Default timezone
    let default_tz = if let Some(tz_str) = tz {
        tz_str.parse::<Tz>().map_err(|e| {
            Box::new(EvalAltResult::ErrorRuntime(
                format!("Invalid timezone '{}': {}", tz_str, e).into(),
                Position::NONE,
            ))
        })?
    } else {
        Tz::UTC
    };

    // Try explicit format first
    if let Some(fmt) = format {
        if let Ok(naive_dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(DateTimeWrapper::new(
                default_tz.from_utc_datetime(&naive_dt),
            ));
        }
        // Also try with timezone-aware parsing for explicit format
        if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
            return Ok(DateTimeWrapper::new(dt.with_timezone(&default_tz)));
        }
    }

    // For auto-parsing (no explicit format), use the adaptive parser
    let parsed_utc = RHAI_TIMESTAMP_PARSER.with(|parser| {
        parser.borrow_mut().parse_timestamp(s)
    });

    if let Some(utc_dt) = parsed_utc {
        // Convert to the requested timezone
        let tz_dt = if default_tz == Tz::UTC {
            utc_dt.with_timezone(&Tz::UTC)
        } else {
            utc_dt.with_timezone(&default_tz)
        };
        return Ok(DateTimeWrapper::new(tz_dt));
    }

    Err(Box::new(EvalAltResult::ErrorRuntime(
        format!("Unable to parse timestamp: '{}'", s).into(),
        Position::NONE,
    )))
}

/// Parse duration from string like "1h 30m", "2d", etc.
pub fn parse_duration(s: &str) -> Result<DurationWrapper, Box<EvalAltResult>> {
    let mut total_duration = Duration::zero();
    let mut current_number = String::new();
    let mut found_unit = false;
    let chars = s.chars();

    for ch in chars {
        if ch.is_numeric() {
            current_number.push(ch);
        } else if ch.is_alphabetic() || ch == ' ' {
            if !current_number.is_empty() {
                let number: i64 = current_number.parse().map_err(|_| {
                    Box::new(EvalAltResult::ErrorRuntime(
                        format!("Invalid number in duration: '{}'", current_number).into(),
                        Position::NONE,
                    ))
                })?;

                let unit = ch.to_lowercase().next().unwrap();
                let duration_part = match unit {
                    's' => Duration::seconds(number),
                    'm' => Duration::minutes(number),
                    'h' => Duration::hours(number),
                    'd' => Duration::days(number),
                    _ => {
                        return Err(Box::new(EvalAltResult::ErrorRuntime(
                            format!("Unknown duration unit: '{}'", unit).into(),
                            Position::NONE,
                        )))
                    }
                };

                total_duration += duration_part;
                current_number.clear();
                found_unit = true;
            }
        } else if ch == ' ' {
            // Skip spaces
            continue;
        } else {
            return Err(Box::new(EvalAltResult::ErrorRuntime(
                format!("Invalid character in duration: '{}'", ch).into(),
                Position::NONE,
            )));
        }
    }

    // Return error if we found no valid units
    if !found_unit {
        return Err(Box::new(EvalAltResult::ErrorRuntime(
            format!("Unable to parse duration: '{}'", s).into(),
            Position::NONE,
        )));
    }

    Ok(DurationWrapper::new(total_duration))
}

/// Register all datetime functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    // Parsing functions
    engine.register_fn(
        "parse_timestamp",
        |s: &str| -> Result<DateTimeWrapper, Box<EvalAltResult>> { parse_timestamp(s, None, None) },
    );

    engine.register_fn(
        "parse_timestamp",
        |s: &str, format: &str| -> Result<DateTimeWrapper, Box<EvalAltResult>> {
            parse_timestamp(s, Some(format), None)
        },
    );

    engine.register_fn(
        "parse_timestamp",
        |s: &str, format: &str, tz: &str| -> Result<DateTimeWrapper, Box<EvalAltResult>> {
            parse_timestamp(s, Some(format), Some(tz))
        },
    );

    engine.register_fn("parse_duration", parse_duration);

    // Current time helpers
    engine.register_fn("now_utc", || DateTimeWrapper::from_utc(Utc::now()));
    engine.register_fn("now_local", || DateTimeWrapper::from_local(Local::now()));

    // Duration creation functions
    engine.register_fn("duration_from_seconds", DurationWrapper::from_seconds);
    engine.register_fn("duration_from_minutes", DurationWrapper::from_minutes);
    engine.register_fn("duration_from_hours", DurationWrapper::from_hours);
    engine.register_fn("duration_from_days", DurationWrapper::from_days);
    engine.register_fn(
        "duration_from_milliseconds",
        DurationWrapper::from_milliseconds,
    );
    engine.register_fn(
        "duration_from_nanoseconds",
        DurationWrapper::from_nanoseconds,
    );

    // Register the custom types
    engine
        .register_type::<DateTimeWrapper>()
        .register_type::<DurationWrapper>();

    // DateTime methods
    engine.register_fn("to_utc", |dt: &mut DateTimeWrapper| {
        DateTimeWrapper::new(dt.inner.with_timezone(&Tz::UTC))
    });

    engine.register_fn("to_local", |dt: &mut DateTimeWrapper| {
        let local_tz = chrono_tz::Tz::from_str("UTC").unwrap(); // This should be system local timezone
        DateTimeWrapper::new(dt.inner.with_timezone(&local_tz))
    });

    engine.register_fn(
        "to_timezone",
        |dt: &mut DateTimeWrapper, tz: &str| -> Result<DateTimeWrapper, Box<EvalAltResult>> {
            let timezone = tz.parse::<Tz>().map_err(|e| {
                Box::new(EvalAltResult::ErrorRuntime(
                    format!("Invalid timezone '{}': {}", tz, e).into(),
                    Position::NONE,
                ))
            })?;
            Ok(DateTimeWrapper::new(dt.inner.with_timezone(&timezone)))
        },
    );

    engine.register_fn("format", |dt: &mut DateTimeWrapper, fmt: &str| -> String {
        dt.inner.format(fmt).to_string()
    });

    engine.register_fn("year", |dt: &mut DateTimeWrapper| dt.inner.year() as i64);
    engine.register_fn("month", |dt: &mut DateTimeWrapper| dt.inner.month() as i64);
    engine.register_fn("day", |dt: &mut DateTimeWrapper| dt.inner.day() as i64);
    engine.register_fn("hour", |dt: &mut DateTimeWrapper| dt.inner.hour() as i64);
    engine.register_fn("minute", |dt: &mut DateTimeWrapper| {
        dt.inner.minute() as i64
    });
    engine.register_fn("second", |dt: &mut DateTimeWrapper| {
        dt.inner.second() as i64
    });
    engine.register_fn("timestamp_nanos", |dt: &mut DateTimeWrapper| {
        dt.inner.timestamp_nanos_opt().unwrap_or(0)
    });
    engine.register_fn("timezone_name", |dt: &mut DateTimeWrapper| {
        dt.inner.timezone().to_string()
    });

    // Duration methods
    engine.register_fn("as_seconds", |dur: &mut DurationWrapper| {
        dur.inner.num_seconds()
    });
    engine.register_fn("as_milliseconds", |dur: &mut DurationWrapper| {
        dur.inner.num_milliseconds()
    });
    engine.register_fn("as_nanoseconds", |dur: &mut DurationWrapper| {
        dur.inner.num_nanoseconds().unwrap_or(0)
    });
    engine.register_fn("as_minutes", |dur: &mut DurationWrapper| {
        dur.inner.num_minutes()
    });
    engine.register_fn("as_hours", |dur: &mut DurationWrapper| {
        dur.inner.num_hours()
    });
    engine.register_fn("as_days", |dur: &mut DurationWrapper| dur.inner.num_days());

    // DateTime arithmetic
    engine.register_fn("+", |dt: DateTimeWrapper, dur: DurationWrapper| {
        DateTimeWrapper::new(dt.inner + dur.inner)
    });

    engine.register_fn("-", |dt: DateTimeWrapper, dur: DurationWrapper| {
        DateTimeWrapper::new(dt.inner - dur.inner)
    });

    engine.register_fn("-", |dt1: DateTimeWrapper, dt2: DateTimeWrapper| {
        DurationWrapper::new((dt1.inner - dt2.inner).abs())
    });

    // Duration arithmetic
    engine.register_fn("+", |dur1: DurationWrapper, dur2: DurationWrapper| {
        DurationWrapper::new(dur1.inner + dur2.inner)
    });

    engine.register_fn("-", |dur1: DurationWrapper, dur2: DurationWrapper| {
        DurationWrapper::new((dur1.inner - dur2.inner).abs())
    });

    engine.register_fn("*", |dur: DurationWrapper, n: i64| {
        DurationWrapper::new(dur.inner * n as i32)
    });

    engine.register_fn("/", |dur: DurationWrapper, n: i64| {
        DurationWrapper::new(dur.inner / n as i32)
    });

    // Duration comparison
    engine.register_fn("==", |dur1: DurationWrapper, dur2: DurationWrapper| {
        dur1.inner == dur2.inner
    });
    engine.register_fn("!=", |dur1: DurationWrapper, dur2: DurationWrapper| {
        dur1.inner != dur2.inner
    });
    engine.register_fn(">", |dur1: DurationWrapper, dur2: DurationWrapper| {
        dur1.inner > dur2.inner
    });
    engine.register_fn("<", |dur1: DurationWrapper, dur2: DurationWrapper| {
        dur1.inner < dur2.inner
    });
    engine.register_fn(">=", |dur1: DurationWrapper, dur2: DurationWrapper| {
        dur1.inner >= dur2.inner
    });
    engine.register_fn("<=", |dur1: DurationWrapper, dur2: DurationWrapper| {
        dur1.inner <= dur2.inner
    });

    // DateTime comparison
    engine.register_fn("==", |dt1: DateTimeWrapper, dt2: DateTimeWrapper| {
        dt1.inner == dt2.inner
    });
    engine.register_fn("!=", |dt1: DateTimeWrapper, dt2: DateTimeWrapper| {
        dt1.inner != dt2.inner
    });
    engine.register_fn(">", |dt1: DateTimeWrapper, dt2: DateTimeWrapper| {
        dt1.inner > dt2.inner
    });
    engine.register_fn("<", |dt1: DateTimeWrapper, dt2: DateTimeWrapper| {
        dt1.inner < dt2.inner
    });
    engine.register_fn(">=", |dt1: DateTimeWrapper, dt2: DateTimeWrapper| {
        dt1.inner >= dt2.inner
    });
    engine.register_fn("<=", |dt1: DateTimeWrapper, dt2: DateTimeWrapper| {
        dt1.inner <= dt2.inner
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_wrapper_always_positive() {
        // Test that negative durations become positive
        let negative_dur = Duration::seconds(-100);
        let wrapper = DurationWrapper::new(negative_dur);
        assert_eq!(wrapper.inner.num_seconds(), 100);
    }

    #[test]
    fn test_duration_display_formatting() {
        // Test various duration display formats
        assert_eq!(DurationWrapper::from_seconds(30).to_string(), "30s");
        assert_eq!(DurationWrapper::from_seconds(60).to_string(), "1m");
        assert_eq!(DurationWrapper::from_seconds(90).to_string(), "1m 30s");
        assert_eq!(DurationWrapper::from_seconds(3600).to_string(), "1h");
        assert_eq!(DurationWrapper::from_seconds(3660).to_string(), "1h 1m");
        assert_eq!(DurationWrapper::from_seconds(86400).to_string(), "1d");
        assert_eq!(DurationWrapper::from_seconds(90000).to_string(), "1d 1h");
    }

    #[test]
    fn test_parse_timestamp_edge_cases() {
        // Test empty string
        assert!(parse_timestamp("", None, None).is_err());

        // Test invalid formats
        assert!(parse_timestamp("not-a-date", None, None).is_err());
        assert!(parse_timestamp("2023-13-01T12:00:00Z", None, None).is_err()); // Invalid month
        assert!(parse_timestamp("2023-02-30T12:00:00Z", None, None).is_err()); // Invalid day

        // Test valid edge cases
        assert!(parse_timestamp("2023-01-01T00:00:00Z", None, None).is_ok());
        assert!(parse_timestamp("2023-12-31T23:59:59Z", None, None).is_ok());
    }

    #[test]
    fn test_parse_timestamp_with_explicit_format() {
        // Test custom format parsing
        let result = parse_timestamp("2023/07/04 12:34:56", Some("%Y/%m/%d %H:%M:%S"), None);
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);

        // Test invalid format with explicit format
        assert!(parse_timestamp("2023-07-04", Some("%Y/%m/%d"), None).is_err());
    }

    #[test]
    fn test_parse_timestamp_with_timezone() {
        // Test parsing with valid timezone
        let result = parse_timestamp(
            "2023-07-04 12:34:56",
            Some("%Y-%m-%d %H:%M:%S"),
            Some("UTC"),
        );
        assert!(result.is_ok());

        // Test parsing with invalid timezone
        let result = parse_timestamp(
            "2023-07-04 12:34:56",
            Some("%Y-%m-%d %H:%M:%S"),
            Some("INVALID"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_duration_edge_cases() {
        // Test empty string
        assert!(parse_duration("").is_err());

        // Test invalid characters
        assert!(parse_duration("1x").is_err());
        assert!(parse_duration("1h@30m").is_err());

        // Test invalid numbers
        assert!(parse_duration("ah").is_err());

        // Test zero duration
        assert!(parse_duration("0s").is_ok());

        // Test complex valid durations
        assert!(parse_duration("1d 2h 3m 4s").is_ok());
        assert!(parse_duration("100h").is_ok());
    }

    #[test]
    fn test_parse_duration_various_formats() {
        // Test single units
        let dur_s = parse_duration("30s").unwrap();
        assert_eq!(dur_s.inner.num_seconds(), 30);

        let dur_m = parse_duration("5m").unwrap();
        assert_eq!(dur_m.inner.num_minutes(), 5);

        let dur_h = parse_duration("2h").unwrap();
        assert_eq!(dur_h.inner.num_hours(), 2);

        let dur_d = parse_duration("3d").unwrap();
        assert_eq!(dur_d.inner.num_days(), 3);

        // Test mixed units
        let dur_mixed = parse_duration("1h 30m").unwrap();
        assert_eq!(dur_mixed.inner.num_minutes(), 90);

        // Test with extra spaces
        let dur_spaced = parse_duration("  1h   30m  ").unwrap();
        assert_eq!(dur_spaced.inner.num_minutes(), 90);
    }

    #[test]
    fn test_duration_arithmetic_non_negative() {
        let dur1 = DurationWrapper::from_hours(2);
        let dur2 = DurationWrapper::from_hours(3);

        // Subtraction that would normally be negative becomes positive
        let result = DurationWrapper::new((dur1.inner - dur2.inner).abs());
        assert_eq!(result.inner.num_hours(), 1);
    }

    #[test]
    fn test_datetime_wrapper_display() {
        let dt = parse_timestamp("2023-07-04T12:34:56Z", None, None).unwrap();
        let display_str = dt.to_string();
        assert!(display_str.contains("2023-07-04"));
        assert!(display_str.contains("12:34:56"));
    }

    #[test]
    fn test_datetime_component_access() {
        let dt = parse_timestamp("2023-07-04T12:34:56Z", None, None).unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);
    }

    #[test]
    fn test_duration_conversions() {
        let dur = DurationWrapper::from_hours(2);
        assert_eq!(dur.inner.num_hours(), 2);
        assert_eq!(dur.inner.num_minutes(), 120);
        assert_eq!(dur.inner.num_seconds(), 7200);

        let dur_ms = DurationWrapper::from_milliseconds(5000);
        assert_eq!(dur_ms.inner.num_seconds(), 5);
    }

    #[test]
    fn test_rfc3339_and_rfc2822_parsing() {
        // RFC3339
        let rfc3339_result = parse_timestamp("2023-07-04T12:34:56+00:00", None, None);
        assert!(rfc3339_result.is_ok());

        // RFC2822
        let rfc2822_result = parse_timestamp("Tue, 04 Jul 2023 12:34:56 +0000", None, None);
        assert!(rfc2822_result.is_ok());
    }

    #[test]
    fn test_standard_format_parsing() {
        // Apache log format
        let apache_result = parse_timestamp("04/Jul/2023:12:34:56 +0000", None, None);
        assert!(apache_result.is_ok());

        // Common log format
        let common_result = parse_timestamp("2023-07-04 12:34:56", None, None);
        assert!(common_result.is_ok());

        // ISO 8601 variants
        let iso_result = parse_timestamp("2023-07-04T12:34:56.123Z", None, None);
        assert!(iso_result.is_ok());
    }

    #[test]
    fn test_large_duration_values() {
        // Test very large durations
        let large_dur = DurationWrapper::from_days(365);
        assert_eq!(large_dur.inner.num_days(), 365);

        // Test nanosecond precision
        let nano_dur = DurationWrapper::from_nanoseconds(1_000_000_000);
        assert_eq!(nano_dur.inner.num_seconds(), 1);
    }

    #[test]
    fn test_boundary_conditions() {
        // Test leap year
        let leap_year_result = parse_timestamp("2024-02-29T12:00:00Z", None, None);
        assert!(leap_year_result.is_ok());

        // Test non-leap year (should fail)
        let non_leap_result = parse_timestamp("2023-02-29T12:00:00Z", None, None);
        assert!(non_leap_result.is_err());

        // Test year boundaries
        let y2k_result = parse_timestamp("2000-01-01T00:00:00Z", None, None);
        assert!(y2k_result.is_ok());
    }

    #[test]
    fn test_unix_timestamp_parsing() {
        // Test Unix timestamp in seconds (10 digits)
        let unix_seconds = parse_timestamp("1735566123", None, None);
        assert!(unix_seconds.is_ok());
        let dt = unix_seconds.unwrap();
        assert_eq!(dt.inner.year(), 2024);
        assert_eq!(dt.inner.month(), 12);
        assert_eq!(dt.inner.day(), 30);

        // Test Unix timestamp in milliseconds (13 digits)
        let unix_millis = parse_timestamp("1735566123000", None, None);
        assert!(unix_millis.is_ok());
        let dt = unix_millis.unwrap();
        assert_eq!(dt.inner.year(), 2024);
        assert_eq!(dt.inner.month(), 12);
        assert_eq!(dt.inner.day(), 30);

        // Test Unix timestamp in microseconds (16 digits)
        let unix_micros = parse_timestamp("1735566123000000", None, None);
        assert!(unix_micros.is_ok());
        let dt = unix_micros.unwrap();
        assert_eq!(dt.inner.year(), 2024);
        assert_eq!(dt.inner.month(), 12);
        assert_eq!(dt.inner.day(), 30);

        // Test Unix timestamp in nanoseconds (19 digits)
        let unix_nanos = parse_timestamp("1735566123000000000", None, None);
        assert!(unix_nanos.is_ok());
        let dt = unix_nanos.unwrap();
        assert_eq!(dt.inner.year(), 2024);
        assert_eq!(dt.inner.month(), 12);
        assert_eq!(dt.inner.day(), 30);

        // Test invalid Unix timestamp (wrong length)
        let invalid_unix = parse_timestamp("12345", None, None);
        assert!(invalid_unix.is_err());

        // Test Unix timestamp with non-numeric characters
        let invalid_chars = parse_timestamp("1735566123a", None, None);
        assert!(invalid_chars.is_err());
    }

    #[test]
    fn test_new_timestamp_formats() {
        // Test Python logging format with comma separator
        let python_result = parse_timestamp("2023-07-04 12:34:56,123", None, None);
        assert!(python_result.is_ok());
        let dt = python_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Note: Ambiguous formats like "7/4/2023 12:34:56 PM" are not supported
        // in automatic parsing due to month/day ambiguity. Use explicit format instead.

        // Test MySQL legacy format
        let mysql_result = parse_timestamp("230704 12:34:56", None, None);
        assert!(mysql_result.is_ok());
        let dt = mysql_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Test Nginx error log format
        let nginx_result = parse_timestamp("2023/07/04 12:34:56", None, None);
        assert!(nginx_result.is_ok());
        let dt = nginx_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Test BSD syslog with year
        let bsd_result = parse_timestamp("Jul 04 2023 12:34:56", None, None);
        assert!(bsd_result.is_ok());
        let dt = bsd_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Test Java SimpleDateFormat
        let java_result = parse_timestamp("Jul 04, 2023 12:34:56 PM", None, None);
        assert!(java_result.is_ok());
        let dt = java_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Test German format with dots
        let german_result = parse_timestamp("04.07.2023 12:34:56", None, None);
        assert!(german_result.is_ok());
        let dt = german_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);
    }

    #[test]
    fn test_klp_inspired_formats() {
        // Test space-separated ISO 8601 with fractional seconds and Z
        let space_iso_frac_z = parse_timestamp("2023-07-04 12:34:56.123Z", None, None);
        assert!(space_iso_frac_z.is_ok());
        let dt = space_iso_frac_z.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Test space-separated ISO 8601 without fractional seconds but with Z
        let space_iso_z = parse_timestamp("2023-07-04 12:34:56Z", None, None);
        assert!(space_iso_z.is_ok());
        let dt = space_iso_z.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Test space-separated ISO 8601 with timezone offset
        let space_iso_tz = parse_timestamp("2023-07-04 12:34:56+0000", None, None);
        assert!(space_iso_tz.is_ok());
        let dt = space_iso_tz.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Test space-separated ISO 8601 with fractional seconds and timezone
        let space_iso_frac_tz = parse_timestamp("2023-07-04 12:34:56.123+0000", None, None);
        assert!(space_iso_frac_tz.is_ok());
        let dt = space_iso_frac_tz.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);

        // Test classic Unix timestamp with weekday
        // July 4th, 2023 was a Tuesday
        let unix_weekday = parse_timestamp("Tue Jul 04 12:34:56 2023", None, None);
        assert!(unix_weekday.is_ok());
        let dt = unix_weekday.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);
    }

    #[test]
    fn test_oracle_format_parsing() {
        // Test Oracle format - this one is complex and may need adjustment
        let oracle_result = parse_timestamp("04-JUL-23 12:34:56.123 PM", None, None);
        assert!(oracle_result.is_ok());
        let dt = oracle_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);
        assert_eq!(dt.inner.minute(), 34);
        assert_eq!(dt.inner.second(), 56);
    }

    #[test]
    fn test_adaptive_parsing_in_rhai() {
        // Test that the Rhai parse_timestamp function benefits from adaptive parsing
        // by parsing similar formats multiple times
        
        // First parse - should learn the format
        let result1 = parse_timestamp("2023-07-04 12:34:56", None, None);
        assert!(result1.is_ok());
        let dt1 = result1.unwrap();
        assert_eq!(dt1.inner.year(), 2023);
        assert_eq!(dt1.inner.month(), 7);
        assert_eq!(dt1.inner.day(), 4);
        
        // Second parse with same format - should be faster due to learning
        let result2 = parse_timestamp("2023-07-05 13:45:07", None, None);
        assert!(result2.is_ok());
        let dt2 = result2.unwrap();
        assert_eq!(dt2.inner.year(), 2023);
        assert_eq!(dt2.inner.month(), 7);
        assert_eq!(dt2.inner.day(), 5);
        
        // Third parse with same format - should still work efficiently
        let result3 = parse_timestamp("2023-07-06 14:56:08", None, None);
        assert!(result3.is_ok());
        let dt3 = result3.unwrap();
        assert_eq!(dt3.inner.year(), 2023);
        assert_eq!(dt3.inner.month(), 7);
        assert_eq!(dt3.inner.day(), 6);
    }

    #[test]
    fn test_explicit_format_for_ambiguous_dates() {
        // Test US format (M/D/YYYY) with explicit format
        let us_result = parse_timestamp("7/4/2023 12:34:56 PM", Some("%m/%d/%Y %I:%M:%S %p"), None);
        assert!(us_result.is_ok());
        let dt = us_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 7);  // July (US interpretation)
        assert_eq!(dt.inner.day(), 4);
        assert_eq!(dt.inner.hour(), 12);

        // Test European format (D/M/YYYY) with explicit format
        let eu_result = parse_timestamp("7/4/2023 12:34:56", Some("%d/%m/%Y %H:%M:%S"), None);
        assert!(eu_result.is_ok());
        let dt = eu_result.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 4);  // April (European interpretation)
        assert_eq!(dt.inner.day(), 7);
        assert_eq!(dt.inner.hour(), 12);

        // Test that ambiguous formats fail without explicit format
        let ambiguous_result = parse_timestamp("7/4/2023 12:34:56 PM", None, None);
        assert!(ambiguous_result.is_err(), "Ambiguous date format should fail without explicit format");

        // Test Windows Event Log format variations with explicit format
        let windows_us = parse_timestamp("12/31/2023 11:59:59 PM", Some("%m/%d/%Y %I:%M:%S %p"), None);
        assert!(windows_us.is_ok());
        let dt = windows_us.unwrap();
        assert_eq!(dt.inner.year(), 2023);
        assert_eq!(dt.inner.month(), 12);
        assert_eq!(dt.inner.day(), 31);
        assert_eq!(dt.inner.hour(), 23);  // 11 PM = 23:00
    }

    #[test]
    fn test_unix_timestamp_edge_cases() {
        // Test earliest Unix timestamp (1970-01-01 00:00:00)
        let epoch_result = parse_timestamp("0", None, None);
        assert!(epoch_result.is_err()); // Single digit should fail

        let epoch_result = parse_timestamp("0000000000", None, None);
        assert!(epoch_result.is_ok());
        let dt = epoch_result.unwrap();
        assert_eq!(dt.inner.year(), 1970);
        assert_eq!(dt.inner.month(), 1);
        assert_eq!(dt.inner.day(), 1);

        // Test year 2038 problem boundary
        let y2038_result = parse_timestamp("2147483647", None, None);
        assert!(y2038_result.is_ok());
        let dt = y2038_result.unwrap();
        assert_eq!(dt.inner.year(), 2038);
        assert_eq!(dt.inner.month(), 1);
        assert_eq!(dt.inner.day(), 19);

        // Test millisecond precision
        let millis_result = parse_timestamp("1735566123456", None, None);
        assert!(millis_result.is_ok());
        let dt = millis_result.unwrap();
        assert_eq!(dt.inner.year(), 2024);
        assert_eq!(dt.inner.month(), 12);
        assert_eq!(dt.inner.day(), 30);
    }
}
