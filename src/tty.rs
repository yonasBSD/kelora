use std::io::IsTerminal;
use crate::config::ColorMode;

/// Check if stdout is connected to a TTY
pub fn is_stdout_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Determine if colors should be used based on CLI color mode and environment
pub fn should_use_colors_with_mode(color_mode: &ColorMode) -> bool {
    match color_mode {
        ColorMode::Never => false,
        ColorMode::Always => {
            // Even with Always, respect NO_COLOR for security/accessibility
            std::env::var("NO_COLOR").is_err()
        },
        ColorMode::Auto => should_use_colors_auto(),
    }
}


/// Auto color detection logic
fn should_use_colors_auto() -> bool {
    // Don't use colors if not on TTY
    if !is_stdout_tty() {
        return false;
    }

    // Respect NO_COLOR environment variable (https://no-color.org/)
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Check FORCE_COLOR for CI environments that support colors
    if std::env::var("FORCE_COLOR").is_ok() {
        return true;
    }

    // Default: use colors for TTY
    true
}