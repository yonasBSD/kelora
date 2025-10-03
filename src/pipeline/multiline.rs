use super::Chunker;
use crate::config::{InputFormat, MultilineConfig, MultilineStrategy};
use crate::timestamp::AdaptiveTsParser;
use regex::Regex;

const MAX_TIMESTAMP_PREFIX_CHARS: usize = 64;
const MAX_TIMESTAMP_TOKENS: usize = 6;

/// Multi-line chunker that implements the reduced set of strategies for detecting event boundaries
pub struct MultilineChunker {
    config: MultilineConfig,
    buffer: Vec<String>,
    start_regex: Option<Regex>,
    end_regex: Option<Regex>,
    input_format: InputFormat,
    timestamp_detector: Option<TimestampDetector>,
    pending_output: Option<String>,
}

impl MultilineChunker {
    pub fn new(config: MultilineConfig, input_format: InputFormat) -> Result<Self, String> {
        let mut start_regex = None;
        let mut end_regex = None;
        let mut timestamp_detector = None;

        match &config.strategy {
            MultilineStrategy::Regex { start, end } => {
                start_regex = Some(
                    Regex::new(start).map_err(|e| format!("Invalid regex start pattern: {}", e))?,
                );

                if let Some(end_pattern) = end {
                    end_regex = Some(
                        Regex::new(end_pattern)
                            .map_err(|e| format!("Invalid regex end pattern: {}", e))?,
                    );
                }
            }
            MultilineStrategy::Timestamp { chrono_format } => {
                timestamp_detector = Some(TimestampDetector::new(chrono_format.clone()));
            }
            MultilineStrategy::Indent | MultilineStrategy::All => {}
        }

        Ok(Self {
            config,
            buffer: Vec::new(),
            start_regex,
            end_regex,
            input_format,
            timestamp_detector,
            pending_output: None,
        })
    }

    /// Check if this line starts a new event based on the current strategy
    fn starts_new_event(&mut self, line: &str) -> bool {
        match &self.config.strategy {
            MultilineStrategy::Timestamp { .. } => {
                if let Some(detector) = self.timestamp_detector.as_mut() {
                    detector.is_header(line)
                } else {
                    false
                }
            }
            MultilineStrategy::Indent => !is_line_indented(line),
            MultilineStrategy::Regex { .. } => {
                if let Some(regex) = &self.start_regex {
                    regex.is_match(line)
                } else {
                    false
                }
            }
            MultilineStrategy::All => false,
        }
    }

    /// Check if this line ends the current event (only relevant for regex strategies with end=...)
    fn ends_current_event(&self, line: &str) -> bool {
        match (&self.config.strategy, &self.end_regex) {
            (MultilineStrategy::Regex { end: Some(_), .. }, Some(regex)) => regex.is_match(line),
            _ => false,
        }
    }

    /// Flush the current buffer and return the event content
    fn flush_buffer(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            return None;
        }

        let joined = match self.config.strategy {
            MultilineStrategy::All => self.buffer.join("\n"),
            _ => self.buffer.join(""),
        };

        let content = match self.config.strategy {
            MultilineStrategy::All => joined,
            _ => match self.input_format {
                InputFormat::Raw => joined,
                _ => joined.replace('\n', " ").replace('\r', ""),
            },
        };

        self.buffer.clear();
        Some(content)
    }
}

struct TimestampDetector {
    parser: AdaptiveTsParser,
    chrono_format: Option<String>,
}

impl TimestampDetector {
    fn new(chrono_format: Option<String>) -> Self {
        Self {
            parser: AdaptiveTsParser::new(),
            chrono_format,
        }
    }

    fn is_header(&mut self, line: &str) -> bool {
        let stripped = line.trim_end_matches(['\n', '\r']);

        if stripped.is_empty() {
            return false;
        }

        if stripped.starts_with(char::is_whitespace) {
            return false;
        }

        let candidates = timestamp_prefix_candidates(stripped);
        if candidates.is_empty() {
            return false;
        }

        let custom_format = self.chrono_format.clone();
        if let Some(format) = custom_format.as_deref() {
            if self.try_candidates(&candidates, Some(format)) {
                return true;
            }
        }

        self.try_candidates(&candidates, None)
    }

    fn try_candidates(&mut self, candidates: &[String], custom_format: Option<&str>) -> bool {
        for candidate in candidates {
            if self
                .parser
                .parse_ts_with_config(candidate, custom_format, None)
                .is_some()
            {
                return true;
            }
        }

        false
    }
}

