// CLI-specific types and structures
// This module contains the command-line interface definitions and parsing logic

use crate::config::ScriptStageType;
use anyhow::Result;
use clap::{ArgMatches, Parser};

// CLI types - specific to command-line interface
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InputFormat {
    Auto,
    Json,
    Line,
    Raw,
    Logfmt,
    Syslog,
    Cef,
    Csv,
    Tsv,
    Csvnh,
    Tsvnh,
    Combined,
    Cols,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    #[default]
    Default,
    Json,
    Logfmt,
    Inspect,
    Levelmap,
    Csv,
    Tsv,
    Csvnh,
    Tsvnh,
    None,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum FileOrder {
    Cli,
    Name,
    Mtime,
}

// CLI structure - contains all command-line arguments and options
#[derive(Parser)]
#[command(name = "kelora")]
#[command(about = "A command-line log analysis tool with embedded Rhai scripting")]
#[command(
    long_about = "A command-line log analysis tool with embedded Rhai scripting\n\nMODES:\n  (default)   Sequential processing - best for streaming/interactive use\n  --parallel  Parallel processing - best for high-throughput batch analysis\n\nCOMMON EXAMPLES:\n  kelora access.log --levels error,critical         Filter streaming logs by level (case-insensitive list)\n  kelora nginx.log --format combined --keys ts,msg  Focus output on a comma-separated field list\n  kelora sample.jsonl --metrics --metrics-file stats.json  Capture tracked metrics to JSON\n\nSee also: --help-rhai for scripting stages, --help-functions for the full built-in catalogue"
)]
#[command(author = "Dirk Loss <mail@dirk-loss.de>")]
#[command(version)]
#[command(args_override_self = true)]
pub struct Cli {
    /// Input files (stdin if not specified, or use "-" to explicitly specify stdin)
    pub files: Vec<String>,

