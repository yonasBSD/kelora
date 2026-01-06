//! Field profiling for log analysis
//!
//! Analyzes parsed events to determine field types, cardinality,
//! value distributions, and other statistics useful for suggesting
//! CLI options.

use super::sampler::Sample;
use crate::event::Event;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use std::collections::HashMap;
use tdigests::TDigest;

/// Inferred type for a field
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FieldType {
    /// Numeric values (integers or floats)
    Numeric,
    /// Boolean values
    Boolean,
    /// Timestamp values
    Timestamp,
    /// String values (default)
    String,
    /// Mixed types detected
    Mixed,
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::Numeric => write!(f, "numeric"),
            FieldType::Boolean => write!(f, "boolean"),
            FieldType::Timestamp => write!(f, "timestamp"),
            FieldType::String => write!(f, "string"),
            FieldType::Mixed => write!(f, "mixed"),
        }
    }
}

/// Profile of a single field
#[derive(Debug, Clone)]
pub struct FieldProfile {
    pub name: String,
    pub field_type: FieldType,
    pub total_count: usize,
    pub null_count: usize,
    pub cardinality: usize,

    // Top values (for low-cardinality fields)
    pub top_values: Vec<(String, usize)>,

    // Numeric stats (if applicable)
    pub numeric_stats: Option<NumericStats>,
}

/// Statistics for numeric fields
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NumericStats {
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
}

impl NumericStats {
    fn from_digest(digest: &TDigest) -> Self {
        Self {
            min: digest.estimate_quantile(0.0),
            max: digest.estimate_quantile(1.0),
            p50: digest.estimate_quantile(0.50),
            p90: digest.estimate_quantile(0.90),
            p95: digest.estimate_quantile(0.95),
            p99: digest.estimate_quantile(0.99),
        }
    }
}

impl FieldProfile {
    /// Check if this field is good for grouping (low cardinality)
    pub fn is_good_for_grouping(&self) -> bool {
        self.cardinality > 1 && self.cardinality <= 20 && self.presence_rate() > 0.5
    }

    /// Check if this field is likely an identifier (high cardinality)
    pub fn is_likely_identifier(&self) -> bool {
        let presence = self.presence_rate();
        let uniqueness = self.cardinality as f64 / self.total_count.max(1) as f64;
        uniqueness > 0.9 && presence > 0.5
    }

    /// Presence rate (non-null ratio)
    pub fn presence_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            (self.total_count - self.null_count) as f64 / self.total_count as f64
        }
    }
}

/// Time-based statistics
#[derive(Debug, Clone)]
pub struct TimeProfile {
    pub first_timestamp: Option<DateTime<Utc>>,
    pub last_timestamp: Option<DateTime<Utc>>,
    pub events_with_timestamp: usize,
    pub events_without_timestamp: usize,
}

impl TimeProfile {
    #[allow(dead_code)]
    pub fn duration_seconds(&self) -> Option<i64> {
        match (self.first_timestamp, self.last_timestamp) {
            (Some(first), Some(last)) => Some((last - first).num_seconds()),
            _ => None,
        }
    }
}

/// Level (severity) statistics
#[derive(Debug, Clone)]
pub struct LevelProfile {
    pub counts: IndexMap<String, usize>,
    pub total: usize,
}

impl LevelProfile {
    pub fn error_rate(&self) -> f64 {
        let errors: usize = self
            .counts
            .iter()
            .filter(|(k, _)| {
                let lower = k.to_lowercase();
                lower == "error" || lower == "err" || lower == "fatal" || lower == "critical"
            })
            .map(|(_, v)| *v)
            .sum();

        if self.total == 0 {
            0.0
        } else {
            errors as f64 / self.total as f64
        }
    }
}

/// Complete profile of the log data
#[derive(Debug, Clone)]
pub struct LogProfile {
    pub fields: IndexMap<String, FieldProfile>,
    pub time_profile: TimeProfile,
    pub level_profile: Option<LevelProfile>,
    pub total_events: usize,
    pub parse_errors: usize,
}

impl LogProfile {
    /// Get fields suitable for filtering (low cardinality, good presence)
    pub fn filterable_fields(&self) -> Vec<&FieldProfile> {
        self.fields
            .values()
            .filter(|f| f.is_good_for_grouping())
            .collect()
    }

    /// Get numeric fields suitable for analysis
    pub fn numeric_fields(&self) -> Vec<&FieldProfile> {
        self.fields
            .values()
            .filter(|f| f.field_type == FieldType::Numeric && f.numeric_stats.is_some())
            .collect()
    }

