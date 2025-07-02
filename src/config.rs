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
}

/// Output configuration
#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub keys: Vec<String>,
    pub exclude_keys: Vec<String>,
    pub plain: bool,
    pub color: ColorMode,
}

/// Processing configuration
#[derive(Debug, Clone)]
pub struct ProcessingConfig {
    pub begin: Option<String>,
    pub filters: Vec<String>,
    pub execs: Vec<String>,
    pub end: Option<String>,
    pub no_inject_fields: bool,
    pub inject_prefix: Option<String>,
    pub on_error: ErrorStrategy,
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
    /// Create configuration from CLI arguments
    pub fn from_cli(cli: &crate::Cli) -> Self {
        Self {
            input: InputConfig {
                files: cli.files.clone(),
                format: cli.format.clone().into(),
                file_order: cli.file_order.clone().into(),
            },
            output: OutputConfig {
                format: cli.output_format.clone().into(),
                keys: cli.keys.clone(),
                exclude_keys: cli.exclude_keys.clone(),
                plain: cli.plain,
                color: cli.color.clone().into(),
            },
            processing: ProcessingConfig {
                begin: cli.begin.clone(),
                filters: cli.filters.clone(),
                execs: cli.execs.clone(),
                end: cli.end.clone(),
                no_inject_fields: cli.no_inject_fields,
                inject_prefix: cli.inject_prefix.clone(),
                on_error: cli.on_error.clone().into(),
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
            },
            output: OutputConfig {
                format: OutputFormat::Default,
                keys: Vec::new(),
                exclude_keys: Vec::new(),
                plain: false,
                color: ColorMode::Auto,
            },
            processing: ProcessingConfig {
                begin: None,
                filters: Vec::new(),
                execs: Vec::new(),
                end: None,
                no_inject_fields: false,
                inject_prefix: None,
                on_error: ErrorStrategy::Print,
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

impl From<crate::ColorMode> for ColorMode {
    fn from(mode: crate::ColorMode) -> Self {
        match mode {
            crate::ColorMode::Auto => ColorMode::Auto,
            crate::ColorMode::Always => ColorMode::Always,
            crate::ColorMode::Never => ColorMode::Never,
        }
    }
}

impl From<ColorMode> for crate::ColorMode {
    fn from(mode: ColorMode) -> Self {
        match mode {
            ColorMode::Auto => crate::ColorMode::Auto,
            ColorMode::Always => crate::ColorMode::Always,
            ColorMode::Never => crate::ColorMode::Never,
        }
    }
}