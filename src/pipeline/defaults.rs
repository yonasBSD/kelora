use crate::event::Event;
use super::{Chunker, WindowManager, OutputWriter, EventLimiter};

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