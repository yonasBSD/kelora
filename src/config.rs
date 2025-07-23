#![allow(dead_code)]
use clap::ValueEnum;

/// Main configuration struct for Kelora
#[derive(Debug, Clone)]
pub struct KeloraConfig {
    pub input: InputConfig,
    pub output: OutputConfig,
    pub processing: ProcessingConfig,
    pub performance: PerformanceConfig,
}

/// Input configuration
#[derive(Debug, Clone)]
pub struct InputConfig {
    pub files: Vec<String>,
    pub format: InputFormat,
    pub file_order: FileOrder,
    pub skip_lines: usize,
    pub ignore_lines: Option<regex::Regex>,
    pub multiline: Option<MultilineConfig>,
    /// Custom timestamp field name (reserved for --since/--until features)
    #[allow(dead_code)]
    pub ts_field: Option<String>,
    /// Custom timestamp format string
    pub ts_format: Option<String>,
    /// Default timezone for naive timestamps (None = local time)
    pub default_timezone: Option<String>,
}

/// Output configuration
#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub keys: Vec<String>,
    pub exclude_keys: Vec<String>,
    pub core: bool,
    pub brief: bool,
    pub color: ColorMode,
    pub no_emoji: bool,
    pub no_section_headers: bool,
    pub stats: bool,
    pub metrics: bool,
    pub metrics_file: Option<String>,
    /// Timestamp formatting configuration (display-only)
    pub timestamp_formatting: TimestampFormatConfig,
}

/// Ordered script stages that preserve CLI order
#[derive(Debug, Clone)]
pub enum ScriptStageType {
    Filter(String),
    Exec(String),
}

/// Error reporting configuration
#[derive(Debug, Clone)]
pub struct ErrorReportConfig {
    pub style: ErrorReportStyle,
    pub file: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ErrorReportStyle {
    Off,
    Summary,
    Print,
}

/// Processing configuration
#[derive(Debug, Clone)]
pub struct ProcessingConfig {
    pub begin: Option<String>,
    pub stages: Vec<ScriptStageType>,
    pub end: Option<String>,
    pub no_inject_fields: bool,
    pub inject_prefix: Option<String>,
    pub error_report: ErrorReportConfig,
    pub levels: Vec<String>,
    pub exclude_levels: Vec<String>,
    /// Window size for sliding window functionality (0 = disabled)
    pub window_size: usize,
    /// Timestamp filtering configuration
    pub timestamp_filter: Option<TimestampFilterConfig>,
    /// Limit output to the first N events (None = no limit)
    pub take_limit: Option<usize>,
    /// Exit on first error (fail-fast behavior) - new resiliency model
    pub strict: bool,
    /// Show detailed error information - new resiliency model
    pub verbose: bool,
    /// Suppress error summary (quiet mode) - new resiliency model
    pub quiet: bool,
}

/// Performance configuration
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    pub parallel: bool,
    pub threads: usize,
    pub batch_size: Option<usize>,
    pub batch_timeout: u64,
    pub no_preserve_order: bool,
}

/// Input format enumeration
#[derive(ValueEnum, Clone, Debug)]
pub enum InputFormat {
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
}

/// Output format enumeration
#[derive(ValueEnum, Clone, Debug, Default)]
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


/// File processing order
#[derive(ValueEnum, Clone, Debug)]
pub enum FileOrder {
    None,
    Name,
    Mtime,
}

/// Color output mode
#[derive(ValueEnum, Clone, Debug)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

/// Timestamp filtering configuration
#[derive(Debug, Clone)]
pub struct TimestampFilterConfig {
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    pub until: Option<chrono::DateTime<chrono::Utc>>,
}

/// Timestamp formatting configuration (display-only, affects default output format only)
#[derive(Debug, Clone, Default)]
pub struct TimestampFormatConfig {
    /// Specific fields to format as timestamps
    pub format_fields: Vec<String>,
    /// Auto-format all known timestamp fields
    pub auto_format_all: bool,
    /// Target timezone for formatting (true = UTC, false = local)
    pub format_as_utc: bool,
}

/// Multi-line event detection configuration
#[derive(Debug, Clone)]
pub struct MultilineConfig {
    pub strategy: MultilineStrategy,
}

/// Multi-line event detection strategies
#[derive(Debug, Clone)]
pub enum MultilineStrategy {
    /// Events start with timestamp pattern
    Timestamp { pattern: String },
    /// Continuation lines are indented
    Indent {
        spaces: Option<u32>,
        tabs: bool,
        mixed: bool,
    },
    /// Lines end with continuation character
    Backslash { char: char },
    /// Events start with pattern
    Start { pattern: String },
    /// Events end with pattern
    End { pattern: String },
    /// Events have both start and end boundaries
    Boundary { start: String, end: String },
}

