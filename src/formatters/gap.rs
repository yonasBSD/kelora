use chrono::{DateTime, Utc};
use std::time::Duration as StdDuration;

/// Helper that tracks time gaps between events and renders markers when needed
#[derive(Debug, Clone)]
pub struct GapTracker {
    threshold: chrono::Duration,
    last_timestamp: Option<DateTime<Utc>>,
    use_colors: bool,
}

impl GapTracker {
    pub fn new(threshold: chrono::Duration, use_colors: bool) -> Self {
        Self {
            threshold,
            last_timestamp: None,
            use_colors,
        }
    }

    /// Returns a marker string if the supplied timestamp is sufficiently far from the last one
    pub fn check(&mut self, timestamp: Option<DateTime<Utc>>) -> Option<String> {
        let current_ts = timestamp?;
        let marker = self.last_timestamp.and_then(|previous_ts| {
            let diff = current_ts.signed_duration_since(previous_ts);
            if diff >= self.threshold || diff <= -self.threshold {
                Some(self.render_marker(diff))
            } else {
                None
            }
        });

        self.last_timestamp = Some(current_ts);
        marker
    }

    fn render_marker(&self, diff: chrono::Duration) -> String {
        let diff = if diff >= chrono::Duration::zero() {
            diff
        } else {
            -diff
        };

        let std_duration = diff.to_std().unwrap_or_else(|_| StdDuration::from_secs(0));

        let total_seconds = std_duration.as_secs();
        let micros = std_duration.subsec_micros();

        // Calculate time units
        let years = total_seconds / (365 * 24 * 3600);
        let remaining_after_years = total_seconds % (365 * 24 * 3600);
        let days = remaining_after_years / (24 * 3600);
        let remaining_after_days = remaining_after_years % (24 * 3600);
        let hours = remaining_after_days / 3600;
        let minutes = (remaining_after_days % 3600) / 60;
        let seconds = remaining_after_days % 60;

        // Build humanized time string
        let mut parts = Vec::new();

        if years > 0 {
            parts.push(format!(
                "{} year{}",
                years,
                if years == 1 { "" } else { "s" }
            ));
        }
        if days > 0 {
            parts.push(format!("{} day{}", days, if days == 1 { "" } else { "s" }));
        }
        if hours > 0 {
            parts.push(format!(
                "{} hour{}",
                hours,
                if hours == 1 { "" } else { "s" }
            ));
        }
        if minutes > 0 {
            parts.push(format!(
                "{} minute{}",
                minutes,
                if minutes == 1 { "" } else { "s" }
            ));
        }
        if seconds > 0 || parts.is_empty() {
            if micros > 0 {
                let mut fractional = format!("{:06}", micros);
                while fractional.ends_with('0') {
                    fractional.pop();
                }
                parts.push(format!(
                    "{}.{} second{}",
                    seconds,
                    fractional,
                    if seconds == 1 && micros == 0 { "" } else { "s" }
                ));
            } else {
                parts.push(format!(
                    "{} second{}",
                    seconds,
                    if seconds == 1 { "" } else { "s" }
                ));
            }
        }

        let time_label = parts.join(" ");
        let label = format!(" time gap: {} ", time_label);

        let blue = "\x1b[34m";
        let reset = "\x1b[0m";

        let mut width = crate::tty::get_terminal_width();
        if width == 0 {
            width = 80;
        }

        if width <= label.len() {
            let marker = label.trim().to_string();
            if self.use_colors {
                return format!("{}{}{}", blue, marker, reset);
            }
            return marker;
        }

        let remaining = width - label.len();
        let left = remaining / 2;
        let right = remaining - left;

        let mut marker = String::with_capacity(width);
        marker.push_str(&"_".repeat(left));
        marker.push_str(&label);
        marker.push_str(&"_".repeat(right));
        if self.use_colors {
            return format!("{}{}{}", blue, marker, reset);
        }
        marker
    }
}
