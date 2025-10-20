use crate::config::SectionConfig;

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
        let initial_state = if config.start_pattern.is_none() {
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
                if let Some(ref start_pattern) = self.config.start_pattern {
                    if start_pattern.is_match(line) {
                        // Found section start
                        self.state = SectionState::InSection;
                        self.sections_seen += 1;
                        true // Include the start line
                    } else {
                        false // Still waiting for section to start
                    }
                } else {
                    // No start pattern, so we're always in a section
                    true
                }
            }
            SectionState::InSection => {
                // Currently in a section, check for end
                if let Some(ref end_pattern) = self.config.end_pattern {
                    if end_pattern.is_match(line) {
                        // Found section end

                        // Check if we've hit the section limit
                        if self.config.max_sections > 0
                            && self.sections_seen >= self.config.max_sections
                        {
                            self.state = SectionState::Done;
                        } else {
                            // More sections to process
                            self.state = if self.config.start_pattern.is_some() {
                                SectionState::BetweenSections
                            } else {
                                // No start pattern means we're always in a section
                                SectionState::InSection
                            };
                        }

                        false // Don't include the end line (exclusive)
                    } else {
                        true // Still in section, include line
                    }
                } else {
                    // No end pattern, check for next section start
                    if let Some(ref start_pattern) = self.config.start_pattern {
                        if start_pattern.is_match(line) {
                            // Found start of next section
                            self.sections_seen += 1;

                            // Check if we've hit the limit
                            if self.config.max_sections > 0
                                && self.sections_seen > self.config.max_sections
                            {
                                self.state = SectionState::Done;
                                false // Over the limit, skip this line
                            } else {
                                true // Start of new section, include line
                            }
                        } else {
                            true // Still in section, include line
                        }
                    } else {
                        // No end pattern and no start pattern, always include
                        true
                    }
                }
            }
            SectionState::BetweenSections => {
                // Between sections, looking for next section start
                if let Some(ref start_pattern) = self.config.start_pattern {
                    if start_pattern.is_match(line) {
                        // Found start of next section
                        self.sections_seen += 1;

                        // Check if we've hit the limit
                        if self.config.max_sections > 0
                            && self.sections_seen > self.config.max_sections
                        {
                            self.state = SectionState::Done;
                            false // Over the limit, skip this line
                        } else {
                            self.state = SectionState::InSection;
                            true // Start of new section, include line
                        }
                    } else {
                        false // Still between sections, skip line
                    }
                } else {
                    // No start pattern but we're between sections - shouldn't happen
                    // but handle it by staying in section
                    true
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    fn create_config(start: Option<&str>, end: Option<&str>, max_sections: i64) -> SectionConfig {
        SectionConfig {
            start_pattern: start.map(|s| Regex::new(s).unwrap()),
            end_pattern: end.map(|s| Regex::new(s).unwrap()),
            max_sections,
        }
    }

    #[test]
    fn test_single_section_with_start_and_end() {
        let config = create_config(Some(r"^START"), Some(r"^END"), -1);
        let mut selector = SectionSelector::new(config);

        assert!(!selector.should_include_line("before"));
        assert!(selector.should_include_line("START line"));
        assert!(selector.should_include_line("content1"));
        assert!(selector.should_include_line("content2"));
        assert!(!selector.should_include_line("END line")); // end is exclusive
        assert!(!selector.should_include_line("after"));
    }

    #[test]
    fn test_multiple_sections() {
        let config = create_config(Some(r"^START"), Some(r"^END"), -1);
        let mut selector = SectionSelector::new(config);

        // First section
        assert!(selector.should_include_line("START 1"));
        assert!(selector.should_include_line("content 1"));
        assert!(!selector.should_include_line("END 1"));

        // Between sections
        assert!(!selector.should_include_line("between"));

        // Second section
        assert!(selector.should_include_line("START 2"));
        assert!(selector.should_include_line("content 2"));
        assert!(!selector.should_include_line("END 2"));
    }

    #[test]
    fn test_max_sections_limit() {
        let config = create_config(Some(r"^START"), Some(r"^END"), 2);
        let mut selector = SectionSelector::new(config);

        // First section
        assert!(selector.should_include_line("START 1"));
        assert!(selector.should_include_line("content 1"));
        assert!(!selector.should_include_line("END 1"));

        // Second section
        assert!(selector.should_include_line("START 2"));
        assert!(selector.should_include_line("content 2"));
        assert!(!selector.should_include_line("END 2"));

        // Third section (should be skipped)
        assert!(!selector.should_include_line("START 3"));
        assert!(!selector.should_include_line("content 3"));
        assert!(!selector.should_include_line("END 3"));
    }

    #[test]
    fn test_start_only_no_end() {
        let config = create_config(Some(r"^== \w+ Logs"), None, -1);
        let mut selector = SectionSelector::new(config);

        assert!(!selector.should_include_line("header"));
        assert!(selector.should_include_line("== iked Logs"));
        assert!(selector.should_include_line("log line 1"));
        assert!(selector.should_include_line("log line 2"));

        // New section starts, previous ends
        assert!(selector.should_include_line("== UI Logs"));
        assert!(selector.should_include_line("ui log 1"));
    }

    #[test]
    fn test_start_only_with_limit() {
        let config = create_config(Some(r"^=="), None, 1);
        let mut selector = SectionSelector::new(config);

        assert!(selector.should_include_line("== Section 1"));
        assert!(selector.should_include_line("content 1"));

        // Second section should be skipped
        assert!(!selector.should_include_line("== Section 2"));
        assert!(!selector.should_include_line("content 2"));
    }

    #[test]
    fn test_no_patterns_processes_everything() {
        let config = create_config(None, None, -1);
        let mut selector = SectionSelector::new(config);

        // With no patterns, everything is included
        assert!(selector.should_include_line("line 1"));
        assert!(selector.should_include_line("line 2"));
        assert!(selector.should_include_line("line 3"));
    }

    #[test]
    fn test_end_only() {
        let config = create_config(None, Some(r"^END"), -1);
        let mut selector = SectionSelector::new(config);

        // Starts immediately (no start pattern)
        assert!(selector.should_include_line("content 1"));
        assert!(selector.should_include_line("content 2"));
        assert!(!selector.should_include_line("END")); // Exclusive

        // After end, continues processing (no start pattern to resume)
        assert!(selector.should_include_line("content 3"));
    }
}