    /// Get likely identifier fields
    pub fn identifier_fields(&self) -> Vec<&FieldProfile> {
        self.fields
            .values()
            .filter(|f| f.is_likely_identifier())
            .collect()
    }
}

/// Profile parsed events to extract field statistics
pub fn profile_events(events: &[Event], sample: &Sample) -> LogProfile {
    let mut field_values: HashMap<String, Vec<String>> = HashMap::new();
    let mut field_counts: HashMap<String, usize> = HashMap::new();
    let mut time_profile = TimeProfile {
        first_timestamp: None,
        last_timestamp: None,
        events_with_timestamp: 0,
        events_without_timestamp: 0,
    };
    let mut level_counts: IndexMap<String, usize> = IndexMap::new();

    // Common level field names
    let level_field_names = ["level", "severity", "log_level", "loglevel", "lvl"];

    // Collect field values
    for event in events {
        for (key, value) in &event.fields {
            let count = field_counts.entry(key.clone()).or_insert(0);
            *count += 1;

            let values = field_values.entry(key.clone()).or_default();
            let str_value = value_to_string(value);
            if values.len() < 10000 {
                // Limit memory usage
                values.push(str_value.clone());
            }

            // Track levels from common level field names
            let key_lower = key.to_lowercase();
            if level_field_names.contains(&key_lower.as_str()) {
                *level_counts.entry(str_value).or_insert(0) += 1;
            }
        }

        // Track timestamps from parsed_ts
        if let Some(ts) = event.parsed_ts {
            time_profile.events_with_timestamp += 1;
            match time_profile.first_timestamp {
                None => {
                    time_profile.first_timestamp = Some(ts);
                    time_profile.last_timestamp = Some(ts);
                }
                Some(first) => {
                    if ts < first {
                        time_profile.first_timestamp = Some(ts);
                    }
                    if let Some(last) = time_profile.last_timestamp {
                        if ts > last {
                            time_profile.last_timestamp = Some(ts);
                        }
                    }
                }
            }
        } else {
            time_profile.events_without_timestamp += 1;
        }
    }

    // Deduplicate level counts (avoid double counting from field iteration)
    let _ = sample; // Used for parse error calculation

    // Build field profiles
    let mut fields = IndexMap::new();
    for (name, values) in field_values {
        let profile = build_field_profile(&name, &values, events.len());
        fields.insert(name, profile);
    }

    // Sort fields by presence rate (most present first)
    fields.sort_by(|_, a, _, b| {
        b.presence_rate()
            .partial_cmp(&a.presence_rate())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let level_profile = if level_counts.is_empty() {
        None
    } else {
        let total = level_counts.values().sum();
        // Sort by count descending
        level_counts.sort_by(|_, a, _, b| b.cmp(a));
        Some(LevelProfile {
            counts: level_counts,
            total,
        })
    };

    LogProfile {
        fields,
        time_profile,
        level_profile,
        total_events: events.len(),
        parse_errors: sample.lines.len().saturating_sub(events.len()),
    }
}

/// Build a profile for a single field
fn build_field_profile(name: &str, values: &[String], total_events: usize) -> FieldProfile {
    let non_empty: Vec<&String> = values.iter().filter(|v| !v.is_empty()).collect();
    let null_count = total_events - non_empty.len();

    // Count unique values
    let mut value_counts: HashMap<&str, usize> = HashMap::new();
    for v in &non_empty {
        *value_counts.entry(v.as_str()).or_insert(0) += 1;
    }
    let cardinality = value_counts.len();

    // Get top values
    let mut top_values: Vec<(String, usize)> = value_counts
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    top_values.sort_by(|a, b| b.1.cmp(&a.1));
    top_values.truncate(10);

    // Infer type
    let (field_type, numeric_stats) = infer_type_and_stats(&non_empty);

    FieldProfile {
        name: name.to_string(),
        field_type,
        total_count: total_events,
        null_count,
        cardinality,
        top_values,
        numeric_stats,
    }
}

/// Infer field type and compute stats if numeric
fn infer_type_and_stats(values: &[&String]) -> (FieldType, Option<NumericStats>) {
    if values.is_empty() {
        return (FieldType::String, None);
    }

    let sample_size = values.len().min(500);
    let sample: Vec<_> = values.iter().take(sample_size).collect();

    let mut int_count = 0;
    let mut float_count = 0;
    let mut bool_count = 0;
    let mut numeric_values = Vec::new();

    for v in &sample {
        let s = v.trim();

        // Check boolean
        if s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("false") {
            bool_count += 1;
            continue;
        }

        // Check integer
        if s.parse::<i64>().is_ok() {
            int_count += 1;
            if let Ok(n) = s.parse::<f64>() {
                numeric_values.push(n);
            }
            continue;
        }

        // Check float
        if s.parse::<f64>().is_ok() {
            float_count += 1;
            if let Ok(n) = s.parse::<f64>() {
                numeric_values.push(n);
            }
        }
    }

    let total = sample.len() as f64;
    let numeric_rate = (int_count + float_count) as f64 / total;
    let bool_rate = bool_count as f64 / total;

    if bool_rate > 0.9 {
        return (FieldType::Boolean, None);
    }

    if numeric_rate > 0.9 && !numeric_values.is_empty() {
        // Build T-Digest for percentiles
        let digest = TDigest::from_values(numeric_values);
        let stats = NumericStats::from_digest(&digest);
        return (FieldType::Numeric, Some(stats));
    }

    // Check if values look like timestamps
    let timestamp_count = sample.iter().filter(|v| looks_like_timestamp(v)).count();
    if timestamp_count as f64 / total > 0.8 {
        return (FieldType::Timestamp, None);
    }

    (FieldType::String, None)
}

/// Quick check if a string looks like a timestamp
fn looks_like_timestamp(s: &str) -> bool {
    let s = s.trim();

    // ISO 8601 / RFC 3339
    if s.contains('T') && (s.contains('-') || s.contains(':')) {
        return true;
    }

    // Common date patterns
    if s.len() >= 10 {
        let bytes = s.as_bytes();
        // YYYY-MM-DD
        if bytes.len() >= 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[0..4].iter().all(|b| b.is_ascii_digit())
        {
            return true;
        }
    }

    false
}

/// Convert a rhai Dynamic value to string for analysis
fn value_to_string(value: &rhai::Dynamic) -> String {
    if value.is_string() {
        value.clone().into_string().unwrap_or_default()
    } else if value.is_int() {
        value.as_int().map(|i| i.to_string()).unwrap_or_default()
    } else if value.is_float() {
        value.as_float().map(|f| f.to_string()).unwrap_or_default()
    } else if value.is_bool() {
        value.as_bool().map(|b| b.to_string()).unwrap_or_default()
    } else {
        format!("{:?}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_numeric_type() {
        let values: Vec<String> = vec!["1", "2", "3", "4.5", "100"]
            .into_iter()
            .map(String::from)
            .collect();
        let refs: Vec<&String> = values.iter().collect();

        let (field_type, stats) = infer_type_and_stats(&refs);

        assert_eq!(field_type, FieldType::Numeric);
        assert!(stats.is_some());
    }

    #[test]
    fn infers_boolean_type() {
        let values: Vec<String> = vec!["true", "false", "true", "TRUE", "False"]
            .into_iter()
            .map(String::from)
            .collect();
        let refs: Vec<&String> = values.iter().collect();

        let (field_type, _) = infer_type_and_stats(&refs);

        assert_eq!(field_type, FieldType::Boolean);
    }

    #[test]
    fn infers_string_type_for_mixed() {
        let values: Vec<String> = vec!["hello", "world", "123", "foo"]
            .into_iter()
            .map(String::from)
            .collect();
        let refs: Vec<&String> = values.iter().collect();

        let (field_type, _) = infer_type_and_stats(&refs);

        assert_eq!(field_type, FieldType::String);
    }

    #[test]
    fn detects_timestamp_patterns() {
        assert!(looks_like_timestamp("2024-01-15T12:00:00Z"));
        assert!(looks_like_timestamp("2024-01-15"));
        assert!(!looks_like_timestamp("hello"));
        assert!(!looks_like_timestamp("12345"));
    }

    #[test]
    fn field_profile_grouping_detection() {
        let profile = FieldProfile {
            name: "level".to_string(),
            field_type: FieldType::String,
            total_count: 100,
            null_count: 5,
            cardinality: 4,
            top_values: vec![
                ("INFO".to_string(), 50),
                ("WARN".to_string(), 30),
                ("ERROR".to_string(), 15),
            ],
            numeric_stats: None,
        };

        assert!(profile.is_good_for_grouping());
        assert!(!profile.is_likely_identifier());
    }

    #[test]
    fn field_profile_identifier_detection() {
        let profile = FieldProfile {
            name: "request_id".to_string(),
            field_type: FieldType::String,
            total_count: 100,
            null_count: 0,
            cardinality: 99,
            top_values: vec![],
            numeric_stats: None,
        };

        assert!(!profile.is_good_for_grouping());
        assert!(profile.is_likely_identifier());
    }
}
