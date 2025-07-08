use crate::config::ColorMode;
use std::io::IsTerminal;

/// Check if stdout is connected to a TTY
pub fn is_stdout_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Check if stdin is connected to a TTY
pub fn is_stdin_tty() -> bool {
    std::io::stdin().is_terminal()
}

/// Determine if colors should be used based on CLI color mode and environment
pub fn should_use_colors_with_mode(color_mode: &ColorMode) -> bool {
    match color_mode {
        ColorMode::Never => false,
        ColorMode::Always => {
            // --force-color should override NO_COLOR environment variable
            true
        }
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