    /// Input format. Supports standard formats (json, line, csv, etc.) and cols:<spec> for column parsing.
    /// Example: -f json, -f 'cols:ts(2) level - *msg'
    #[arg(
        short = 'f',
        long = "format",
        default_value = "line",
        help_heading = "Input Options",
        value_parser = parse_format_value
    )]
    pub format: String,

    /// Shortcut for -f json
    #[arg(short = 'j', help_heading = "Input Options", conflicts_with = "format")]
    pub json_input: bool,

    /// File processing order
    #[arg(
        long = "file-order",
        value_enum,
        default_value = "cli",
        help_heading = "Input Options"
    )]
    pub file_order: FileOrder,

    /// Skip the first N input lines
    #[arg(long = "skip-lines", help_heading = "Input Options")]
    pub skip_lines: Option<usize>,

    /// Keep only input lines matching this regex pattern (applied before ignore-lines)
    #[arg(long = "keep-lines", help_heading = "Input Options")]
    pub keep_lines: Option<String>,

    /// Ignore input lines matching this regex pattern
    #[arg(long = "ignore-lines", help_heading = "Input Options")]
    pub ignore_lines: Option<String>,

    /// Custom timestamp field name for parsing
    #[arg(long = "ts-field", help_heading = "Input Options")]
    pub ts_field: Option<String>,

    /// Custom timestamp format for parsing (uses chrono format strings)
    #[arg(long = "ts-format", help_heading = "Input Options")]
    pub ts_format: Option<String>,

    /// Assume timezone for input timestamps without timezone info (default: UTC).
    /// Use 'local' for system local time.
    /// Examples: 'UTC', 'local', 'Europe/Berlin'.
    #[arg(long = "input-tz", help_heading = "Input Options")]
    pub input_tz: Option<String>,

    /// Multi-line event detection strategy
    #[arg(short = 'M', long = "multiline", help_heading = "Input Options")]
    pub multiline: Option<String>,

    /// Extract text before separator to specified field (runs before parsing)
    #[arg(long = "extract-prefix", help_heading = "Input Options")]
    pub extract_prefix: Option<String>,

    /// Separator string for prefix extraction (default: pipe '|')
    #[arg(
        long = "prefix-sep",
        default_value = "|",
        help_heading = "Input Options"
    )]
    pub prefix_sep: String,

    /// Column separator for cols:<spec> format (default: whitespace)
    #[arg(long = "cols-sep", help_heading = "Input Options")]
    pub cols_sep: Option<String>,

    /// Pre-run a Rhai script before any other stage runs.
    #[arg(
        long = "begin",
        help_heading = "Processing Options",
        help = "Pre-run a Rhai script before any other stage runs.\n\nTypical use: seed the global `conf` map with lookup tables or shared context.\n\nHelpers available only here:\n  read_lines(path) -> Array<String>  # UTF-8, one entry per line\n  read_file(path)  -> String         # UTF-8, entire file contents\n\nData stored in `conf` becomes read-only afterwards. See --help-rhai for stage order."
    )]
    pub begin: Option<String>,

    /// Boolean filter expressions. See --help-rhai for expression examples.
    #[arg(long = "filter", help_heading = "Processing Options")]
    pub filters: Vec<String>,

    /// Transform/process exec scripts evaluated on each event. See --help-rhai for stage semantics.
    #[arg(short = 'e', long = "exec", help_heading = "Processing Options")]
    pub execs: Vec<String>,

    /// Execute script from file (contents run in the exec stage).
    #[arg(short = 'E', long = "exec-file", help_heading = "Processing Options")]
    pub exec_files: Vec<String>,
    /// Include Rhai files before script stages
    #[arg(short = 'I', long = "include", help_heading = "Processing Options")]
    pub includes: Vec<String>,

    /// Run once after processing completes (post-processing stage). Ideal for summarising metrics or emitting reports. The global `metrics` map from track_*() calls is accessible here.
    #[arg(long = "end", help_heading = "Processing Options")]
    pub end: Option<String>,

    /// Allow Rhai scripts to create directories and write files on disk (required for file helpers like append_file or mkdir).
    #[arg(long = "allow-fs-writes", help_heading = "Processing Options")]
    pub allow_fs_writes: bool,

    /// Enable access to a sliding window of N+1 recent events (needed for window_* functions).
    #[arg(long = "window", help_heading = "Processing Options")]
    pub window_size: Option<usize>,

    /// Exit on first error (fail-fast behavior)
    #[arg(long = "strict", help_heading = "Error Handling")]
    pub strict: bool,

    /// Disable strict error handling (resilient mode)
    #[arg(
        long = "no-strict",
        help_heading = "Error Handling",
        overrides_with = "strict"
    )]
    pub no_strict: bool,

    /// Show detailed error information (use multiple times for more verbosity: -v, -vv, -vvv)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count, help_heading = "Error Handling")]
    pub verbose: u8,

    /// Quiet mode with explicit levels (default: off).
    #[arg(
        short = 'q',
        long = "quiet",
        action = clap::ArgAction::Count,
        help_heading = "Error Handling",
        help = "Quiet mode with explicit levels (default: off).\n\n  -q   Suppress kelora diagnostics (errors, stats) and context markers (-A/-B/-C)\n  -qq  Additionally suppress event output (same as -F none)\n  -qqq Additionally suppress script side effects (print/eprint)"
    )]
    pub quiet: u8,

    /// Include only events with these log levels
    #[arg(
        short = 'l',
        long = "levels",
        value_delimiter = ',',
        help_heading = "Filtering Options",
        help = "Include only events with these log levels (comma-separated, case-insensitive)."
    )]
    pub levels: Vec<String>,

    /// Exclude events with these log levels
    #[arg(
        short = 'L',
        long = "exclude-levels",
        value_delimiter = ',',
        help_heading = "Filtering Options",
        help = "Exclude events with these log levels (comma-separated, case-insensitive)."
    )]
    pub exclude_levels: Vec<String>,

    /// Output only specific fields
    #[arg(
        short = 'k',
        long = "keys",
        value_delimiter = ',',
        help_heading = "Filtering Options",
        help = "Output only these fields (comma-separated list)."
    )]
    pub keys: Vec<String>,

    /// Exclude specific fields from output
    #[arg(
        short = 'K',
        long = "exclude-keys",
        value_delimiter = ',',
        help_heading = "Filtering Options",
        help = "Exclude these fields from output (comma-separated list)."
    )]
    pub exclude_keys: Vec<String>,

    /// Start showing entries on or newer than the specified date
    #[arg(
        long = "since",
        help_heading = "Filtering Options",
        help = "Accepts journalctl-style timestamps (e.g., 2024-01-15T12:00:00Z, '2024-01-15 12:00', '1h', '-30m', 'yesterday')."
    )]
    pub since: Option<String>,

    /// Stop showing entries on or older than the specified date
    #[arg(
        long = "until",
        help_heading = "Filtering Options",
        help = "Accepts journalctl-style timestamps (e.g., 2024-01-15T12:00:00Z, '2024-01-15 12:00', '1h', '+30m', 'tomorrow')."
    )]
    pub until: Option<String>,

    /// Limit output to the first N events
    #[arg(long = "take", help_heading = "Filtering Options")]
    pub take: Option<usize>,

    /// Show N lines before each match (requires filtering)
    #[arg(
        short = 'B',
        long = "before-context",
        help_heading = "Filtering Options"
    )]
    pub before_context: Option<usize>,

    /// Show N lines after each match (requires filtering)
    #[arg(
        short = 'A',
        long = "after-context",
        help_heading = "Filtering Options"
    )]
    pub after_context: Option<usize>,

    /// Show N lines before and after each match (requires filtering)
    #[arg(short = 'C', long = "context", help_heading = "Filtering Options")]
    pub context: Option<usize>,

    /// Output format
    #[arg(
        short = 'F',
        long = "output-format",
        value_enum,
        default_value = "default",
        help_heading = "Output Options"
    )]
    pub output_format: OutputFormat,

    /// Shortcut for -F json
    #[arg(
        short = 'J',
        help_heading = "Output Options",
        conflicts_with = "output_format"
    )]
    pub json_output: bool,

    /// Output only core fields
    #[arg(short = 'c', long = "core", help_heading = "Output Options")]
    pub core: bool,

    /// Output file for formatted events
    #[arg(short = 'o', long = "output-file", help_heading = "Output Options")]
    pub output_file: Option<String>,

    /// Output only field values (default: false).
    #[arg(short = 'b', long = "brief", help_heading = "Default Format Options")]
    pub brief: bool,

    /// Pretty-print nested values (default: false).
    #[arg(short = 'P', long = "pretty", help_heading = "Default Format Options")]
    pub pretty: bool,

    /// Enable word-wrapping (default: on).
    #[arg(long = "wrap", help_heading = "Default Format Options")]
    pub wrap: bool,

    /// Disable word-wrapping (overrides --wrap).
    #[arg(
        long = "no-wrap",
        help_heading = "Default Format Options",
        overrides_with = "wrap"
    )]
    pub no_wrap: bool,

    /// Comma-separated list of fields to format as RFC3339 timestamps
    #[arg(long = "pretty-ts", help_heading = "Default Format Options")]
    pub pretty_ts: Option<String>,

    /// Auto-format all known timestamp fields as local RFC3339 (requires parsed timestamps).
    #[arg(short = 'z', help_heading = "Default Format Options")]
    pub format_timestamps_local: bool,

    /// Auto-format all known timestamp fields as UTC RFC3339 (requires parsed timestamps).
    #[arg(short = 'Z', help_heading = "Default Format Options")]
    pub format_timestamps_utc: bool,

    /// Force colored output
    #[arg(long = "force-color", help_heading = "Display Options")]
    pub force_color: bool,

    /// Disable colored output
    #[arg(long = "no-color", help_heading = "Display Options")]
    pub no_color: bool,

    /// Insert a centered marker when time gaps grow large.
    #[arg(
        long = "mark-gaps",
        value_name = "DURATION",
        help_heading = "Display Options",
        help = "Insert a centered marker when the time delta between events exceeds the given duration.\nExample: --mark-gaps 30s prints a divider when consecutive events are separated by >=30s."
    )]
    pub mark_gaps: Option<String>,

    /// Disable emoji prefixes
    #[arg(long = "no-emoji", help_heading = "Display Options")]
    pub no_emoji: bool,

    /// Enable parallel processing (default: sequential processing).
    #[arg(long = "parallel", help_heading = "Performance Options")]
    pub parallel: bool,

    /// Disable parallel processing explicitly (default mode is sequential).
    #[arg(
        long = "no-parallel",
        help_heading = "Performance Options",
        overrides_with = "parallel"
    )]
    pub no_parallel: bool,

    /// Number of worker threads
    #[arg(
        long = "threads",
        default_value_t = 0,
        help_heading = "Performance Options"
    )]
    pub threads: usize,

    /// Batch size for parallel processing
    #[arg(long = "batch-size", help_heading = "Performance Options")]
    pub batch_size: Option<usize>,

    /// Batch timeout in milliseconds
    #[arg(
        long = "batch-timeout",
        default_value_t = 200,
        help_heading = "Performance Options",
        help = "Flush partially full parallel batches after this idle period. Lower values reduce latency; higher values improve throughput."
    )]
    pub batch_timeout: u64,

    /// Disable ordered output
    #[arg(long = "unordered", help_heading = "Performance Options")]
    pub no_preserve_order: bool,

    /// Show processing statistics (default: off).
    #[arg(short = 's', long = "stats", help_heading = "Metrics and Stats")]
    pub stats: bool,

    /// Disable processing statistics explicitly (default: off).
    #[arg(
        long = "no-stats",
        help_heading = "Metrics and Stats",
        overrides_with = "stats"
    )]
    pub no_stats: bool,

    /// Show processing statistics with no output
    #[arg(
        short = 'S',
        long = "stats-only",
        help_heading = "Metrics and Stats",
        help = "Print processing statistics only (implies -F none)."
    )]
    pub stats_only: bool,

    /// Show tracked metrics (default: off). Pair with --metrics-file for JSON output.
    #[arg(
        short = 'm',
        long = "metrics",
        help_heading = "Metrics and Stats",
        help = "Expose metrics recorded via track_*() in Rhai (see --help-rhai)."
    )]
    pub metrics: bool,

    /// Disable tracked metrics explicitly (default: off).
    #[arg(
        long = "no-metrics",
        help_heading = "Metrics and Stats",
        overrides_with = "metrics"
    )]
    pub no_metrics: bool,

    /// Write metrics to file (JSON format). Requires --metrics.
    #[arg(
        long = "metrics-file",
        help_heading = "Metrics and Stats",
        help = "Persist the metrics map (populated by track_*()) to disk as JSON."
    )]
    pub metrics_file: Option<String>,

    /// Use alias from configuration file
    #[arg(short = 'a', long = "alias", help_heading = "Configuration Options")]
    pub alias: Vec<String>,

    /// Specify custom configuration file path
    #[arg(long = "config-file", help_heading = "Configuration Options")]
    pub config_file: Option<String>,

    /// Show configuration file and exit
    #[arg(long = "show-config", help_heading = "Configuration Options")]
    pub show_config: bool,

    /// Edit configuration file in default editor and exit
    #[arg(long = "edit-config", help_heading = "Configuration Options")]
    pub edit_config: bool,

    /// Ignore configuration file
    #[arg(long = "ignore-config", help_heading = "Configuration Options")]
    pub ignore_config: bool,

    /// Save current command as alias to configuration file
    #[arg(long = "save-alias", help_heading = "Configuration Options")]
    pub save_alias: Option<String>,

    /// Show Rhai scripting guide and exit
    #[arg(long = "help-rhai", help_heading = "Help Options")]
    pub help_rhai: bool,

    /// Show available Rhai functions and exit
    #[arg(long = "help-functions", help_heading = "Help Options")]
    pub help_functions: bool,

    /// Show time format help and exit
    #[arg(long = "help-time", help_heading = "Help Options")]
    pub help_time: bool,

    /// Show multiline strategy help and exit
    #[arg(long = "help-multiline", help_heading = "Help Options")]
    pub help_multiline: bool,
}