fn is_line_indented(line: &str) -> bool {
    let stripped = line.trim_end_matches(['\n', '\r']);
    if stripped.is_empty() {
        return false;
    }

    stripped.starts_with(char::is_whitespace)
}

fn timestamp_prefix_candidates(line: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    if line.is_empty() {
        return candidates;
    }

    let mut tokens = Vec::new();
    let mut token_start: Option<usize> = None;
    let mut reached_limit = false;

    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some(start) = token_start.take() {
                tokens.push((start, idx));
                if tokens.len() == MAX_TIMESTAMP_TOKENS {
                    reached_limit = true;
                    break;
                }
            }
        } else if token_start.is_none() {
            token_start = Some(idx);
        }
    }

    if !reached_limit {
        if let Some(start) = token_start {
            tokens.push((start, line.len()));
        }
    }

    if tokens.is_empty() {
        let fallback = take_prefix_chars(line, MAX_TIMESTAMP_PREFIX_CHARS);
        push_candidate(&mut candidates, fallback.to_string());
        return candidates;
    }

    let max_tokens = tokens.len().min(MAX_TIMESTAMP_TOKENS);
    let start_idx = tokens[0].0;

    for count in 1..=max_tokens {
        let (_, end_idx) = tokens[count - 1];
        let slice = &line[start_idx..end_idx.min(line.len())];

        if slice.chars().count() > MAX_TIMESTAMP_PREFIX_CHARS {
            continue;
        }

        push_candidate(&mut candidates, slice.to_string());

        let trimmed_slice = slice.trim_end_matches([':', ',', ';', '-', '.']);
        if trimmed_slice.len() < slice.len() && trimmed_slice.chars().count() >= 4 {
            push_candidate(&mut candidates, trimmed_slice.to_string());
        }
    }

    let fallback = take_prefix_chars(line, MAX_TIMESTAMP_PREFIX_CHARS);
    push_candidate(&mut candidates, fallback.to_string());

    candidates
}

fn take_prefix_chars(s: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }

    for (count, (idx, _)) in s.char_indices().enumerate() {
        if count == max_chars {
            return &s[..idx];
        }
    }

    s
}

fn push_candidate(candidates: &mut Vec<String>, candidate: String) {
    if candidate.is_empty() {
        return;
    }

    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

impl Chunker for MultilineChunker {
    fn feed_line(&mut self, line: String) -> Option<String> {
        let mut produced = Vec::new();

        if let Some(pending) = self.pending_output.take() {
            produced.push(pending);
        }

        let flush_on_start = matches!(
            self.config.strategy,
            MultilineStrategy::Timestamp { .. }
                | MultilineStrategy::Indent
                | MultilineStrategy::Regex { .. }
        );

        if flush_on_start && !self.buffer.is_empty() && self.starts_new_event(&line) {
            if let Some(event) = self.flush_buffer() {
                produced.push(event);
            }
        }

        match &self.config.strategy {
            MultilineStrategy::All => {
                self.buffer.push(line);
            }
            MultilineStrategy::Regex { .. } => {
                self.buffer.push(line);

                if let Some(last_line) = self.buffer.last() {
                    if self.ends_current_event(last_line) {
                        if let Some(event) = self.flush_buffer() {
                            produced.push(event);
                        }
                    }
                }
            }
            MultilineStrategy::Timestamp { .. } | MultilineStrategy::Indent => {
                self.buffer.push(line);
            }
        }

        if produced.is_empty() {
            None
        } else {
            let first = produced.remove(0);
            if let Some(next) = produced.into_iter().next() {
                self.pending_output = Some(next);
            }
            Some(first)
        }
    }

    fn flush(&mut self) -> Option<String> {
        if let Some(pending) = self.pending_output.take() {
            return Some(pending);
        }

        self.flush_buffer()
    }

    fn has_pending(&self) -> bool {
        self.pending_output.is_some() || !self.buffer.is_empty()
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
    fn timestamp_detection_with_format_hint() {
        let config = MultilineConfig {
            strategy: MultilineStrategy::Timestamp {
                chrono_format: Some("%b %e %H:%M:%S".to_string()),
            },
        };

        let mut chunker =
            MultilineChunker::new(config, InputFormat::Syslog).expect("chunker should build");

        assert!(chunker
            .feed_line("Jan  2 03:04:05 host app: one\n".to_string())
            .is_none());
        assert!(chunker
            .feed_line("  stack frame line\n".to_string())
            .is_none());

        let flushed = chunker.feed_line("Jan  3 03:04:05 host app: two\n".to_string());
        assert!(flushed.is_some());

        assert!(chunker.flush().is_some());
    }
}
