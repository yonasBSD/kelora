// CLI-specific types and structures
// This module contains the command-line interface definitions and parsing logic

use crate::config::ScriptStageType;
use anyhow::Result;
use clap::{ArgMatches, Parser};

// CLI types - specific to command-line interface
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InputFormat {
    Auto,
    Jsonl,
    Line,
    Logfmt,
    Syslog,
    Cef,
    Csv,
    Tsv,
    Csvnh,
    Tsvnh,
    Apache,
    Nginx,
    Cols,
    Docker,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    Jsonl,
    #[default]
    Default,
    Logfmt,
    Csv,
    Tsv,
    Csvnh,
    Tsvnh,
    Hide,
    Null,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum FileOrder {
    None,
    Name,
    Mtime,
}

// CLI structure - contains all command-line arguments and options
#[derive(Parser)]
#[command(name = "kelora")]
#[command(about = "A command-line log analysis tool with embedded Rhai scripting")]
#[command(
    long_about = "A command-line log analysis tool with embedded Rhai scripting\n\nMODES:\n  (default)   Sequential processing - best for streaming/interactive use\n  --parallel  Parallel processing - best for high-throughput batch analysis"
)]
#[command(author = "Dirk Loss <mail@dirk-loss.de>")]
#[command(version)]
pub struct Cli {
    /// Input files (stdin if not specified, or use "-" to explicitly specify stdin)
    pub files: Vec<String>,

    /// Input format
    #[arg(
        short = 'f',
        long = "format",
        value_enum,
        default_value = "line",
        help_heading = "Input Options"
    )]
    pub format: InputFormat,

    /// Shortcut for -f jsonl
    #[arg(
        short = 'j',
        help_heading = "Input Options",
        conflicts_with = "format"
    )]
    pub jsonl_input: bool,

    /// File processing order
    #[arg(
        long = "file-order",
        value_enum,
        default_value = "none",
        help_heading = "Input Options"
    )]
    pub file_order: FileOrder,

    /// Skip the first N input lines
    #[arg(long = "skip-lines", help_heading = "Input Options")]
    pub skip_lines: Option<usize>,

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

    /// Pre-run a Rhai script. Use it to populate the global `init` map
    /// with shared, read-only data.
    ///
    /// Functions (usable only here):
    ///   read_lines(path) → Array<String>  # UTF-8, one element per line
    ///   read_file(path)  → String         # UTF-8, full file
    ///
    /// Data written to `init` becomes read-only for the rest of the run.
    #[arg(long = "begin", help_heading = "Processing Options")]
    pub begin: Option<String>,

    /// Boolean filter expressions
    #[arg(long = "filter", help_heading = "Processing Options")]
    pub filters: Vec<String>,

    /// Transform/process exec scripts
    #[arg(short = 'e', long = "exec", help_heading = "Processing Options")]
    pub execs: Vec<String>,

    /// Execute script from file
    #[arg(short = 'E', long = "exec-file", help_heading = "Processing Options")]
    pub exec_files: Vec<String>,

    /// Run once after processing
    #[arg(long = "end", help_heading = "Processing Options")]
    pub end: Option<String>,

    /// Enable access to a sliding window of N+1 recent events
    #[arg(long = "window", help_heading = "Processing Options")]
    pub window_size: Option<usize>,

    /// File to write error reports to
    #[arg(long = "error-report-file", help_heading = "Error Handling")]
    pub error_report_file: Option<String>,

    /// Exit on first error (fail-fast behavior)
    #[arg(long = "strict", help_heading = "Error Handling")]
    pub strict: bool,

    /// Show detailed error information (use multiple times for more verbosity: -v, -vv, -vvv)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count, help_heading = "Error Handling")]
    pub verbose: u8,

    /// Suppress all kelora output (events, errors, stats) but preserve script side effects
    #[arg(short = 'q', long = "quiet", help_heading = "Error Handling")]
    pub quiet: bool,

    /// Include only events with these log levels
    #[arg(
        short = 'l',
        long = "levels",
        value_delimiter = ',',
        help_heading = "Filtering Options"
    )]
    pub levels: Vec<String>,

    /// Exclude events with these log levels
    #[arg(
        short = 'L',
        long = "exclude-levels",
        value_delimiter = ',',
        help_heading = "Filtering Options"
    )]
    pub exclude_levels: Vec<String>,

    /// Output only specific fields
    #[arg(
        short = 'k',
        long = "keys",
        value_delimiter = ',',
        help_heading = "Filtering Options"
    )]
    pub keys: Vec<String>,

    /// Exclude specific fields from output
    #[arg(
        short = 'K',
        long = "exclude-keys",
        value_delimiter = ',',
        help_heading = "Filtering Options"
    )]
    pub exclude_keys: Vec<String>,

    /// Start showing entries on or newer than the specified date
    #[arg(long = "since", help_heading = "Filtering Options")]
    pub since: Option<String>,

    /// Stop showing entries on or older than the specified date
    #[arg(long = "until", help_heading = "Filtering Options")]
    pub until: Option<String>,

    /// Limit output to the first N events
    #[arg(long = "take", help_heading = "Filtering Options")]
    pub take: Option<usize>,

    /// Output format
    #[arg(
        short = 'F',
        long = "output-format",
        value_enum,
        default_value = "default",
        help_heading = "Output Options"
    )]
    pub output_format: OutputFormat,

    /// Shortcut for -F jsonl
    #[arg(
        short = 'J',
        help_heading = "Output Options",
        conflicts_with = "output_format"
    )]
    pub jsonl_output: bool,

    /// Output only core fields
    #[arg(short = 'c', long = "core", help_heading = "Output Options")]
    pub core: bool,

    /// Output only field values
    #[arg(short = 'b', long = "brief", help_heading = "Output Options")]
    pub brief: bool,

    /// Output file for formatted events
    #[arg(short = 'o', long = "output-file", help_heading = "Output Options")]
    pub output_file: Option<String>,

    /// Comma-separated list of fields to format as RFC3339 timestamps.
    /// Only affects default output; does not modify event data.
    #[arg(long = "pretty-ts", help_heading = "Output Options")]
    pub pretty_ts: Option<String>,

    /// Auto-format all known timestamp fields as local RFC3339.
    /// Only affects default output; does not modify event data.
    #[arg(short = 'z', help_heading = "Output Options")]
    pub format_timestamps_local: bool,

    /// Auto-format all known timestamp fields as UTC RFC3339.
    /// Only affects default output; does not modify event data.
    #[arg(short = 'Z', help_heading = "Output Options")]
    pub format_timestamps_utc: bool,

    /// Enable parallel processing
    #[arg(long = "parallel", help_heading = "Performance Options")]
    pub parallel: bool,

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
        help_heading = "Performance Options"
    )]
    pub batch_timeout: u64,

    /// Disable ordered output
    #[arg(long = "unordered", help_heading = "Performance Options")]
    pub no_preserve_order: bool,

    /// Force colored output
    #[arg(long = "force-color", help_heading = "Display Options")]
    pub force_color: bool,

    /// Disable colored output
    #[arg(long = "no-color", help_heading = "Display Options")]
    pub no_color: bool,

    /// Disable emoji prefixes
    #[arg(long = "no-emoji", help_heading = "Display Options")]
    pub no_emoji: bool,

    /// Show processing statistics
    #[arg(short = 's', long = "stats", help_heading = "Metrics and Stats")]
    pub stats: bool,

    /// Show processing statistics with no output
    #[arg(short = 'S', long = "stats-only", help_heading = "Metrics and Stats")]
    pub stats_only: bool,

    /// Show tracked metrics
    #[arg(short = 'm', long = "metrics", help_heading = "Metrics and Stats")]
    pub metrics: bool,

    /// Write metrics to file (JSON format)
    #[arg(long = "metrics-file", help_heading = "Metrics and Stats")]
    pub metrics_file: Option<String>,


    /// Use alias from configuration file
    #[arg(short = 'a', long = "alias", help_heading = "Configuration Options")]
    pub alias: Vec<String>,

    /// Show configuration file and exit
    #[arg(long = "show-config", help_heading = "Configuration Options")]
    pub show_config: bool,

    /// Ignore configuration file
    #[arg(long = "ignore-config", help_heading = "Configuration Options")]
    pub ignore_config: bool,

    /// Show Rhai scripting guide and exit
    #[arg(long = "help-rhai", help_heading = "Help Options")]
    pub help_rhai: bool,

    /// Show available Rhai functions and exit
    #[arg(long = "help-functions", help_heading = "Help Options")]
    pub help_functions: bool,

    /// Show time format help and exit
    #[arg(long = "help-time", help_heading = "Help Options")]
    pub help_time: bool,
}

