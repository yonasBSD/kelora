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
            MultilineStrategy::Whole => None,
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
            MultilineStrategy::Whole => {
                // Whole strategy never starts new events during feed - everything gets buffered
                false
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
                // Event continues if line ends with continuation character (ignoring trailing newlines)
                let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
                !trimmed.ends_with(*char)
            }
            MultilineStrategy::Whole => {
                // Whole strategy never ends current event during feed - everything gets buffered
                false
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
            // For "whole" strategy, preserve line structure with newlines
            // For other strategies, lines already contain newlines, so join with empty string to avoid double newlines
            let result = match &self.config.strategy {
                MultilineStrategy::Whole => {
                    // Join with newlines to preserve line structure for whole file reading
                    Some(self.buffer.join("\n"))
                }
                _ => {
                    // Lines already contain newlines for other strategies
                    Some(self.buffer.join(""))
                }
            };
            self.buffer.clear();
            result
        }
    }
}

impl Chunker for MultilineChunker {
    fn feed_line(&mut self, line: String) -> Option<String> {
        // Whole strategy always buffers everything and never returns content during feed
        if let MultilineStrategy::Whole = &self.config.strategy {
            self.buffer.push(line);
            return None;
        }

        // Backslash strategy has different logic - we need to add the line first,
        // then check if the event should end
        if let MultilineStrategy::Backslash { .. } = &self.config.strategy {
            // Add the line to buffer first
            self.buffer.push(line);

            // Check if this line (the one we just added) ends the event
            if let Some(last_line) = self.buffer.last() {
                if self.ends_current_event(last_line) {
                    // Event is complete, flush the buffer
                    return self.flush_buffer();
                }
            }

            // Event continues, return None
            return None;
        }

        // For all other strategies, use the original logic
        let should_flush = match &self.config.strategy {
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

    fn has_pending(&self) -> bool {
        !self.buffer.is_empty()
    }
}

/// Create a chunker based on multiline configuration
pub fn create_multiline_chunker(config: &MultilineConfig) -> Result<Box<dyn Chunker>, String> {
    let chunker = MultilineChunker::new(config.clone())?;
    Ok(Box::new(chunker))
}
