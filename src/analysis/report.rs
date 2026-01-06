//! Report generation for log analysis
//!
//! Formats analysis results into human-readable output with
//! actionable CLI suggestions.

use super::profiler::LogProfile;
use super::sampler::Sample;
use crate::rhai_functions::datetime::DurationWrapper;

/// Complete analysis report
#[derive(Debug, Clone)]
pub struct AnalysisReport {
    pub profile: LogProfile,
    pub format: String,
    pub sample_info: SampleInfo,
    pub suggestions: Vec<Suggestion>,
}

/// Information about the sample
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SampleInfo {
    pub lines_sampled: usize,
    pub lines_estimated: usize,
    pub coverage_percent: f64,
    pub files_sampled: usize,
}

/// A suggested CLI command
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub description: String,
    pub command: String,
    pub category: SuggestionCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionCategory {
    Filter,
    Grouping,
    NumericAnalysis,
    TimeRange,
}

/// Generate the complete analysis report
pub fn generate_report(profile: LogProfile, format: &str, sample: &Sample) -> AnalysisReport {
    let sample_info = SampleInfo {
        lines_sampled: sample.lines.len(),
        lines_estimated: sample.total_lines_estimate,
        coverage_percent: sample.coverage_percent(),
        files_sampled: sample.files_sampled,
    };

    let suggestions = generate_suggestions(&profile);

    AnalysisReport {
        profile,
        format: format.to_string(),
        sample_info,
        suggestions,
    }
}

/// Generate CLI suggestions based on the profile
fn generate_suggestions(profile: &LogProfile) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    // Level-based filtering
    if let Some(ref levels) = profile.level_profile {
        if levels.error_rate() > 0.0 {
            let error_levels: Vec<_> = levels
                .counts
                .keys()
                .filter(|k| {
                    let l = k.to_lowercase();
                    l == "error" || l == "err" || l == "fatal" || l == "critical"
                })
                .cloned()
                .collect();

            if !error_levels.is_empty() {
                suggestions.push(Suggestion {
                    description: "Filter to errors only".to_string(),
                    command: format!("-l {}", error_levels.join(",")),
                    category: SuggestionCategory::Filter,
                });
            }
        }
    }

    // Groupable field suggestions
    for field in profile.filterable_fields() {
        if field.top_values.len() >= 2 {
            // Suggest grouping by this field
            suggestions.push(Suggestion {
                description: format!("Group by {} ({} values)", field.name, field.cardinality),
                command: format!(
                    "-m --exec \"track_count('by_{}', e.{})\"",
                    field.name, field.name
                ),
                category: SuggestionCategory::Grouping,
            });

            // If there's an interesting value, suggest filtering
            if let Some((top_val, count)) = field.top_values.first() {
                let pct = (*count as f64 / field.total_count as f64 * 100.0) as usize;
                if pct < 90 {
                    // Don't suggest if one value dominates
                    suggestions.push(Suggestion {
                        description: format!("Filter {} = '{}' ({}%)", field.name, top_val, pct),
                        command: format!("--filter \"e.{} == '{}'\"", field.name, top_val),
                        category: SuggestionCategory::Filter,
                    });
                }
            }
        }
    }

    // Numeric field analysis
    for field in profile.numeric_fields() {
        if let Some(ref stats) = field.numeric_stats {
            suggestions.push(Suggestion {
                description: format!(
                    "Analyze {} distribution (p50={:.1}, p95={:.1})",
                    field.name, stats.p50, stats.p95
                ),
                command: format!(
                    "-m --exec \"track_percentiles('{}', e.{})\"",
                    field.name, field.name
                ),
                category: SuggestionCategory::NumericAnalysis,
            });

            // Suggest filtering for outliers
            if stats.p95 > stats.p50 * 2.0 {
                suggestions.push(Suggestion {
                    description: format!("Filter {} > p95 (outliers)", field.name),
                    command: format!("--filter \"e.{} > {}\"", field.name, stats.p95 as i64),
                    category: SuggestionCategory::Filter,
                });
            }
        }
    }

    // Time range suggestions
    if let (Some(first), Some(last)) = (
        profile.time_profile.first_timestamp,
        profile.time_profile.last_timestamp,
    ) {
        if first != last {
            // Suggest last hour if time span is larger
            let duration_secs = (last - first).num_seconds();
            if duration_secs > 3600 {
                suggestions.push(Suggestion {
                    description: "Filter to last hour".to_string(),
                    command: "--since 1h".to_string(),
                    category: SuggestionCategory::TimeRange,
                });
            }
        }
    }

    suggestions
}