impl MultilineConfig {
    /// Parse multiline configuration from CLI string
    pub fn parse(value: &str) -> Result<Self, String> {
        let parts: Vec<&str> = value.split(':').collect();

        if parts.is_empty() {
            return Err("Empty multiline configuration".to_string());
        }

        let strategy_name = parts[0];
        let strategy = match strategy_name {
            "timestamp" => {
                let pattern = if parts.len() > 1 {
                    Self::parse_pattern_option(parts[1])?
                } else {
                    // Default timestamp patterns (ISO and syslog) - both anchored to start
                    r"^(\d{4}-\d{2}-\d{2}|\w{3}\s+\d{1,2})".to_string()
                };
                MultilineStrategy::Timestamp { pattern }
            }
            "indent" => {
                let (spaces, tabs, mixed) = if parts.len() > 1 {
                    Self::parse_indent_options(parts[1])?
                } else {
                    (None, false, true) // Default: any whitespace
                };
                MultilineStrategy::Indent {
                    spaces,
                    tabs,
                    mixed,
                }
            }
            "backslash" => {
                let char = if parts.len() > 1 {
                    Self::parse_char_option(parts[1])?
                } else {
                    '\\' // Default backslash
                };
                MultilineStrategy::Backslash { char }
            }
            "start" => {
                if parts.len() < 2 {
                    return Err("Start strategy requires pattern: start:REGEX".to_string());
                }
                let pattern = parts[1].to_string();
                MultilineStrategy::Start { pattern }
            }
            "end" => {
                if parts.len() < 2 {
                    return Err("End strategy requires pattern: end:REGEX".to_string());
                }
                let pattern = parts[1].to_string();
                MultilineStrategy::End { pattern }
            }
            "boundary" => {
                if parts.len() < 2 {
                    return Err(
                        "Boundary strategy requires start and end: boundary:start=REGEX:end=REGEX"
                            .to_string(),
                    );
                }
                let (start, end) = Self::parse_boundary_options(&parts[1..])?;
                MultilineStrategy::Boundary { start, end }
            }
            _ => return Err(format!("Unknown multiline strategy: {}", strategy_name)),
        };

        Ok(MultilineConfig { strategy })
    }

    fn parse_pattern_option(option: &str) -> Result<String, String> {
        if let Some(pattern) = option.strip_prefix("pattern=") {
            Ok(pattern.to_string())
        } else {
            Err(format!("Invalid timestamp option: {}", option))
        }
    }

    fn parse_indent_options(option: &str) -> Result<(Option<u32>, bool, bool), String> {
        match option {
            "tabs" => Ok((None, true, false)),
            "mixed" => Ok((None, false, true)),
            option if option.starts_with("spaces=") => {
                let spaces_str = &option[7..];
                match spaces_str.parse::<u32>() {
                    Ok(n) => Ok((Some(n), false, false)),
                    Err(_) => Err(format!("Invalid spaces value: {}", spaces_str)),
                }
            }
            _ => Err(format!("Invalid indent option: {}", option)),
        }
    }

    fn parse_char_option(option: &str) -> Result<char, String> {
        if let Some(char_str) = option.strip_prefix("char=") {
            if char_str.len() == 1 {
                Ok(char_str.chars().next().unwrap())
            } else {
                Err(format!(
                    "Continuation character must be single character: {}",
                    char_str
                ))
            }
        } else {
            Err(format!("Invalid backslash option: {}", option))
        }
    }

    fn parse_boundary_options(parts: &[&str]) -> Result<(String, String), String> {
        let mut start = None;
        let mut end = None;

        for part in parts {
            if let Some(start_pattern) = part.strip_prefix("start=") {
                start = Some(start_pattern.to_string());
            } else if let Some(end_pattern) = part.strip_prefix("end=") {
                end = Some(end_pattern.to_string());
            } else {
                return Err(format!("Invalid boundary option: {}", part));
            }
        }

        match (start, end) {
            (Some(s), Some(e)) => Ok((s, e)),
            _ => Err("Boundary strategy requires both start=REGEX and end=REGEX".to_string()),
        }
    }
}

impl Default for MultilineConfig {
    fn default() -> Self {
        Self {
            strategy: MultilineStrategy::Timestamp {
                pattern: r"^(\d{4}-\d{2}-\d{2}|\w{3}\s+\d{1,2})".to_string(),
            },
        }
    }
}

