use std::cell::RefCell;
use std::time::{Duration, Instant};

/// Statistics collected during log processing
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    pub lines_read: usize,
    pub lines_output: usize,
    pub lines_filtered: usize,
    pub lines_errors: usize, // Parse errors (regardless of error handling strategy)
    pub events_created: usize,
    pub events_output: usize,
    pub events_filtered: usize,
    pub files_processed: usize,
    pub script_executions: usize,
    pub errors: usize, // Kept for backward compatibility, but lines_errors is more specific
    pub processing_time: Duration,
    pub start_time: Option<Instant>,
}

// Thread-local storage for statistics (following track_count pattern)
thread_local! {
    static THREAD_STATS: RefCell<ProcessingStats> = RefCell::new(ProcessingStats::new());
}

// Public API functions for stats collection (following track_count pattern)
pub fn stats_add_line_read() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().lines_read += 1;
    });
}

pub fn stats_add_line_output() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().lines_output += 1;
    });
}

pub fn stats_add_line_filtered() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().lines_filtered += 1;
    });
}

pub fn stats_add_line_error() {
    THREAD_STATS.with(|stats| {
        let mut stats = stats.borrow_mut();
        stats.lines_errors += 1;
        stats.errors += 1; // Keep both for backward compatibility
    });
}

pub fn stats_add_event_created() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().events_created += 1;
    });
}

pub fn stats_add_event_output() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().events_output += 1;
    });
}

pub fn stats_add_event_filtered() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().events_filtered += 1;
    });
}

pub fn stats_add_error() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().errors += 1;
    });
}

pub fn stats_start_timer() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().start_time = Some(Instant::now());
    });
}

pub fn stats_finish_processing() {
    THREAD_STATS.with(|stats| {
        let mut stats = stats.borrow_mut();
        if let Some(start) = stats.start_time {
            stats.processing_time = start.elapsed();
        }
    });
}

pub fn get_thread_stats() -> ProcessingStats {
    THREAD_STATS.with(|stats| stats.borrow().clone())
}

impl ProcessingStats {
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }

    pub fn format_stats(&self, _multiline_enabled: bool) -> String {
        let mut output = String::new();

        // Always show both lines and events for consistency and accuracy
        // This clearly separates line-level filtering (ignore-lines) from event-level filtering (filter expressions)
        if self.lines_errors > 0 {
            output.push_str(&format!(
                "Lines processed: {} total, {} filtered, {} errors; Events created: {} total, {} output, {} filtered",
                self.lines_read, self.lines_filtered, self.lines_errors, self.events_created, self.events_output, self.events_filtered
            ));
        } else {
            output.push_str(&format!(
                "Lines processed: {} total, {} filtered; Events created: {} total, {} output, {} filtered",
                self.lines_read, self.lines_filtered, self.events_created, self.events_output, self.events_filtered
            ));
        }

        if self.files_processed > 0 {
            output.push_str(&format!(", {} files", self.files_processed));
        }

        // Don't show generic errors count anymore since it's already in the main stats line

        let processing_time_ms = self.processing_time.as_millis();
        output.push_str(&format!(" in {}ms", processing_time_ms));

        if processing_time_ms > 0 && self.lines_read > 0 {
            let lines_per_sec = (self.lines_read as f64 * 1000.0) / processing_time_ms as f64;
            output.push_str(&format!(" ({:.0} lines/s)", lines_per_sec));
        }

        if self.script_executions > 0 {
            output.push_str(&format!(", {} script executions", self.script_executions));
        }

        output
    }
}
