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
// Note: These functions are conditionally called based on config.output.stats flag
#[allow(dead_code)] // Used conditionally in lib.rs when stats are enabled
pub fn stats_add_line_read() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().lines_read += 1;
    });
}

#[allow(dead_code)] // Used conditionally in lib.rs when stats are enabled
pub fn stats_add_line_output() {
    THREAD_STATS.with(|stats| {
        stats.borrow_mut().lines_output += 1;
    });
}

#[allow(dead_code)] // Used conditionally in lib.rs when stats are enabled
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

#[allow(dead_code)] // Used conditionally in lib.rs when stats are enabled
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

    /// Format stats according to the specification
    #[allow(dead_code)] // Used in main.rs when stats are enabled
    pub fn format_stats(&self, _multiline_enabled: bool) -> String {
        let mut output = String::new();

        // lines_in = Input lines read
        output.push_str(&format!("lines_in   = {}\n", self.lines_read));

        // lines_out = Events emitted after parsing/filtering
        output.push_str(&format!("lines_out  = {}\n", self.events_output));

        // duration = Total run time, human-readable
        let duration_secs = self.processing_time.as_secs_f64();
        if duration_secs < 1.0 {
            output.push_str(&format!(
                "duration   = {:.0}ms\n",
                self.processing_time.as_millis()
            ));
        } else {
            output.push_str(&format!("duration   = {:.2}s\n", duration_secs));
        }

        // throughput = Processing rate
        if duration_secs > 0.0 && self.lines_read > 0 {
            let throughput = self.lines_read as f64 / duration_secs;
            if throughput >= 1000.0 {
                output.push_str(&format!("throughput = {:.1}k/s\n", throughput / 1000.0));
            } else {
                output.push_str(&format!("throughput = {:.0}/s\n", throughput));
            }
        }

        // TODO: levels and keys will be populated from discovered field names
        // For now, leave them as placeholders
        output.push_str("levels     = \n");
        output.push_str("keys       = \n");

        output.trim_end().to_string()
    }
}