impl InputFormat {
    /// Get default multiline configuration for this input format
    ///
    /// NOTE: Multiline processing is disabled by default for all formats
    /// to avoid unexpected buffering behavior in streaming scenarios.
    /// Users must explicitly enable multiline with --multiline option.
    pub fn default_multiline(&self) -> Option<MultilineConfig> {
        // Multiline is now strictly opt-in for all formats to avoid
        // unexpected "last event buffering" behavior in streaming scenarios
        None
    }
}

impl KeloraConfig {
    /// Get the list of core field names (ts, level, msg variants)
    pub fn get_core_field_names() -> Vec<String> {
        let mut core_fields = Vec::new();

        // Use constants from event.rs to ensure consistency
        core_fields.extend(
            crate::event::TIMESTAMP_FIELD_NAMES
                .iter()
                .map(|s| s.to_string()),
        );
        core_fields.extend(
            crate::event::LEVEL_FIELD_NAMES
                .iter()
                .map(|s| s.to_string()),
        );
        core_fields.extend(
            crate::event::MESSAGE_FIELD_NAMES
                .iter()
                .map(|s| s.to_string()),
        );

        core_fields
    }

    /// Format an error message with appropriate prefix (emoji or "kelora:")
    pub fn format_error_message(&self, message: &str) -> String {
        let use_colors = crate::tty::should_use_colors_with_mode(&self.output.color);
        let use_emoji = use_colors && !self.output.no_emoji;

        if use_emoji {
            format!("🧱 {}", message)
        } else {
            format!("kelora: {}", message)
        }
    }

    /// Format a stats message with appropriate prefix (emoji or "Stats:")
    pub fn format_stats_message(&self, message: &str) -> String {
        if self.output.no_section_headers {
            message.to_string()
        } else {
            let use_colors = crate::tty::should_use_colors_with_mode(&self.output.color);
            let use_emoji = use_colors && !self.output.no_emoji;

            if use_emoji {
                format!("📈 === Kelora Stats ===\n{}", message)
            } else {
                format!("=== Kelora Stats ===\n{}", message)
            }
        }
    }

    /// Format a metrics message with appropriate prefix (emoji or "Metrics:")
    pub fn format_metrics_message(&self, message: &str) -> String {
        if self.output.no_section_headers {
            message.to_string()
        } else {
            let use_colors = crate::tty::should_use_colors_with_mode(&self.output.color);
            let use_emoji = use_colors && !self.output.no_emoji;

            if use_emoji {
                format!("📊 === Kelora Metrics ===\n{}", message)
            } else {
                format!("=== Kelora Metrics ===\n{}", message)
            }
        }
    }
}

/// Format an error message with appropriate prefix when config is not available
/// Uses auto color detection and allows NO_EMOJI environment variable override
pub fn format_error_message_auto(message: &str) -> String {
    let use_colors = crate::tty::should_use_colors_with_mode(&ColorMode::Auto);
    let no_emoji = std::env::var("NO_EMOJI").is_ok();
    let use_emoji = use_colors && !no_emoji;

    if use_emoji {
        format!("🧱 {}", message)
    } else {
        format!("kelora: {}", message)
    }
}

impl OutputConfig {
    /// Get the effective keys for filtering, combining core fields with user-specified keys
    pub fn get_effective_keys(&self) -> Vec<String> {
        if self.core {
            let mut keys = KeloraConfig::get_core_field_names();
            // Add user-specified keys to the core fields
            keys.extend(self.keys.clone());
            keys
        } else {
            self.keys.clone()
        }
    }
}

