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
    pub ignore_lines: Option<regex::Regex>,
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
    pub stats: bool,
}

/// Ordered script stages that preserve CLI order
#[derive(Debug, Clone)]
pub enum ScriptStageType {
    Filter(String),
    Exec(String),
}

/// Processing configuration
#[derive(Debug, Clone)]
pub struct ProcessingConfig {
    pub begin: Option<String>,
    pub stages: Vec<ScriptStageType>,
    pub end: Option<String>,
    pub no_inject_fields: bool,
    pub inject_prefix: Option<String>,
    pub on_error: ErrorStrategy,
    pub levels: Vec<String>,
    pub exclude_levels: Vec<String>,
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
    Csv,
    Apache,
    Nginx,
}

/// Output format enumeration
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    Jsonl,
    #[default]
    Default,
    Logfmt,
    Csv,
}

/// Error handling strategy
#[derive(ValueEnum, Clone, Debug)]
pub enum ErrorStrategy {
    Skip,
    Abort,
    Print,
    Stub,
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

impl KeloraConfig {
    /// Get the list of core field names (timestamp, level, message variants)
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
        let use_colors = crate::tty::should_use_colors_with_mode(&self.output.color);
        let use_emoji = use_colors && !self.output.no_emoji;

        if use_emoji {
            format!("🧱 {}", message)
        } else {
            format!("Stats: {}", message)
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
                ignore_lines: None, // Will be set after CLI parsing
            },
            output: OutputConfig {
                format: cli.output_format.clone().into(),
                keys: cli.keys.clone(),
                exclude_keys: cli.exclude_keys.clone(),
                core: cli.core,
                brief: cli.brief,
                color: color_mode,
                no_emoji: cli.no_emoji,
                stats: cli.stats,
            },
            processing: ProcessingConfig {
                begin: cli.begin.clone(),
                stages: Vec::new(), // Will be set by main() after CLI parsing
                end: cli.end.clone(),
                no_inject_fields: cli.no_inject_fields,
                inject_prefix: cli.inject_prefix.clone(),
                on_error: cli.on_error.clone().into(),
                levels: cli.levels.clone(),
                exclude_levels: cli.exclude_levels.clone(),
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
                ignore_lines: None,
            },
            output: OutputConfig {
                format: OutputFormat::Default,
                keys: Vec::new(),
                exclude_keys: Vec::new(),
                core: false,
                brief: false,
                color: ColorMode::Auto,
                no_emoji: false,
                stats: false,
            },
            processing: ProcessingConfig {
                begin: None,
                stages: Vec::new(),
                end: None,
                no_inject_fields: false,
                inject_prefix: None,
                on_error: ErrorStrategy::Print,
                levels: Vec::new(),
                exclude_levels: Vec::new(),
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

// Conversion traits to maintain compatibility with existing CLI types
impl From<crate::InputFormat> for InputFormat {
    fn from(format: crate::InputFormat) -> Self {
        match format {
            crate::InputFormat::Jsonl => InputFormat::Jsonl,
            crate::InputFormat::Line => InputFormat::Line,
            crate::InputFormat::Logfmt => InputFormat::Logfmt,
            crate::InputFormat::Syslog => InputFormat::Syslog,
            crate::InputFormat::Csv => InputFormat::Csv,
            crate::InputFormat::Apache => InputFormat::Apache,
            crate::InputFormat::Nginx => InputFormat::Nginx,
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
            InputFormat::Csv => crate::InputFormat::Csv,
            InputFormat::Apache => crate::InputFormat::Apache,
            InputFormat::Nginx => crate::InputFormat::Nginx,
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
        }
    }
}

impl From<crate::ErrorStrategy> for ErrorStrategy {
    fn from(strategy: crate::ErrorStrategy) -> Self {
        match strategy {
            crate::ErrorStrategy::Skip => ErrorStrategy::Skip,
            crate::ErrorStrategy::Abort => ErrorStrategy::Abort,
            crate::ErrorStrategy::Print => ErrorStrategy::Print,
            crate::ErrorStrategy::Stub => ErrorStrategy::Stub,
        }
    }
}

impl From<ErrorStrategy> for crate::ErrorStrategy {
    fn from(strategy: ErrorStrategy) -> Self {
        match strategy {
            ErrorStrategy::Skip => crate::ErrorStrategy::Skip,
            ErrorStrategy::Abort => crate::ErrorStrategy::Abort,
            ErrorStrategy::Print => crate::ErrorStrategy::Print,
            ErrorStrategy::Stub => crate::ErrorStrategy::Stub,
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