impl AnalysisReport {
    /// Format the report as a human-readable string
    pub fn format(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str("📊 Log Analysis Report\n\n");

        // Sample info
        output.push_str(&format!(
            "Format: {} ({})\n",
            self.format,
            if self.format == "auto" {
                "auto-detected"
            } else {
                "specified"
            }
        ));

        if self.sample_info.lines_estimated > self.sample_info.lines_sampled {
            output.push_str(&format!(
                "Sampled: {} of ~{} lines ({:.1}%)\n",
                self.sample_info.lines_sampled,
                self.sample_info.lines_estimated,
                self.sample_info.coverage_percent
            ));
        } else {
            output.push_str(&format!("Lines: {}\n", self.sample_info.lines_sampled));
        }

        output.push_str(&format!(
            "Events parsed: {} ({} errors)\n",
            self.profile.total_events, self.profile.parse_errors
        ));

        // Time range
        if let (Some(first), Some(last)) = (
            self.profile.time_profile.first_timestamp,
            self.profile.time_profile.last_timestamp,
        ) {
            if first == last {
                output.push_str(&format!(
                    "Time: {} (single point)\n",
                    first.format("%Y-%m-%d %H:%M:%S")
                ));
            } else {
                let duration = last - first;
                let wrapper = DurationWrapper::new(duration);
                output.push_str(&format!(
                    "Time span: {} → {} ({})\n",
                    first.format("%Y-%m-%d %H:%M:%S"),
                    last.format("%Y-%m-%d %H:%M:%S"),
                    wrapper
                ));
            }
        }

        output.push('\n');

        // Level distribution
        if let Some(ref levels) = self.profile.level_profile {
            output.push_str("📈 Level Distribution:\n");
            for (level, count) in &levels.counts {
                let pct = (*count as f64 / levels.total as f64) * 100.0;
                let bar = "█".repeat((pct / 5.0) as usize);
                output.push_str(&format!(
                    "  {:12} {:>5} ({:5.1}%) {}\n",
                    level, count, pct, bar
                ));
            }
            output.push('\n');
        }

        // Field summary
        output.push_str("📋 Field Summary:\n");

        // Groupable fields
        let groupable: Vec<_> = self.profile.filterable_fields();
        if !groupable.is_empty() {
            output.push_str("  Good for grouping/filtering (low cardinality):\n");
            for field in groupable.iter().take(5) {
                let values_str = field
                    .top_values
                    .iter()
                    .take(4)
                    .map(|(v, _)| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!(
                    "    {:20} {} values  [{}]\n",
                    field.name, field.cardinality, values_str
                ));
            }
            output.push('\n');
        }

        // Numeric fields
        let numeric: Vec<_> = self.profile.numeric_fields();
        if !numeric.is_empty() {
            output.push_str("  Numeric fields (analyzable):\n");
            for field in numeric.iter().take(5) {
                if let Some(ref stats) = field.numeric_stats {
                    output.push_str(&format!(
                        "    {:20} range: {:.0}-{:.0}  p50: {:.1}  p95: {:.1}\n",
                        field.name, stats.min, stats.max, stats.p50, stats.p95
                    ));
                }
            }
            output.push('\n');
        }

        // Identifier fields
        let identifiers: Vec<_> = self.profile.identifier_fields();
        if !identifiers.is_empty() {
            output.push_str("  High-cardinality (likely identifiers):\n");
            for field in identifiers.iter().take(5) {
                let uniqueness = field.cardinality as f64 / field.total_count.max(1) as f64 * 100.0;
                output.push_str(&format!(
                    "    {:20} {:.0}% unique\n",
                    field.name, uniqueness
                ));
            }
            output.push('\n');
        }

        // All other fields (brief)
        let other_fields: Vec<_> = self
            .profile
            .fields
            .values()
            .filter(|f| {
                !f.is_good_for_grouping() && !f.is_likely_identifier() && f.numeric_stats.is_none()
            })
            .take(10)
            .collect();

        if !other_fields.is_empty() {
            output.push_str("  Other fields:\n");
            for field in other_fields {
                let presence = field.presence_rate() * 100.0;
                output.push_str(&format!(
                    "    {:20} {} ({:.0}% present)\n",
                    field.name, field.field_type, presence
                ));
            }
            output.push('\n');
        }

        // Suggestions
        if !self.suggestions.is_empty() {
            output.push_str("💡 Suggested Commands:\n");

            // Group by category
            let filters: Vec<_> = self
                .suggestions
                .iter()
                .filter(|s| s.category == SuggestionCategory::Filter)
                .collect();
            let grouping: Vec<_> = self
                .suggestions
                .iter()
                .filter(|s| s.category == SuggestionCategory::Grouping)
                .collect();
            let numeric_analysis: Vec<_> = self
                .suggestions
                .iter()
                .filter(|s| s.category == SuggestionCategory::NumericAnalysis)
                .collect();
            let time_range: Vec<_> = self
                .suggestions
                .iter()
                .filter(|s| s.category == SuggestionCategory::TimeRange)
                .collect();

            if !filters.is_empty() {
                output.push_str("  Filtering:\n");
                for s in filters.iter().take(3) {
                    output.push_str(&format!("    # {}\n", s.description));
                    output.push_str(&format!("    kelora {} <file>\n\n", s.command));
                }
            }

            if !grouping.is_empty() {
                output.push_str("  Grouping/Counting:\n");
                for s in grouping.iter().take(3) {
                    output.push_str(&format!("    # {}\n", s.description));
                    output.push_str(&format!("    kelora {} <file>\n\n", s.command));
                }
            }

            if !numeric_analysis.is_empty() {
                output.push_str("  Numeric Analysis:\n");
                for s in numeric_analysis.iter().take(2) {
                    output.push_str(&format!("    # {}\n", s.description));
                    output.push_str(&format!("    kelora {} <file>\n\n", s.command));
                }
            }

            if !time_range.is_empty() {
                output.push_str("  Time Range:\n");
                for s in time_range.iter().take(2) {
                    output.push_str(&format!("    # {}\n", s.description));
                    output.push_str(&format!("    kelora {} <file>\n\n", s.command));
                }
            }
        }

        output.trim_end().to_string()
    }

