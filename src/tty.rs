#![allow(dead_code)]
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

/// Get terminal width for word-wrapping, with fallback to default width
pub fn get_terminal_width() -> usize {
    if let Some((terminal_size::Width(width), _)) = terminal_size::terminal_size() {
        width as usize
    } else {
        100 // Default fallback width as requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct EnvGuard {
        vars: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&'static str]) -> Self {
            let vars = keys
                .iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect();
            Self { vars }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.vars {
                if let Some(v) = value {
                    std::env::set_var(key, v);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    fn with_env_lock<F: FnOnce()>(keys: &[&'static str], f: F) {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard::new(keys);
        f();
    }

    #[test]
    fn color_mode_never_disables_colors() {
        assert!(!should_use_colors_with_mode(&ColorMode::Never));
    }

    #[test]
    fn color_mode_always_overrides_no_color_environment() {
        with_env_lock(&["NO_COLOR"], || {
            std::env::set_var("NO_COLOR", "1");
            assert!(should_use_colors_with_mode(&ColorMode::Always));
        });
    }
}