impl KeloraConfig {
    /// Create configuration from CLI arguments
    pub fn from_cli(cli: &crate::Cli) -> Self {
        // Determine color mode from flags (no-color takes precedence over force-color)
        let color_mode = if cli.no_color {
            ColorMode::Never
        } else if cli.force_color {
            ColorMode::Always
        } else {
            ColorMode::Auto
        };

        Self {
            input: InputConfig {
                files: cli.files.clone(),
                format: cli.format.clone().into(),
                file_order: cli.file_order.clone().into(),
                skip_lines: cli.skip_lines.unwrap_or(0),
                ignore_lines: None, // Will be set after CLI parsing
                multiline: None,    // Will be set after CLI parsing
                ts_field: cli.ts_field.clone(),
                ts_format: cli.ts_format.clone(),
                default_timezone: determine_default_timezone(cli),
            },
            output: OutputConfig {
                format: if cli.stats_only {
                    OutputFormat::Null
                } else {
                    cli.output_format.clone().into()
                },
                keys: cli.keys.clone(),
                exclude_keys: cli.exclude_keys.clone(),
                core: cli.core,
                brief: cli.brief,
                color: color_mode,
                no_emoji: cli.no_emoji,
                no_section_headers: cli.no_section_headers,
                stats: cli.stats || cli.stats_only,
                metrics: cli.metrics,
                metrics_file: cli.metrics_file.clone(),
                timestamp_formatting: create_timestamp_format_config(cli),
            },
            processing: ProcessingConfig {
                begin: cli.begin.clone(),
                stages: Vec::new(), // Will be set by main() after CLI parsing
                end: cli.end.clone(),
                no_inject_fields: cli.no_inject_fields,
                inject_prefix: cli.inject_prefix.clone(),
                error_report: parse_error_report_config(cli),
                levels: cli.levels.clone(),
                exclude_levels: cli.exclude_levels.clone(),
                window_size: cli.window_size.unwrap_or(0),
                timestamp_filter: None, // Will be set in main() after parsing since/until
                take_limit: cli.take,
                strict: cli.strict,
                verbose: cli.verbose,
                quiet: cli.quiet,
            },
            performance: PerformanceConfig {
                parallel: cli.parallel,
                threads: cli.threads,
                batch_size: cli.batch_size,
                batch_timeout: cli.batch_timeout,
                no_preserve_order: cli.no_preserve_order,
            },
        }
    }

    /// Check if parallel processing should be used
    pub fn should_use_parallel(&self) -> bool {
        self.performance.parallel
            || self.performance.threads > 0
            || self.performance.batch_size.is_some()
    }

    /// Get effective batch size with defaults
    pub fn effective_batch_size(&self) -> usize {
        self.performance.batch_size.unwrap_or(1000)
    }

    /// Get effective thread count with defaults
    pub fn effective_threads(&self) -> usize {
        if self.performance.threads == 0 {
            num_cpus::get()
        } else {
            self.performance.threads
        }
    }
}

impl Default for KeloraConfig {
    fn default() -> Self {
        Self {
            input: InputConfig {
                files: Vec::new(),
                format: InputFormat::Jsonl,
                file_order: FileOrder::None,
                skip_lines: 0,
                ignore_lines: None,
                multiline: None,
                ts_field: None,
                ts_format: None,
                default_timezone: None,
            },
            output: OutputConfig {
                format: OutputFormat::Default,
                keys: Vec::new(),
                exclude_keys: Vec::new(),
                core: false,
                brief: false,
                color: ColorMode::Auto,
                no_emoji: false,
                no_section_headers: false,
                stats: false,
                metrics: false,
                metrics_file: None,
                timestamp_formatting: TimestampFormatConfig::default(),
            },
            processing: ProcessingConfig {
                begin: None,
                stages: Vec::new(),
                end: None,
                no_inject_fields: false,
                inject_prefix: None,
                error_report: ErrorReportConfig {
                    style: ErrorReportStyle::Summary,
                    file: None,
                },
                levels: Vec::new(),
                exclude_levels: Vec::new(),
                window_size: 0,
                timestamp_filter: None,
                take_limit: None,
                strict: false,
                verbose: false,
                quiet: false,
            },
            performance: PerformanceConfig {
                parallel: false,
                threads: 0,
                batch_size: None,
                batch_timeout: 200,
                no_preserve_order: false,
            },
        }
    }
}

/// Create timestamp formatting configuration from CLI options
fn create_timestamp_format_config(cli: &crate::Cli) -> TimestampFormatConfig {
    let format_fields = if let Some(ref pretty_ts) = cli.pretty_ts {
        pretty_ts.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        Vec::new()
    };

    let auto_format_all = cli.format_timestamps_local || cli.format_timestamps_utc;
    let format_as_utc = cli.format_timestamps_utc;

    TimestampFormatConfig {
        format_fields,
        auto_format_all,
        format_as_utc,
    }
}

/// Parse error report configuration from CLI
fn parse_error_report_config(cli: &crate::Cli) -> ErrorReportConfig {
    // Default error report style based on new resiliency model
    let style = if cli.strict {
        ErrorReportStyle::Print  // Show each error immediately in strict mode
    } else {
        ErrorReportStyle::Summary  // Show summary in resilient mode
    };

    ErrorReportConfig {
        style,
        file: cli.error_report_file.clone(),
    }
}

