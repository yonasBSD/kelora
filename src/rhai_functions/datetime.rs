use chrono::{DateTime, Datelike, Duration, Local, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use rhai::{Engine, EvalAltResult, Position};
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
    }

    // Try standard formats
    let standard_formats = [
        "%Y-%m-%dT%H:%M:%S%.fZ",  // ISO 8601 with fractional seconds
        "%Y-%m-%dT%H:%M:%SZ",     // ISO 8601 without fractional seconds
        "%Y-%m-%dT%H:%M:%S%z",    // ISO 8601 with timezone offset
        "%Y-%m-%dT%H:%M:%S%.f%z", // ISO 8601 with fractional seconds and timezone
        "%Y-%m-%d %H:%M:%S%.f",   // Common log format with fractional seconds
        "%Y-%m-%d %H:%M:%S",      // Common log format
        "%d/%b/%Y:%H:%M:%S %z",   // Apache log format
        "%b %d %H:%M:%S",         // Syslog format
    ];

    for fmt in &standard_formats {
        if let Ok(naive_dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(DateTimeWrapper::new(
                default_tz.from_utc_datetime(&naive_dt),
            ));
        }
        // Also try with timezone-aware parsing
        if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
            return Ok(DateTimeWrapper::new(dt.with_timezone(&Tz::UTC)));
        }
    }

    // Try RFC3339 parsing
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(DateTimeWrapper::new(dt.with_timezone(&Tz::UTC)));
    }

    // Try RFC2822 parsing
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Ok(DateTimeWrapper::new(dt.with_timezone(&Tz::UTC)));
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
}