impl Cli {
    /// Resolve inverted boolean flags to their actual values
    pub fn resolve_boolean_flags(&mut self) {
        // Handle stats/no-stats
        if self.no_stats {
            self.stats = false;
        }

        // Handle parallel/no-parallel
        if self.no_parallel {
            self.parallel = false;
        }

        // Handle metrics/no-metrics
        if self.no_metrics {
            self.metrics = false;
        }

        // Handle strict/no-strict
        if self.no_strict {
            self.strict = false;
        }
    }
}

/// Preprocess script by prepending include file contents
fn preprocess_script_with_includes(script: &str, includes: &[String]) -> Result<String> {
    let mut result = String::new();

    // Concatenate include files first
    for include_path in includes {
        let include_content = std::fs::read_to_string(include_path).map_err(|e| {
            anyhow::anyhow!("Failed to read include file '{}': {}", include_path, e)
        })?;
        result.push_str(&include_content);
        result.push('\n'); // Ensure separation between files
    }

    // Append main script
    result.push_str(script);
    Ok(result)
}

/// Get includes that apply to begin/end stages based on CLI position
/// For begin: includes that appear before any script stage
/// For end: includes that appear after all script stages
fn get_begin_end_includes(matches: &ArgMatches) -> Result<(Vec<String>, Vec<String>)> {
    let mut begin_includes = Vec::new();
    let mut end_includes = Vec::new();

    if let Some(include_indices) = matches.indices_of("includes") {
        let include_values: Vec<&String> =
            matches.get_many::<String>("includes").unwrap().collect();

        // Collect all script stage positions
        let mut script_positions = Vec::new();
        if let Some(filter_indices) = matches.indices_of("filters") {
            script_positions.extend(filter_indices);
        }
        if let Some(exec_indices) = matches.indices_of("execs") {
            script_positions.extend(exec_indices);
        }
        if let Some(exec_file_indices) = matches.indices_of("exec_files") {
            script_positions.extend(exec_file_indices);
        }

        if script_positions.is_empty() {
            // No script stages - all includes go to begin
            for (pos, _) in include_indices.enumerate() {
                begin_includes.push(include_values[pos].clone());
            }
        } else {
            script_positions.sort();
            let first_script_pos = script_positions[0];
            let last_script_pos = script_positions[script_positions.len() - 1];

            for (pos, include_index) in include_indices.enumerate() {
                let include_file = include_values[pos].clone();

                if include_index < first_script_pos {
                    begin_includes.push(include_file);
                } else if include_index > last_script_pos {
                    end_includes.push(include_file);
                }
                // Includes between script stages are handled by get_ordered_script_stages
            }
        }
    }

    Ok((begin_includes, end_includes))
}

