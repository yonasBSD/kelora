use super::{Chunker, EventLimiter, OutputWriter, WindowManager};
use crate::event::Event;
use std::collections::VecDeque;

/// Default implementations for pipeline stages
///
/// Simple pass-through chunker (no multi-line support)
pub struct SimpleChunker;

impl Chunker for SimpleChunker {
    fn feed_line(&mut self, line: String) -> Option<String> {
        Some(line)
    }

    fn flush(&mut self) -> Option<String> {
        None
    }
}

/// Simple window manager (no windowing support)
pub struct SimpleWindowManager {
    current: Option<Event>,
}

impl SimpleWindowManager {
    pub fn new() -> Self {
        Self { current: None }
    }
}

impl WindowManager for SimpleWindowManager {
    fn get_window(&self) -> Vec<Event> {
        if let Some(ref event) = self.current {
            vec![event.clone()]
        } else {
            Vec::new()
        }
    }

    fn update(&mut self, current: &Event) {
        self.current = Some(current.clone());
    }
}

/// Standard output writer
pub struct StdoutWriter;

impl OutputWriter for StdoutWriter {
    fn write(&mut self, line: &str) -> std::io::Result<()> {
        println!("{}", line);
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        use std::io::Write;
        std::io::stdout().flush()
    }
}

/// Simple event limiter for --take N
pub struct TakeNLimiter {
    remaining: usize,
}

impl TakeNLimiter {
    pub fn new(limit: usize) -> Self {
        Self { remaining: limit }
    }
}

impl EventLimiter for TakeNLimiter {
    fn allow(&mut self) -> bool {
        if self.remaining > 0 {
            self.remaining -= 1;
            true
        } else {
            false
        }
    }
}

/// Sliding window manager that maintains a configurable window of recent events
///
/// The window maintains events in order: [current, previous, older...]
/// - window[0] = current event  
/// - window[1] = previous event
/// - window[2] = event before that, etc.
///
/// When window_size=N, we keep N+1 events total (current + N previous).
/// For example, --window 2 gives access to window[0], window[1], window[2].
pub struct SlidingWindowManager {
    window_size: usize,
    buffer: VecDeque<Event>,
}

impl SlidingWindowManager {
    /// Create new sliding window manager with specified window size
    ///
    /// # Arguments
    /// * `window_size` - Number of previous events to keep (0 = only current event)
    ///
    /// # Examples
    /// ```
    /// use kelora::pipeline::defaults::SlidingWindowManager;
    /// // Keep current + 2 previous events (window[0], window[1], window[2])
    /// let manager = SlidingWindowManager::new(2);
    /// ```
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            buffer: VecDeque::with_capacity(window_size + 1),
        }
    }
}

impl WindowManager for SlidingWindowManager {
    /// Get current window of events
    ///
    /// Returns events in order: [current, previous, older...]
    /// The returned vector always has the current event at index 0.
    fn get_window(&self) -> Vec<Event> {
        self.buffer.iter().cloned().collect()
    }

    /// Update window with new current event
    ///
    /// The new event becomes window[0], previous events shift:
    /// - Old window[0] becomes window[1]  
    /// - Old window[1] becomes window[2]
    /// - etc.
    ///
    /// If buffer exceeds window_size+1, oldest events are discarded.
    fn update(&mut self, current: &Event) {
        // Add new event to front (becomes window[0])
        self.buffer.push_front(current.clone());

        // Remove excess events beyond window_size + 1 (current + N previous)
        while self.buffer.len() > self.window_size + 1 {
            self.buffer.pop_back();
        }
    }
}
