use std::cell::RefCell;
use std::time::{Duration, Instant};

/// Statistics collected during log processing
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    pub lines_read: usize,
    pub lines_output: usize,
    pub lines_filtered: usize,
    pub events_created: usize,
    pub events_output: usize,
    pub events_filtered: usize,
    pub files_processed: usize,
    pub script_executions: usize,
    pub errors: usize,
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

    pub fn format_stats(&self, multiline_enabled: bool) -> String {
        let mut output = String::new();

        if multiline_enabled && self.events_created > 0 {
            // In multiline mode, show both lines and events
            output.push_str(&format!(
                "Lines processed: {} total, {} filtered; Events created: {} total, {} output, {} filtered",
                self.lines_read, self.lines_filtered, self.events_created, self.events_output, self.events_filtered
            ));
        } else {
            // In non-multiline mode, show traditional line stats
            output.push_str(&format!(
                "Lines processed: {} total, {} output, {} filtered",
                self.lines_read, self.lines_output, self.lines_filtered
            ));
        }

        if self.files_processed > 0 {
            output.push_str(&format!(", {} files", self.files_processed));
        }

        if self.errors > 0 {
            output.push_str(&format!(", {} errors", self.errors));
        }

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