impl Cli {
    /// Extract filter and exec stages in the order they appeared on the command line
    pub fn get_ordered_script_stages(&self, matches: &ArgMatches) -> Result<Vec<ScriptStageType>> {
        let mut stages_with_indices = Vec::new();

        // Get filter stages with their indices
        if let Some(filter_indices) = matches.indices_of("filters") {
            let filter_values: Vec<&String> =
                matches.get_many::<String>("filters").unwrap().collect();
            for (pos, index) in filter_indices.enumerate() {
                stages_with_indices
                    .push((index, ScriptStageType::Filter(filter_values[pos].clone())));
            }
        }

        // Get exec stages with their indices
        if let Some(exec_indices) = matches.indices_of("execs") {
            let exec_values: Vec<&String> = matches.get_many::<String>("execs").unwrap().collect();
            for (pos, index) in exec_indices.enumerate() {
                stages_with_indices.push((index, ScriptStageType::Exec(exec_values[pos].clone())));
            }
        }

        // Get exec-file stages with their indices
        if let Some(exec_file_indices) = matches.indices_of("exec_files") {
            let exec_file_values: Vec<&String> =
                matches.get_many::<String>("exec_files").unwrap().collect();
            for (pos, index) in exec_file_indices.enumerate() {
                let file_path = &exec_file_values[pos];
                let script_content = std::fs::read_to_string(file_path).map_err(|e| {
                    anyhow::anyhow!("Failed to read exec file '{}': {}", file_path, e)
                })?;
                stages_with_indices.push((index, ScriptStageType::Exec(script_content)));
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
}