/// Determine the default timezone based on CLI options and environment
/// Following the new spec: --input-tz defaults to UTC
fn determine_default_timezone(cli: &crate::Cli) -> Option<String> {
    // Priority 1: --input-tz option
    if let Some(ref input_tz) = cli.input_tz {
        if input_tz == "local" {
            return None; // None means local time
        } else {
            return Some(input_tz.clone());
        }
    }

    // Priority 2: TZ environment variable
    if let Ok(tz) = std::env::var("TZ") {
        if !tz.is_empty() {
            return Some(tz);
        }
    }

    // DEFAULT: UTC (per spec, --input-tz defaults to UTC)
    Some("UTC".to_string())
}

// Conversion traits to maintain compatibility with existing CLI types
impl From<crate::InputFormat> for InputFormat {
    fn from(format: crate::InputFormat) -> Self {
        match format {
            crate::InputFormat::Jsonl => InputFormat::Jsonl,
            crate::InputFormat::Line => InputFormat::Line,
            crate::InputFormat::Logfmt => InputFormat::Logfmt,
            crate::InputFormat::Syslog => InputFormat::Syslog,
            crate::InputFormat::Cef => InputFormat::Cef,
            crate::InputFormat::Csv => InputFormat::Csv,
            crate::InputFormat::Tsv => InputFormat::Tsv,
            crate::InputFormat::Csvnh => InputFormat::Csvnh,
            crate::InputFormat::Tsvnh => InputFormat::Tsvnh,
            crate::InputFormat::Apache => InputFormat::Apache,
            crate::InputFormat::Nginx => InputFormat::Nginx,
            crate::InputFormat::Cols => InputFormat::Cols,
        }
    }
}

impl From<InputFormat> for crate::InputFormat {
    fn from(format: InputFormat) -> Self {
        match format {
            InputFormat::Jsonl => crate::InputFormat::Jsonl,
            InputFormat::Line => crate::InputFormat::Line,
            InputFormat::Logfmt => crate::InputFormat::Logfmt,
            InputFormat::Syslog => crate::InputFormat::Syslog,
            InputFormat::Cef => crate::InputFormat::Cef,
            InputFormat::Csv => crate::InputFormat::Csv,
            InputFormat::Tsv => crate::InputFormat::Tsv,
            InputFormat::Csvnh => crate::InputFormat::Csvnh,
            InputFormat::Tsvnh => crate::InputFormat::Tsvnh,
            InputFormat::Apache => crate::InputFormat::Apache,
            InputFormat::Nginx => crate::InputFormat::Nginx,
            InputFormat::Cols => crate::InputFormat::Cols,
        }
    }
}

impl From<crate::OutputFormat> for OutputFormat {
    fn from(format: crate::OutputFormat) -> Self {
        match format {
            crate::OutputFormat::Jsonl => OutputFormat::Jsonl,
            crate::OutputFormat::Default => OutputFormat::Default,
            crate::OutputFormat::Logfmt => OutputFormat::Logfmt,
            crate::OutputFormat::Csv => OutputFormat::Csv,
            crate::OutputFormat::Tsv => OutputFormat::Tsv,
            crate::OutputFormat::Csvnh => OutputFormat::Csvnh,
            crate::OutputFormat::Tsvnh => OutputFormat::Tsvnh,
            crate::OutputFormat::Hide => OutputFormat::Hide,
            crate::OutputFormat::Null => OutputFormat::Null,
        }
    }
}

impl From<OutputFormat> for crate::OutputFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Jsonl => crate::OutputFormat::Jsonl,
            OutputFormat::Default => crate::OutputFormat::Default,
            OutputFormat::Logfmt => crate::OutputFormat::Logfmt,
            OutputFormat::Csv => crate::OutputFormat::Csv,
            OutputFormat::Tsv => crate::OutputFormat::Tsv,
            OutputFormat::Csvnh => crate::OutputFormat::Csvnh,
            OutputFormat::Tsvnh => crate::OutputFormat::Tsvnh,
            OutputFormat::Hide => crate::OutputFormat::Hide,
            OutputFormat::Null => crate::OutputFormat::Null,
        }
    }
}


impl From<crate::FileOrder> for FileOrder {
    fn from(order: crate::FileOrder) -> Self {
        match order {
            crate::FileOrder::None => FileOrder::None,
            crate::FileOrder::Name => FileOrder::Name,
            crate::FileOrder::Mtime => FileOrder::Mtime,
        }
    }
}

impl From<FileOrder> for crate::FileOrder {
    fn from(order: FileOrder) -> Self {
        match order {
            FileOrder::None => crate::FileOrder::None,
            FileOrder::Name => crate::FileOrder::Name,
            FileOrder::Mtime => crate::FileOrder::Mtime,
        }
    }
}
