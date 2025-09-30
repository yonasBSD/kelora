use super::Chunker;
use crate::config::{InputFormat, MultilineConfig, MultilineStrategy};
use chrono::format::{parse_and_remainder, Parsed, StrftimeItems};
use regex::Regex;

/// Multi-line chunker that implements various strategies for detecting event boundaries
pub struct MultilineChunker {
    config: MultilineConfig,
    buffer: Vec<String>,
    regex: Option<Regex>,
    input_format: InputFormat,
}

impl MultilineChunker {
    pub fn new(config: MultilineConfig, input_format: InputFormat) -> Result<Self, String> {
        let regex = match &config.strategy {
            MultilineStrategy::Timestamp { pattern, .. } => {
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
            input_format,
        })
    }

    /// Check if this line starts a new event based on the strategy
    fn starts_new_event(&self, line: &str) -> bool {
        match &self.config.strategy {
            MultilineStrategy::Timestamp { chrono_format, .. } => {
                if let Some(format) = chrono_format {
                    if self.matches_chrono_timestamp(line, format) {
                        return true;
                    }
                }
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

    /// Clean backslash continuation sequences from a string
    fn clean_backslash_continuations(&self, input: &str, continuation_char: char) -> String {
        let mut result = String::with_capacity(input.len());
        let lines: Vec<&str> = input.split('\n').collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim_end_matches('\r');

            if trimmed.ends_with(continuation_char) {
                // Remove the continuation character and join directly with the next line
                let without_continuation = &trimmed[..trimmed.len() - continuation_char.len_utf8()];
                result.push_str(without_continuation);
                // Don't add space - continuation means direct concatenation
            } else {
                result.push_str(trimmed);
            }

            // Only add space between lines if current line doesn't end with continuation
            // and this isn't the last line
            if !trimmed.ends_with(continuation_char) && i < lines.len() - 1 {
                result.push(' ');
            }
        }

        result
    }

    /// Flush the current buffer and return the event
    fn flush_buffer(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            None
        } else {
            let joined = match &self.config.strategy {
                MultilineStrategy::Whole => {
                    // Join with newlines to preserve line structure for whole file reading
                    self.buffer.join("\n")
                }
                _ => {
                    // Lines already contain newlines for other strategies
                    self.buffer.join("")
                }
            };

            // Apply format-aware line cleaning
            let result = match &self.config.strategy {
                MultilineStrategy::Backslash { char } => {
                    // Always clean backslash continuations regardless of format
                    self.clean_backslash_continuations(&joined, *char)
                }
                _ => {
                    // For other strategies, clean based on input format
                    match self.input_format {
                        InputFormat::Raw => {
                            // Preserve newlines for raw format - this is the new use case
                            joined
                        }
                        _ => {
                            // Replace newlines with spaces for all other formats (including line)
                            joined.replace('\n', " ").replace('\r', "")
                        }
                    }
                }
            };

            self.buffer.clear();
            Some(result)
        }
    }
}

impl MultilineChunker {
    /// Check whether the line starts with a timestamp matching the provided chrono format.
    fn matches_chrono_timestamp(&self, line: &str, format: &str) -> bool {
        let mut parsed = Parsed::new();

        parse_and_remainder(&mut parsed, line, StrftimeItems::new(format)).is_ok()
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

        match &self.config.strategy {
            MultilineStrategy::End { .. } | MultilineStrategy::Boundary { .. } => {
                // For end/boundary strategies we must include the terminating line in the event
                self.buffer.push(line);

                if let Some(last_line) = self.buffer.last() {
                    if self.ends_current_event(last_line) {
                        return self.flush_buffer();
                    }
                }

                None
            }
            _ => {
                // For timestamp, indent, and start strategies, check if current line starts a new event
                let should_flush = !self.buffer.is_empty() && self.starts_new_event(&line);
                let result = if should_flush {
                    self.flush_buffer()
                } else {
                    None
                };

                // Add the new line to buffer
                self.buffer.push(line);

                result
            }
        }
    }

    fn flush(&mut self) -> Option<String> {
        self.flush_buffer()
    }

    fn has_pending(&self) -> bool {
        !self.buffer.is_empty()
    }
}

/// Create a chunker based on multiline configuration
pub fn create_multiline_chunker(
    config: &MultilineConfig,
    input_format: InputFormat,
) -> Result<Box<dyn Chunker>, String> {
    let chunker = MultilineChunker::new(config.clone(), input_format)?;
    Ok(Box::new(chunker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_strategy_prefers_ts_format_hint() {
        let config = MultilineConfig {
            strategy: MultilineStrategy::Timestamp {
                pattern: r"^\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}".to_string(),
                chrono_format: Some("%b %e %H:%M:%S".to_string()),
            },
        };

        let mut chunker =
            MultilineChunker::new(config, InputFormat::Syslog).expect("chunker should build");

        // First event starts with the chrono format, continuation line is indented
        assert!(chunker
            .feed_line("Jan  2 03:04:05 host app: one\n".to_string())
            .is_none());
        assert!(chunker
            .feed_line("  stack frame line\n".to_string())
            .is_none());

        // New timestamp should flush the buffered event
        let flushed = chunker.feed_line("Jan  3 03:04:05 host app: two\n".to_string());
        assert!(flushed.is_some());

        // Flush remaining buffered lines for the second event
        assert!(chunker.flush().is_some());
    }
}
