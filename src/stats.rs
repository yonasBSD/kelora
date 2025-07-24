use std::cell::RefCell;
use std::collections::BTreeSet;
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
    pub discovered_levels: BTreeSet<String>,
    pub discovered_keys: BTreeSet<String>,
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

    /// Extract discovered levels and keys from tracking data (for sequential processing)
    #[allow(dead_code)] // Used in sequential processing, but clippy doesn't detect it properly
    pub fn extract_discovered_from_tracking(
        &mut self,
        tracking_data: &std::collections::HashMap<String, rhai::Dynamic>,
    ) {
        // Extract discovered levels from tracking data
        if let Some(levels_dynamic) = tracking_data.get("__kelora_stats_discovered_levels") {
            if let Ok(levels_array) = levels_dynamic.clone().into_array() {
                for level in levels_array {
                    if let Ok(level_str) = level.into_string() {
                        self.discovered_levels.insert(level_str);
                    }
                }
            }
        }

        // Extract discovered keys from tracking data
        if let Some(keys_dynamic) = tracking_data.get("__kelora_stats_discovered_keys") {
            if let Ok(keys_array) = keys_dynamic.clone().into_array() {
                for key in keys_array {
                    if let Ok(key_str) = key.into_string() {
                        self.discovered_keys.insert(key_str);
                    }
                }
            }
        }
    }

    /// Format stats according to the specification
    #[allow(dead_code)] // Used in main.rs when stats are enabled
    pub fn format_stats(&self, _multiline_enabled: bool) -> String {
        let mut output = String::new();

        // Lines processed: N total, N filtered, N errors
        output.push_str(&format!(
            "Lines processed: {} total, {} filtered, {} errors\n",
            self.lines_read, self.lines_filtered, self.lines_errors
        ));

        // Events created: N total, N output, N filtered
        output.push_str(&format!(
            "Events created: {} total, {} output, {} filtered\n",
            self.events_created, self.events_output, self.events_filtered
        ));

        // Throughput: N lines/s in Nms
        let duration_secs = self.processing_time.as_secs_f64();
        if duration_secs > 0.0 && self.lines_read > 0 {
            let throughput = self.lines_read as f64 / duration_secs;
            if duration_secs < 1.0 {
                output.push_str(&format!(
                    "Throughput: {:.0} lines/s in {:.0}ms\n",
                    throughput,
                    self.processing_time.as_millis()
                ));
            } else {
                output.push_str(&format!(
                    "Throughput: {:.0} lines/s in {:.2}s\n",
                    throughput, duration_secs
                ));
            }
        }

        // Levels seen: (only if we have discovered levels)
        if !self.discovered_levels.is_empty() {
            let levels: Vec<String> = self.discovered_levels.iter().cloned().collect();
            output.push_str(&format!("Levels seen: {}\n", levels.join(",")));
        }

        // Keys seen: (only if we have discovered keys)
        if !self.discovered_keys.is_empty() {
            let keys: Vec<String> = self.discovered_keys.iter().cloned().collect();
            output.push_str(&format!("Keys seen: {}\n", keys.join(",")));
        }

        output.trim_end().to_string()
    }

    /// Check if any errors occurred during processing
    #[allow(dead_code)] // Used by main.rs binary target, not detected by clippy in lib context
    pub fn has_errors(&self) -> bool {
        self.lines_errors > 0
    }

    /// Format a concise error summary for default output (when errors occur)
    #[allow(dead_code)] // Used by main.rs binary target, not detected by clippy in lib context
    pub fn format_error_summary(&self) -> String {
        if !self.has_errors() {
            return String::new();
        }

        let mut parts = Vec::new();

        // Show parse errors
        if self.lines_errors > 0 {
            parts.push(format!("{} parse error{}", 
                self.lines_errors, 
                if self.lines_errors == 1 { "" } else { "s" }
            ));
        }

        // Show events filtered (could indicate filter errors converted to false)
        if self.events_filtered > 0 {
            parts.push(format!("{} event{} filtered", 
                self.events_filtered,
                if self.events_filtered == 1 { "" } else { "s" }
            ));
        }

        if parts.is_empty() {
            return String::new();
        }

        format!("⚠️  Processing completed with {}", parts.join(", "))
    }
}
