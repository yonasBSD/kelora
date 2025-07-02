use anyhow::Result;
use std::io::{self, Write};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

// Cross-platform signal handling
#[cfg(unix)]
use signal_hook::{consts::SIGINT, consts::SIGPIPE, consts::SIGTERM, iterator::Signals};

#[cfg(windows)]
use signal_hook::{consts::SIGINT, iterator::Signals};

/// Standard Unix exit codes
#[derive(Debug, Clone, Copy)]
pub enum ExitCode {
    Success = 0,
    GeneralError = 1,
    InvalidUsage = 2,
    SignalInt = 130,    // 128 + SIGINT (2)
    SignalPipe = 141,   // 128 + SIGPIPE (13)  
    SignalTerm = 143,   // 128 + SIGTERM (15)
}

impl ExitCode {
    pub fn exit(self) -> ! {
        process::exit(self as i32)
    }
}

/// Global termination flag for graceful shutdown
pub static SHOULD_TERMINATE: AtomicBool = AtomicBool::new(false);

/// Signal handler for graceful shutdown
pub struct SignalHandler {
    _handle: thread::JoinHandle<()>,
}

impl SignalHandler {
    /// Initialize signal handling - cross-platform
    pub fn new() -> Result<Self> {
        #[cfg(unix)]
        let signals_to_handle = vec![SIGINT, SIGPIPE, SIGTERM];
        
        #[cfg(windows)]
        let signals_to_handle = vec![SIGINT]; // Windows only supports SIGINT reliably
        
        let mut signals = Signals::new(&signals_to_handle)?;
        
        let handle = thread::spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGINT => {
                        SHOULD_TERMINATE.store(true, Ordering::Relaxed);
                        // Give main thread a moment to handle graceful shutdown
                        thread::sleep(std::time::Duration::from_millis(100));
                        // If still running after grace period, exit immediately
                        ExitCode::SignalInt.exit();
                    }
                    #[cfg(unix)]
                    SIGPIPE => {
                        // Broken pipe - exit quietly (normal for Unix pipes)
                        SHOULD_TERMINATE.store(true, Ordering::Relaxed);
                        ExitCode::SignalPipe.exit();
                    }
                    #[cfg(unix)]
                    SIGTERM => {
                        eprintln!("{}", crate::config::format_error_message_auto("Received SIGTERM, shutting down gracefully..."));
                        SHOULD_TERMINATE.store(true, Ordering::Relaxed);
                        ExitCode::SignalTerm.exit();
                    }
                    _ => {
                        // Unknown signal - should not happen with our registration
                        eprintln!("{}", crate::config::format_error_message_auto(&format!("Received unexpected signal: {}", sig)));
                    }
                }
            }
        });

        Ok(SignalHandler { _handle: handle })
    }

    /// Check if we should terminate processing
    pub fn should_terminate() -> bool {
        SHOULD_TERMINATE.load(Ordering::Relaxed)
    }
}

/// Safe wrapper for writing to stdout that handles broken pipes and other I/O errors
pub struct SafeStdout {
    stdout: io::Stdout,
}

impl SafeStdout {
    pub fn new() -> Self {
        Self {
            stdout: io::stdout(),
        }
    }

    /// Write a line to stdout, handling broken pipes gracefully (cross-platform)
    pub fn writeln(&mut self, data: &str) -> Result<()> {
        match writeln!(self.stdout, "{}", data) {
            Ok(()) => Ok(()),
            Err(e) if Self::is_broken_pipe(&e) => {
                // Broken pipe is normal in pipelines - exit quietly
                ExitCode::SignalPipe.exit();
            }
            Err(e) => {
                // Other I/O errors should be reported
                Err(anyhow::anyhow!("Failed to write to stdout: {}", e))
            }
        }
    }

    /// Flush stdout, handling errors gracefully (cross-platform)
    pub fn flush(&mut self) -> Result<()> {
        match self.stdout.flush() {
            Ok(()) => Ok(()),
            Err(e) if Self::is_broken_pipe(&e) => {
                // Broken pipe is normal - exit quietly
                ExitCode::SignalPipe.exit();
            }
            Err(e) => {
                // Other flush errors should be reported
                Err(anyhow::anyhow!("Failed to flush stdout: {}", e))
            }
        }
    }

    /// Cross-platform broken pipe detection
    fn is_broken_pipe(e: &io::Error) -> bool {
        #[cfg(unix)]
        {
            e.kind() == io::ErrorKind::BrokenPipe
        }
        #[cfg(windows)]
        {
            // On Windows, broken pipe manifests as different error codes
            e.kind() == io::ErrorKind::BrokenPipe 
                || e.raw_os_error() == Some(232) // ERROR_NO_DATA "The pipe is being closed"
                || e.raw_os_error() == Some(109) // ERROR_BROKEN_PIPE "The pipe has been ended"
        }
    }
}

/// Safe wrapper for writing to stderr
pub struct SafeStderr {
    stderr: io::Stderr,
}

impl SafeStderr {
    pub fn new() -> Self {
        Self {
            stderr: io::stderr(),
        }
    }

    /// Write a line to stderr, handling errors gracefully
    pub fn writeln(&mut self, data: &str) -> Result<()> {
        match writeln!(self.stderr, "{}", data) {
            Ok(()) => Ok(()),
            Err(e) => {
                // If we can't write to stderr, there's not much we can do
                // Just exit with a general error
                eprintln!("{}", crate::config::format_error_message_auto(&format!("Fatal: Failed to write to stderr: {}", e)));
                ExitCode::GeneralError.exit();
            }
        }
    }
}

/// Utility function to check for termination between processing steps
pub fn check_termination() -> Result<()> {
    if SignalHandler::should_terminate() {
        return Err(anyhow::anyhow!("Processing terminated by signal"));
    }
    Ok(())
}

/// Process cleanup utilities
pub struct ProcessCleanup {
    cleanup_tasks: Vec<Box<dyn FnOnce() + Send>>,
}

impl ProcessCleanup {
    pub fn new() -> Self {
        Self {
            cleanup_tasks: Vec::new(),
        }
    }
}

impl Drop for ProcessCleanup {
    fn drop(&mut self) {
        // If ProcessCleanup is dropped without explicit cleanup,
        // we should still try to clean up
        while let Some(task) = self.cleanup_tasks.pop() {
            task();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_codes() {
        assert_eq!(ExitCode::Success as i32, 0);
        assert_eq!(ExitCode::GeneralError as i32, 1);
        assert_eq!(ExitCode::InvalidUsage as i32, 2);
        assert_eq!(ExitCode::SignalInt as i32, 130);
        assert_eq!(ExitCode::SignalPipe as i32, 141);
        assert_eq!(ExitCode::SignalTerm as i32, 143);
    }

    #[test]
    fn test_should_terminate_initial_state() {
        // Should start as false
        assert!(!SignalHandler::should_terminate());
    }
}