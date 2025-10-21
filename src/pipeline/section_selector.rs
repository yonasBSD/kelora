use crate::config::{SectionConfig, SectionEnd, SectionStart};

/// State machine for section selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionState {
    /// Haven't found the first section start yet
    NotStarted,
    /// Currently inside a section, emitting lines
    InSection,
    /// Between sections, skipping lines
    BetweenSections,
    /// Reached max sections limit, done processing
    Done,
}

/// Section selector that filters lines based on start/end patterns
pub struct SectionSelector {
    config: SectionConfig,
    state: SectionState,
    sections_seen: i64,
}

impl SectionSelector {
    /// Create a new section selector
    pub fn new(config: SectionConfig) -> Self {
        // If no start pattern is provided, start immediately
        let initial_state = if config.start.is_none() {
            SectionState::InSection
        } else {
            SectionState::NotStarted
        };

        Self {
            config,
            state: initial_state,
            sections_seen: 0,
        }
    }

    /// Check if a line should be included based on section boundaries
    /// Returns true if the line should be processed, false if it should be skipped
    pub fn should_include_line(&mut self, line: &str) -> bool {
        match self.state {
            SectionState::Done => {
                // Already reached max sections, skip everything
                false
            }
            SectionState::NotStarted => {
                // Looking for the first section start
                if let Some(include_line) = self.matches_start(line) {
                    self.sections_seen += 1;
                    self.state = SectionState::InSection;
                    include_line
                } else {
                    // Still waiting for section to start
                    false
                }
            }
            SectionState::InSection => {
                // Currently in a section, check for end
                if let Some(include_line) = self.matches_end(line) {
                    if self.limit_reached() {
                        self.state = SectionState::Done;
                    } else if self.config.start.is_some() {
                        self.state = SectionState::BetweenSections;
                    } else {
                        // No start pattern means we keep streaming within the same section
                        self.state = SectionState::InSection;
                    }
                    include_line
                } else if let Some(include_line) = self.matches_start(line) {
                    // Found start of next section (only happens when no explicit end)
                    self.sections_seen += 1;
                    if self.limit_exceeded() {
                        self.state = SectionState::Done;
                        false
                    } else {
                        include_line
                    }
                } else {
                    true // Still in section, include line
                }
            }
            SectionState::BetweenSections => {
                if let Some(include_line) = self.matches_start(line) {
                    self.sections_seen += 1;
                    if self.limit_exceeded() {
                        self.state = SectionState::Done;
                        false
                    } else {
                        self.state = SectionState::InSection;
                        include_line
                    }
                } else {
                    // Between sections, skip until we find the next start
                    false
                }
            }
        }
    }

    fn matches_start(&self, line: &str) -> Option<bool> {
        match &self.config.start {
            Some(SectionStart::From(pattern)) => {
                if pattern.is_match(line) {
                    Some(true)
                } else {
                    None
                }
            }
            Some(SectionStart::After(pattern)) => {
                if pattern.is_match(line) {
                    Some(false)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn matches_end(&self, line: &str) -> Option<bool> {
        match &self.config.end {
            Some(SectionEnd::Before(pattern)) => {
                if pattern.is_match(line) {
                    Some(false)
                } else {
                    None
                }
            }
            Some(SectionEnd::Through(pattern)) => {
                if pattern.is_match(line) {
                    Some(true)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn limit_reached(&self) -> bool {
        self.config.max_sections > 0 && self.sections_seen >= self.config.max_sections
    }

    fn limit_exceeded(&self) -> bool {
        self.config.max_sections > 0 && self.sections_seen > self.config.max_sections
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    fn start_from(pattern: &str) -> SectionStart {
        SectionStart::From(Regex::new(pattern).unwrap())
    }

    fn start_after(pattern: &str) -> SectionStart {
        SectionStart::After(Regex::new(pattern).unwrap())
    }

    fn end_before(pattern: &str) -> SectionEnd {
        SectionEnd::Before(Regex::new(pattern).unwrap())
    }

    fn end_through(pattern: &str) -> SectionEnd {
        SectionEnd::Through(Regex::new(pattern).unwrap())
    }

    fn build_config(
        start: Option<SectionStart>,
        end: Option<SectionEnd>,
        max_sections: i64,
    ) -> SectionConfig {
        SectionConfig {
            start,
            end,
            max_sections,
        }
    }

    #[test]
    fn start_from_end_before_matches_legacy_behavior() {
        let config = build_config(Some(start_from(r"^START")), Some(end_before(r"^END")), -1);
        let mut selector = SectionSelector::new(config);

        assert!(!selector.should_include_line("before"));
        assert!(selector.should_include_line("START line"));
        assert!(selector.should_include_line("content1"));
        assert!(selector.should_include_line("content2"));
        assert!(!selector.should_include_line("END line"));
        assert!(!selector.should_include_line("after"));
    }

    #[test]
    fn start_after_skips_marker_line() {
        let config = build_config(
            Some(start_after(r"^== HEADER")),
            Some(end_before(r"^== NEXT")),
            -1,
        );
        let mut selector = SectionSelector::new(config);

        assert!(!selector.should_include_line("preamble"));
        assert!(!selector.should_include_line("== HEADER"));
        assert!(selector.should_include_line("body line"));
        assert!(!selector.should_include_line("== NEXT"));
    }

    #[test]
    fn end_through_includes_terminator() {
        let config = build_config(Some(start_from(r"^BEGIN")), Some(end_through(r"^END$")), -1);
        let mut selector = SectionSelector::new(config);

        assert!(selector.should_include_line("BEGIN"));
        assert!(selector.should_include_line("payload"));
        assert!(selector.should_include_line("END"));
        assert!(!selector.should_include_line("after"));
    }

    #[test]
    fn multiple_sections_respect_limits() {
        let config = build_config(Some(start_from(r"^START")), Some(end_before(r"^END")), 2);
        let mut selector = SectionSelector::new(config);

        assert!(selector.should_include_line("START 1"));
        assert!(selector.should_include_line("line 1"));
        assert!(!selector.should_include_line("END 1"));

        assert!(!selector.should_include_line("noise"));

        assert!(selector.should_include_line("START 2"));
        assert!(selector.should_include_line("line 2"));
        assert!(!selector.should_include_line("END 2"));

        assert!(!selector.should_include_line("START 3"));
        assert!(!selector.should_include_line("line 3"));
        assert!(!selector.should_include_line("END 3"));
    }

    #[test]
    fn start_only_with_inclusive_marker() {
        let config = build_config(Some(start_from(r"^== ")), None, -1);
        let mut selector = SectionSelector::new(config);

        assert!(!selector.should_include_line("header"));
        assert!(selector.should_include_line("== A"));
        assert!(selector.should_include_line("a1"));
        assert!(selector.should_include_line("== B"));
        assert!(selector.should_include_line("b1"));
    }

    #[test]
    fn start_after_respects_limits_without_end() {
        let config = build_config(Some(start_after(r"^== ")), None, 1);
        let mut selector = SectionSelector::new(config);

        assert!(!selector.should_include_line("== A"));
        assert!(selector.should_include_line("a1"));
        assert!(selector.should_include_line("a2"));
        assert!(!selector.should_include_line("== B"));
        assert!(!selector.should_include_line("b1"));
    }

    #[test]
    fn no_patterns_include_everything() {
        let config = build_config(None, None, -1);
        let mut selector = SectionSelector::new(config);

        assert!(selector.should_include_line("line 1"));
        assert!(selector.should_include_line("line 2"));
        assert!(selector.should_include_line("line 3"));
    }
}