impl Cli {
    /// Extract filter and exec stages in the order they appeared on the command line
    pub fn get_ordered_script_stages(&self, matches: &ArgMatches) -> Result<Vec<ScriptStageType>> {
        use std::collections::HashMap;

        let mut stages_with_indices = Vec::new();
        let mut include_map: HashMap<usize, Vec<String>> = HashMap::new();

        // First, collect all include arguments and map them to the next script stage
        if let Some(include_indices) = matches.indices_of("includes") {
            let include_values: Vec<&String> =
                matches.get_many::<String>("includes").unwrap().collect();

            // Collect all script stage positions
            let mut script_positions = Vec::new();

            if let Some(filter_indices) = matches.indices_of("filters") {
                script_positions.extend(filter_indices);
            }
            if let Some(exec_indices) = matches.indices_of("execs") {
                script_positions.extend(exec_indices);
            }
            if let Some(exec_file_indices) = matches.indices_of("exec_files") {
                script_positions.extend(exec_file_indices);
            }

            script_positions.sort();

            // Associate each include with the next script stage
            for (pos, include_index) in include_indices.enumerate() {
                let include_file = include_values[pos].clone();

                // Find the next script stage position after this include
                if let Some(&next_script_pos) = script_positions
                    .iter()
                    .find(|&&script_pos| script_pos > include_index)
                {
                    include_map
                        .entry(next_script_pos)
                        .or_default()
                        .push(include_file);
                }
                // If no script stage follows, the include will be ignored (could warn here in future)
            }
        }

        // Get filter stages with their indices and apply preprocessing
        if let Some(filter_indices) = matches.indices_of("filters") {
            let filter_values: Vec<&String> =
                matches.get_many::<String>("filters").unwrap().collect();
            for (pos, index) in filter_indices.enumerate() {
                let script = filter_values[pos].clone();
                let empty_includes = Vec::new();
                let includes = include_map.get(&index).unwrap_or(&empty_includes);
                // For now, filters don't support includes - skip includes for filters
                if !includes.is_empty() {
                    eprintln!(
                        "Warning: --include is not yet supported with --filter, ignoring includes"
                    );
                }
                stages_with_indices.push((index, ScriptStageType::Filter(script)));
            }
        }

        // Get exec stages with their indices and apply preprocessing
        if let Some(exec_indices) = matches.indices_of("execs") {
            let exec_values: Vec<&String> = matches.get_many::<String>("execs").unwrap().collect();
            for (pos, index) in exec_indices.enumerate() {
                let script = exec_values[pos].clone();
                let empty_includes = Vec::new();
                let includes = include_map.get(&index).unwrap_or(&empty_includes);
                let preprocessed_script = preprocess_script_with_includes(&script, includes)?;
                stages_with_indices.push((index, ScriptStageType::Exec(preprocessed_script)));
            }
        }

        // Get exec-file stages with their indices and apply preprocessing
        if let Some(exec_file_indices) = matches.indices_of("exec_files") {
            let exec_file_values: Vec<&String> =
                matches.get_many::<String>("exec_files").unwrap().collect();
            for (pos, index) in exec_file_indices.enumerate() {
                let file_path = &exec_file_values[pos];
                let script_content = std::fs::read_to_string(file_path).map_err(|e| {
                    anyhow::anyhow!("Failed to read exec file '{}': {}", file_path, e)
                })?;
                let empty_includes = Vec::new();
                let includes = include_map.get(&index).unwrap_or(&empty_includes);
                let preprocessed_script =
                    preprocess_script_with_includes(&script_content, includes)?;
                stages_with_indices.push((index, ScriptStageType::Exec(preprocessed_script)));
            }
        }

        // Sort by original command line position
        stages_with_indices.sort_by_key(|(index, _)| *index);

        // Extract just the stages
        Ok(stages_with_indices
            .into_iter()
            .map(|(_, stage)| stage)
            .collect())
    }