    /// Format the report as JSON
    #[allow(dead_code)]
    pub fn format_json(&self) -> String {
        let mut obj = serde_json::Map::new();

        // Basic info
        obj.insert("format".to_string(), serde_json::json!(self.format));
        obj.insert(
            "lines_sampled".to_string(),
            serde_json::json!(self.sample_info.lines_sampled),
        );
        obj.insert(
            "lines_estimated".to_string(),
            serde_json::json!(self.sample_info.lines_estimated),
        );
        obj.insert(
            "events_parsed".to_string(),
            serde_json::json!(self.profile.total_events),
        );
        obj.insert(
            "parse_errors".to_string(),
            serde_json::json!(self.profile.parse_errors),
        );

        // Time range
        if let Some(first) = self.profile.time_profile.first_timestamp {
            obj.insert(
                "first_timestamp".to_string(),
                serde_json::json!(first.to_rfc3339()),
            );
        }
        if let Some(last) = self.profile.time_profile.last_timestamp {
            obj.insert(
                "last_timestamp".to_string(),
                serde_json::json!(last.to_rfc3339()),
            );
        }

        // Levels
        if let Some(ref levels) = self.profile.level_profile {
            let levels_obj: serde_json::Map<_, _> = levels
                .counts
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                .collect();
            obj.insert("levels".to_string(), serde_json::Value::Object(levels_obj));
        }

        // Fields
        let fields: Vec<serde_json::Value> = self
            .profile
            .fields
            .values()
            .map(|f| {
                let mut field_obj = serde_json::json!({
                    "name": f.name,
                    "type": f.field_type.to_string(),
                    "cardinality": f.cardinality,
                    "presence_rate": f.presence_rate(),
                });

                if let Some(ref stats) = f.numeric_stats {
                    field_obj["numeric_stats"] = serde_json::json!({
                        "min": stats.min,
                        "max": stats.max,
                        "p50": stats.p50,
                        "p95": stats.p95,
                        "p99": stats.p99,
                    });
                }

                if !f.top_values.is_empty() {
                    let top: Vec<_> = f
                        .top_values
                        .iter()
                        .take(5)
                        .map(|(v, c)| serde_json::json!({"value": v, "count": c}))
                        .collect();
                    field_obj["top_values"] = serde_json::json!(top);
                }

                field_obj
            })
            .collect();
        obj.insert("fields".to_string(), serde_json::json!(fields));

        // Suggestions
        let suggestions: Vec<serde_json::Value> = self
            .suggestions
            .iter()
            .map(|s| {
                serde_json::json!({
                    "description": s.description,
                    "command": s.command,
                })
            })
            .collect();
        obj.insert("suggestions".to_string(), serde_json::json!(suggestions));

        serde_json::to_string_pretty(&serde_json::Value::Object(obj))
            .unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::profiler::{FieldProfile, FieldType, LevelProfile, TimeProfile};
    use indexmap::IndexMap;

    fn create_test_profile() -> LogProfile {
        let mut fields = IndexMap::new();
        fields.insert(
            "level".to_string(),
            FieldProfile {
                name: "level".to_string(),
                field_type: FieldType::String,
                total_count: 100,
                null_count: 0,
                cardinality: 4,
                top_values: vec![
                    ("INFO".to_string(), 60),
                    ("WARN".to_string(), 25),
                    ("ERROR".to_string(), 15),
                ],
                numeric_stats: None,
            },
        );

        let mut level_counts = IndexMap::new();
        level_counts.insert("INFO".to_string(), 60);
        level_counts.insert("WARN".to_string(), 25);
        level_counts.insert("ERROR".to_string(), 15);

        LogProfile {
            fields,
            time_profile: TimeProfile {
                first_timestamp: None,
                last_timestamp: None,
                events_with_timestamp: 0,
                events_without_timestamp: 100,
            },
            level_profile: Some(LevelProfile {
                counts: level_counts,
                total: 100,
            }),
            total_events: 100,
            parse_errors: 0,
        }
    }

    #[test]
    fn generates_suggestions_for_levels() {
        let profile = create_test_profile();
        let suggestions = generate_suggestions(&profile);

        let has_error_filter = suggestions
            .iter()
            .any(|s| s.command.contains("-l") && s.description.contains("error"));
        assert!(has_error_filter);
    }

    #[test]
    fn report_formats_without_panic() {
        let profile = create_test_profile();
        let sample = Sample {
            lines: vec!["test".to_string(); 100],
            total_lines_estimate: 100,
            files_sampled: 1,
            truncated: false,
        };

        let report = generate_report(profile, "json", &sample);
        let output = report.format();

        assert!(output.contains("Log Analysis Report"));
        assert!(output.contains("json"));
    }

    #[test]
    fn json_output_is_valid() {
        let profile = create_test_profile();
        let sample = Sample {
            lines: vec!["test".to_string(); 100],
            total_lines_estimate: 100,
            files_sampled: 1,
            truncated: false,
        };

        let report = generate_report(profile, "json", &sample);
        let json_output = report.format_json();

        // Should parse as valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
        assert!(parsed.get("format").is_some());
        assert!(parsed.get("fields").is_some());
    }
}
