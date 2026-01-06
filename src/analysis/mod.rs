//! Smart log analysis module
//!
//! Provides sampling-based analysis of log files to help users understand
//! their data and generate appropriate CLI options.
//!
//! This module operates independently of the streaming pipeline, reading
//! files directly with configurable sampling strategies.

mod profiler;
mod report;
mod sampler;

// These exports are used by the module but some may be unused externally
// They're part of the public API for future extensions
#[allow(unused_imports)]
pub use profiler::{FieldProfile, FieldType, LogProfile};
pub use report::AnalysisReport;
#[allow(unused_imports)]
pub use sampler::{Sample, SamplingStrategy};

use crate::config::InputFormat as ConfigInputFormat;
use crate::event::Event;
use crate::parsers::auto_detect::detect_format;
use crate::parsers::{
    CefParser, CombinedParser, JsonlParser, LineParser, LogfmtParser, SyslogParser,
};
use crate::pipeline::EventParser;
use anyhow::Result;
use std::path::Path;

/// Configuration for the analysis mode
#[derive(Debug, Clone)]
pub struct AnalyzeConfig {
    /// Number of lines to sample (default: 1000)
    pub sample_size: usize,
    /// Sampling strategy
    pub strategy: SamplingStrategy,
    /// Input format override (None = auto-detect)
    pub format: Option<String>,
}

impl Default for AnalyzeConfig {
    fn default() -> Self {
        Self {
            sample_size: 1000,
            strategy: SamplingStrategy::Stratified {
                head: 400,
                middle: 300,
                tail: 300,
            },
            format: None,
        }
    }
}

/// Main entry point for log analysis
pub fn analyze(paths: &[&Path], config: &AnalyzeConfig) -> Result<AnalysisReport> {
    // Sample lines from all input files
    let sample = sampler::sample_files(paths, &config.strategy, config.sample_size)?;

    if sample.lines.is_empty() {
        anyhow::bail!("No lines could be sampled from the input files");
    }

    // Detect format from sample
    let format = if let Some(ref fmt) = config.format {
        fmt.clone()
    } else {
        detect_format_from_sample(&sample.lines)?
    };

    // Parse sampled lines into events
    let events = parse_sample(&sample.lines, &format)?;

    // Profile the fields
    let profile = profiler::profile_events(&events, &sample);

    // Generate report with suggestions
    Ok(report::generate_report(profile, &format, &sample))
}

/// Detect format from sample lines
fn detect_format_from_sample(lines: &[String]) -> Result<String> {
    for line in lines.iter().take(10) {
        if let Ok(format) = detect_format(line) {
            return Ok(format_to_string(format));
        }
    }
    Ok("line".to_string()) // Fallback
}

/// Convert ConfigInputFormat to string
fn format_to_string(format: ConfigInputFormat) -> String {
    match format {
        ConfigInputFormat::Auto => "auto".to_string(),
        ConfigInputFormat::Json => "json".to_string(),
        ConfigInputFormat::Logfmt => "logfmt".to_string(),
        ConfigInputFormat::Syslog => "syslog".to_string(),
        ConfigInputFormat::Cef => "cef".to_string(),
        ConfigInputFormat::Csv(_) => "csv".to_string(),
        ConfigInputFormat::Tsv(_) => "tsv".to_string(),
        ConfigInputFormat::Csvnh => "csv".to_string(),
        ConfigInputFormat::Tsvnh => "tsv".to_string(),
        ConfigInputFormat::Combined => "combined".to_string(),
        ConfigInputFormat::Line => "line".to_string(),
        ConfigInputFormat::Raw => "raw".to_string(),
        ConfigInputFormat::Regex(_) => "line".to_string(),
        ConfigInputFormat::Cols(_) => "line".to_string(),
    }
}

/// Parse sample lines into events using the appropriate parser
fn parse_sample(lines: &[String], format: &str) -> Result<Vec<Event>> {
    let mut events = Vec::new();

    // Create the appropriate parser based on format
    // Some parsers can fail to construct (regex compilation), so we fall back to LineParser
    let parser: Box<dyn EventParser> = match format.to_lowercase().as_str() {
        "json" => Box::new(JsonlParser::new()),
        "logfmt" => Box::new(LogfmtParser::new()),
        "syslog" => match SyslogParser::new() {
            Ok(p) => Box::new(p),
            Err(_) => Box::new(LineParser::new()),
        },
        "combined" => match CombinedParser::new() {
            Ok(p) => Box::new(p),
            Err(_) => Box::new(LineParser::new()),
        },
        "cef" => Box::new(CefParser::new()),
        _ => Box::new(LineParser::new()),
    };

    for line in lines {
        match parser.parse(line) {
            Ok(event) => events.push(event),
            Err(_) => continue, // Skip unparseable lines in analysis
        }
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_reasonable_values() {
        let config = AnalyzeConfig::default();
        assert_eq!(config.sample_size, 1000);
        assert!(matches!(
            config.strategy,
            SamplingStrategy::Stratified { .. }
        ));
    }
}