    /// Get processed begin and end scripts with includes applied
    pub fn get_processed_begin_end(
        &self,
        matches: &ArgMatches,
    ) -> Result<(Option<String>, Option<String>)> {
        let (begin_includes, end_includes) = get_begin_end_includes(matches)?;

        let processed_begin = if let Some(ref begin_script) = self.begin {
            Some(preprocess_script_with_includes(
                begin_script,
                &begin_includes,
            )?)
        } else if !begin_includes.is_empty() {
            // If we have includes but no begin script, create one from includes only
            Some(preprocess_script_with_includes("", &begin_includes)?)
        } else {
            None
        };

        let processed_end = if let Some(ref end_script) = self.end {
            Some(preprocess_script_with_includes(end_script, &end_includes)?)
        } else if !end_includes.is_empty() {
            // If we have includes but no end script, create one from includes only
            Some(preprocess_script_with_includes("", &end_includes)?)
        } else {
            None
        };

        Ok((processed_begin, processed_end))
    }
}

/// Parse and validate format value - supports both standard formats and cols:<spec>
fn parse_format_value(s: &str) -> Result<String, String> {
    // Check if it's a cols format
    if let Some(spec) = s.strip_prefix("cols:") {
        if spec.trim().is_empty() {
            return Err(
                "cols format requires a specification, e.g., 'cols:ts level *msg'".to_string(),
            );
        }
        return Ok(s.to_string());
    }

    // Check if it's a standard format
    match s.to_lowercase().as_str() {
        "auto" | "json" | "line" | "raw" | "logfmt" | "syslog" | "cef"
        | "csv" | "tsv" | "csvnh" | "tsvnh" | "combined" | "cols" => {
            Ok(s.to_string())
        }
        _ => {
            Err(format!(
                "Unknown format '{}'. Supported formats: auto, json, line, raw, logfmt, syslog, cef, csv, tsv, csvnh, tsvnh, combined, cols, or cols:<spec>",
                s
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn parse_cli(args: &[String]) -> (Cli, ArgMatches) {
        let matches = Cli::command()
            .try_get_matches_from(args.iter().map(|s| s.as_str()))
            .expect("failed to build matches");
        let cli = Cli::parse_from(args.to_vec());
        (cli, matches)
    }

    #[test]
    fn ordered_script_stages_preserve_cli_sequence() {
        let mut exec_file = NamedTempFile::new().expect("temp file");
        writeln!(exec_file, "meta.count = meta.count + 1;").expect("write script");
        let exec_path = exec_file.path().to_str().unwrap().to_string();

        let args = vec![
            "kelora".to_string(),
            "--filter".to_string(),
            "e.status >= 400".to_string(),
            "-e".to_string(),
            "e.alert = true;".to_string(),
            "--filter".to_string(),
            "e.status < 500".to_string(),
            "-E".to_string(),
            exec_path,
        ];

        let (cli, matches) = parse_cli(&args);
        let stages = cli
            .get_ordered_script_stages(&matches)
            .expect("stages should be parsed");

        assert_eq!(stages.len(), 4);
        assert!(matches!(
            &stages[0],
            ScriptStageType::Filter(script) if script == "e.status >= 400"
        ));
        assert!(matches!(
            &stages[1],
            ScriptStageType::Exec(script) if script == "e.alert = true;"
        ));
        assert!(matches!(
            &stages[2],
            ScriptStageType::Filter(script) if script == "e.status < 500"
        ));
        assert!(
            matches!(&stages[3], ScriptStageType::Exec(script) if script.contains("meta.count"))
        );
    }

    #[test]
    fn ordered_script_stages_error_when_exec_file_missing() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let missing_path = std::env::temp_dir().join(format!(
            "kelora-missing-{}-{}.rhai",
            std::process::id(),
            timestamp
        ));
        let _ = std::fs::remove_file(&missing_path);
        let missing = missing_path.to_string_lossy().to_string();

        let args = vec!["kelora".to_string(), "-E".to_string(), missing.clone()];

        let (cli, matches) = parse_cli(&args);
        let err = cli
            .get_ordered_script_stages(&matches)
            .expect_err("should report missing file");
        assert!(err
            .to_string()
            .contains(&format!("Failed to read exec file '{}':", missing)));
    }

    #[test]
    fn ordered_script_stages_empty_when_no_scripts_specified() {
        let args = vec!["kelora".to_string()];
        let (cli, matches) = parse_cli(&args);
        let stages = cli
            .get_ordered_script_stages(&matches)
            .expect("empty stages should succeed");
        assert!(stages.is_empty());
    }

    #[test]
    fn include_single_file_to_exec_stage() {
        let mut include_file = NamedTempFile::new().expect("temp file");
        writeln!(include_file, "fn helper() {{ return 42; }}").expect("write include");
        let include_path = include_file.path().to_str().unwrap().to_string();

        let args = vec![
            "kelora".to_string(),
            "-I".to_string(),
            include_path,
            "--exec".to_string(),
            "e.result = helper();".to_string(),
        ];

        let (cli, matches) = parse_cli(&args);
        let stages = cli
            .get_ordered_script_stages(&matches)
            .expect("stages should be parsed");

        assert_eq!(stages.len(), 1);
        if let ScriptStageType::Exec(script) = &stages[0] {
            assert!(script.contains("fn helper() { return 42; }"));
            assert!(script.contains("e.result = helper();"));
            assert!(script.starts_with("fn helper()"));
        } else {
            panic!("Expected Exec stage");
        }
    }

    #[test]
    fn include_multiple_files_to_single_stage() {
        let mut include1 = NamedTempFile::new().expect("temp file");
        writeln!(include1, "fn helper1() {{ return 1; }}").expect("write include1");
        let include1_path = include1.path().to_str().unwrap().to_string();

        let mut include2 = NamedTempFile::new().expect("temp file");
        writeln!(include2, "fn helper2() {{ return 2; }}").expect("write include2");
        let include2_path = include2.path().to_str().unwrap().to_string();

        let args = vec![
            "kelora".to_string(),
            "-I".to_string(),
            include1_path,
            "-I".to_string(),
            include2_path,
            "--exec".to_string(),
            "e.result = helper1() + helper2();".to_string(),
        ];

        let (cli, matches) = parse_cli(&args);
        let stages = cli
            .get_ordered_script_stages(&matches)
            .expect("stages should be parsed");

        assert_eq!(stages.len(), 1);
        if let ScriptStageType::Exec(script) = &stages[0] {
            assert!(script.contains("fn helper1() { return 1; }"));
            assert!(script.contains("fn helper2() { return 2; }"));
            assert!(script.contains("e.result = helper1() + helper2();"));
        } else {
            panic!("Expected Exec stage");
        }
    }

    #[test]
    fn includes_apply_to_next_script_stage() {
        let mut include1 = NamedTempFile::new().expect("temp file");
        writeln!(include1, "fn util1() {{ return 1; }}").expect("write include1");
        let include1_path = include1.path().to_str().unwrap().to_string();

        let mut include2 = NamedTempFile::new().expect("temp file");
        writeln!(include2, "fn util2() {{ return 2; }}").expect("write include2");
        let include2_path = include2.path().to_str().unwrap().to_string();

        let args = vec![
            "kelora".to_string(),
            "-I".to_string(),
            include1_path,
            "--exec".to_string(),
            "e.val1 = util1();".to_string(),
            "-I".to_string(),
            include2_path,
            "--exec".to_string(),
            "e.val2 = util2();".to_string(),
        ];

        let (cli, matches) = parse_cli(&args);
        let stages = cli
            .get_ordered_script_stages(&matches)
            .expect("stages should be parsed");

        assert_eq!(stages.len(), 2);

        // First stage should have include1
        if let ScriptStageType::Exec(script) = &stages[0] {
            assert!(script.contains("fn util1() { return 1; }"));
            assert!(script.contains("e.val1 = util1();"));
            assert!(!script.contains("fn util2() { return 2; }"));
        } else {
            panic!("Expected Exec stage");
        }

        // Second stage should have include2
        if let ScriptStageType::Exec(script) = &stages[1] {
            assert!(script.contains("fn util2() { return 2; }"));
            assert!(script.contains("e.val2 = util2();"));
            assert!(!script.contains("fn util1() { return 1; }"));
        } else {
            panic!("Expected Exec stage");
        }
    }

    #[test]
    fn include_with_exec_file() {
        let mut include_file = NamedTempFile::new().expect("temp file");
        writeln!(include_file, "fn shared_util() {{ return 42; }}").expect("write include");
        let include_path = include_file.path().to_str().unwrap().to_string();

        let mut exec_file = NamedTempFile::new().expect("temp file");
        writeln!(exec_file, "e.value = shared_util();").expect("write exec");
        let exec_path = exec_file.path().to_str().unwrap().to_string();

        let args = vec![
            "kelora".to_string(),
            "-I".to_string(),
            include_path,
            "-E".to_string(),
            exec_path,
        ];

        let (cli, matches) = parse_cli(&args);
        let stages = cli
            .get_ordered_script_stages(&matches)
            .expect("stages should be parsed");

        assert_eq!(stages.len(), 1);
        if let ScriptStageType::Exec(script) = &stages[0] {
            assert!(script.contains("fn shared_util() { return 42; }"));
            assert!(script.contains("e.value = shared_util();"));
        } else {
            panic!("Expected Exec stage");
        }
    }

    #[test]
    fn include_error_when_file_missing() {
        let missing_path = "/non/existent/path.rhai";

        let args = vec![
            "kelora".to_string(),
            "-I".to_string(),
            missing_path.to_string(),
            "--exec".to_string(),
            "e.test = true;".to_string(),
        ];

        let (cli, matches) = parse_cli(&args);
        let err = cli
            .get_ordered_script_stages(&matches)
            .expect_err("should report missing include file");
        assert!(err
            .to_string()
            .contains(&format!("Failed to read include file '{}':", missing_path)));
    }

    #[test]
    fn get_processed_begin_end_with_includes() {
        let mut include1 = NamedTempFile::new().expect("temp file");
        writeln!(include1, "fn setup() {{ print('setup'); }}").expect("write include1");
        let include1_path = include1.path().to_str().unwrap().to_string();

        let mut include2 = NamedTempFile::new().expect("temp file");
        writeln!(include2, "fn cleanup() {{ print('cleanup'); }}").expect("write include2");
        let include2_path = include2.path().to_str().unwrap().to_string();

        let args = vec![
            "kelora".to_string(),
            "-I".to_string(),
            include1_path,
            "--begin".to_string(),
            "setup();".to_string(),
            "--exec".to_string(),
            "e.processed = true;".to_string(),
            "-I".to_string(),
            include2_path,
            "--end".to_string(),
            "cleanup();".to_string(),
        ];

        let (cli, matches) = parse_cli(&args);
        let (begin, end) = cli
            .get_processed_begin_end(&matches)
            .expect("should process begin/end");

        assert!(begin.is_some());
        let begin_script = begin.unwrap();
        assert!(begin_script.contains("fn setup() { print('setup'); }"));
        assert!(begin_script.contains("setup();"));

        assert!(end.is_some());
        let end_script = end.unwrap();
        assert!(end_script.contains("fn cleanup() { print('cleanup'); }"));
        assert!(end_script.contains("cleanup();"));
    }

    #[test]
    fn include_only_before_begin_creates_begin_stage() {
        let mut include_file = NamedTempFile::new().expect("temp file");
        writeln!(include_file, "print('auto setup');").expect("write include");
        let include_path = include_file.path().to_str().unwrap().to_string();

        let args = vec![
            "kelora".to_string(),
            "-I".to_string(),
            include_path,
            "--exec".to_string(),
            "e.processed = true;".to_string(),
        ];

        let (cli, matches) = parse_cli(&args);
        let (begin, end) = cli
            .get_processed_begin_end(&matches)
            .expect("should process begin/end");

        assert!(begin.is_some());
        let begin_script = begin.unwrap();
        assert!(begin_script.contains("print('auto setup');"));

        assert!(end.is_none());
    }
}
