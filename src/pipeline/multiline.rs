use super::Chunker;
use crate::config::{MultilineConfig, MultilineStrategy};
use regex::Regex;

/// Multi-line chunker that implements various strategies for detecting event boundaries
pub struct MultilineChunker {
    config: MultilineConfig,
    buffer: Vec<String>,
    regex: Option<Regex>,
}

impl MultilineChunker {
    pub fn new(config: MultilineConfig) -> Result<Self, String> {
        let regex = match &config.strategy {
            MultilineStrategy::Timestamp { pattern } => {
                Some(Regex::new(pattern).map_err(|e| format!("Invalid timestamp regex: {}", e))?)
            }
            MultilineStrategy::Start { pattern } => {
                Some(Regex::new(pattern).map_err(|e| format!("Invalid start regex: {}", e))?)
            }
            MultilineStrategy::End { pattern } => {
                Some(Regex::new(pattern).map_err(|e| format!("Invalid end regex: {}", e))?)
            }
            MultilineStrategy::Boundary { start, end: _ } => {
                // We'll compile both patterns, but store start pattern here
                Some(
                    Regex::new(start)
                        .map_err(|e| format!("Invalid boundary start regex: {}", e))?,
                )
            }
            _ => None,
        };

        Ok(Self {
            config,
            buffer: Vec::new(),
            regex,
        })
    }

    /// Check if this line starts a new event based on the strategy
    fn starts_new_event(&self, line: &str) -> bool {
        match &self.config.strategy {
            MultilineStrategy::Timestamp { .. } => {
                if let Some(ref regex) = self.regex {
                    regex.is_match(line)
                } else {
                    false
                }
            }
            MultilineStrategy::Indent {
                spaces,
                tabs,
                mixed,
            } => {
                // A new event starts when the line is NOT indented
                !self.is_indented(line, *spaces, *tabs, *mixed)
            }
            MultilineStrategy::Start { .. } => {
                if let Some(ref regex) = self.regex {
                    regex.is_match(line)
                } else {
                    false
                }
            }
            MultilineStrategy::End { .. } => {
                // For end strategy, we look at the previous line in the buffer
                false // New events don't start based on current line
            }
            MultilineStrategy::Boundary { .. } => {
                if let Some(ref regex) = self.regex {
                    regex.is_match(line)
                } else {
                    false
                }
            }
            MultilineStrategy::Backslash { .. } => {
                // New events start when previous line doesn't end with continuation char
                false // Logic handled elsewhere
            }
        }
    }

    /// Check if this line ends the current event based on the strategy
    fn ends_current_event(&self, line: &str) -> bool {
        match &self.config.strategy {
            MultilineStrategy::End { pattern: _ } => {
                if let Some(ref regex) = self.regex {
                    regex.is_match(line)
                } else {
                    false
                }
            }
            MultilineStrategy::Boundary { end, .. } => {
                if let Ok(end_regex) = Regex::new(end) {
                    end_regex.is_match(line)
                } else {
                    false
                }
            }
            MultilineStrategy::Backslash { char } => {
                // Event continues if line ends with continuation character
                !line.ends_with(*char)
            }
            _ => false,
        }
    }

    /// Check if a line is indented according to the indent strategy
    fn is_indented(&self, line: &str, spaces: Option<u32>, tabs: bool, mixed: bool) -> bool {
        if line.is_empty() {
            return false; // Empty lines are not considered indented
        }

        if mixed {
            // Any whitespace counts as indentation
            line.starts_with(' ') || line.starts_with('\t')
        } else if tabs {
            // Only tabs count
            line.starts_with('\t')
        } else if let Some(min_spaces) = spaces {
            // Specific number of spaces required
            let leading_spaces = line.chars().take_while(|&c| c == ' ').count() as u32;
            leading_spaces >= min_spaces
        } else {
            // Default: any space-based indentation
            line.starts_with(' ')
        }
    }

    /// Flush the current buffer and return the event
    fn flush_buffer(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            None
        } else {
            let result = Some(self.buffer.join("\n"));
            self.buffer.clear();
            result
        }
    }
}

impl Chunker for MultilineChunker {
    fn feed_line(&mut self, line: String) -> Option<String> {
        // Check boundary conditions based on strategy
        let should_flush = match &self.config.strategy {
            MultilineStrategy::Backslash { .. } => {
                // For backslash strategy, check if previous line ended the event
                if let Some(last_line) = self.buffer.last() {
                    self.ends_current_event(last_line)
                } else {
                    false
                }
            }
            MultilineStrategy::End { .. } | MultilineStrategy::Boundary { .. } => {
                // For end/boundary strategies, check if current line ends the event
                self.ends_current_event(&line)
            }
            _ => {
                // For timestamp, indent, and start strategies, check if current line starts a new event
                !self.buffer.is_empty() && self.starts_new_event(&line)
            }
        };

        let result = if should_flush {
            self.flush_buffer()
        } else {
            None
        };

        // Add the new line to buffer
        self.buffer.push(line);

        result
    }

    fn flush(&mut self) -> Option<String> {
        self.flush_buffer()
    }
}

/// Create a chunker based on multiline configuration
pub fn create_multiline_chunker(config: &MultilineConfig) -> Result<Box<dyn Chunker>, String> {
    let chunker = MultilineChunker::new(config.clone())?;
    Ok(Box::new(chunker))
}
