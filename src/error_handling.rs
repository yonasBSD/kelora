use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use serde_json::json;

/// Error severity levels according to the specification
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Fatal,   // I/O failure, panic
    Hard,    // Rhai error, CLI misuse, bad regex  
    Medium,  // Parse failure, CSV mismatch
    Soft,    // Missing field, null, coercion fail
}

/// Error information for tracking and reporting
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub severity: ErrorSeverity,
    pub message: String,
    pub context: Option<String>,
    pub line_number: Option<usize>,
}

/// Error reporting and collection system
#[derive(Debug)]
pub struct ErrorReporter {
    pub config: crate::config::ErrorReportConfig,
    errors: Vec<ErrorInfo>,
    error_counts: HashMap<String, usize>,
    error_examples: HashMap<String, Vec<String>>,
}

impl ErrorReporter {
    pub fn new(config: crate::config::ErrorReportConfig) -> Self {
        Self {
            config,
            errors: Vec::new(),
            error_counts: HashMap::new(),
            error_examples: HashMap::new(),
        }
    }

    /// Report an error according to the configured strategy
    pub fn report_error(&mut self, error: ErrorInfo) -> bool {
        let should_continue = match (&self.config.style, &error.severity) {
            // Fatal errors always printed and cause exit
            (_, ErrorSeverity::Fatal) => {
                eprintln!("{}", crate::config::format_error_message_auto(&error.message));
                false
            }
            // Hard errors always printed and cause exit  
            (_, ErrorSeverity::Hard) => {
                eprintln!("{}", crate::config::format_error_message_auto(&error.message));
                false
            }
            // Medium and Soft errors depend on reporting style
            (crate::config::ErrorReportStyle::Off, _) => {
                // Suppress all non-fatal error messages, but still track them
                self.track_error(&error);
                true
            }
            (crate::config::ErrorReportStyle::Print, _) => {
                // Print each error immediately
                eprintln!("{}", crate::config::format_error_message_auto(&error.message));
                self.track_error(&error);
                true
            }
            (crate::config::ErrorReportStyle::Summary, _) => {
                // Just track for summary at end
                self.track_error(&error);
                true
            }
        };

        self.errors.push(error);
        should_continue
    }

    /// Track error for summary reporting
    fn track_error(&mut self, error: &ErrorInfo) {
        let error_type = format!("{:?}", error.severity);
        *self.error_counts.entry(error_type.clone()).or_insert(0) += 1;
        
        let examples = self.error_examples.entry(error_type).or_insert_with(Vec::new);
        if examples.len() < 3 {
            examples.push(error.message.clone());
        }
    }

    /// Generate summary report
    pub fn generate_summary(&self) -> Option<String> {
        if self.errors.is_empty() {
            return None;
        }

        match self.config.style {
            crate::config::ErrorReportStyle::Summary => {
                let mut summary = json!({});
                
                for (error_type, count) in &self.error_counts {
                    let empty_examples = Vec::new();
                    let examples = self.error_examples.get(error_type).unwrap_or(&empty_examples);
                    summary[error_type] = json!({
                        "count": count,
                        "examples": examples
                    });
                }
                
                Some(serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "Error serializing summary".to_string()))
            }
            _ => None,
        }
    }

    /// Write summary to file if configured
    pub fn write_summary_to_file(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref file_path) = self.config.file {
            if let Some(summary) = self.generate_summary() {
                let mut file = File::create(file_path)?;
                file.write_all(summary.as_bytes())?;
            }
        }
        Ok(())
    }

    /// Check if any fatal or hard errors occurred (for exit code determination)
    pub fn has_fatal_or_hard_errors(&self) -> bool {
        self.errors.iter().any(|e| matches!(e.severity, ErrorSeverity::Fatal | ErrorSeverity::Hard))
    }

    /// Check if any errors occurred at all
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get total error count
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}

/// Helper functions to create errors with appropriate severity
pub fn create_fatal_error(message: String, context: Option<String>) -> ErrorInfo {
    ErrorInfo {
        severity: ErrorSeverity::Fatal,
        message,
        context,
        line_number: None,
    }
}

pub fn create_hard_error(message: String, context: Option<String>) -> ErrorInfo {
    ErrorInfo {
        severity: ErrorSeverity::Hard,
        message,
        context,
        line_number: None,
    }
}

pub fn create_medium_error(message: String, context: Option<String>) -> ErrorInfo {
    ErrorInfo {
        severity: ErrorSeverity::Medium,
        message,
        context,
        line_number: None,
    }
}

pub fn create_soft_error(message: String, context: Option<String>) -> ErrorInfo {
    ErrorInfo {
        severity: ErrorSeverity::Soft,
        message,
        context,
        line_number: None,
    }
}