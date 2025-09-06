use crate::event::Event;
use rhai::Dynamic;

/// Extracts prefix from lines and adds it to parsed events
#[derive(Debug, Clone)]
pub struct PrefixExtractor {
    pub field_name: String,
    pub separator: String,
}

impl PrefixExtractor {
    pub fn new(field_name: String, separator: String) -> Self {
        Self {
            field_name,
            separator,
        }
    }

    /// Extract prefix from line if separator is found, return (modified_line, extracted_prefix)
    pub fn extract_prefix(&self, line: &str) -> (String, Option<String>) {
        if let Some(sep_pos) = line.find(&self.separator) {
            let prefix = line[..sep_pos].trim();
            let remaining = line[sep_pos + self.separator.len()..].trim();

            if !prefix.is_empty() {
                (remaining.to_string(), Some(prefix.to_string()))
            } else {
                // Empty prefix, return the remaining part but don't extract prefix
                (remaining.to_string(), None)
            }
        } else {
            // No separator found, return original line
            (line.to_string(), None)
        }
    }

    /// Add extracted prefix to an event
    pub fn add_prefix_to_event(&self, event: &mut Event, prefix: Option<String>) {
        if let Some(prefix_value) = prefix {
            event.set_field(self.field_name.clone(), Dynamic::from(prefix_value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_prefix_with_pipe() {
        let extractor = PrefixExtractor::new("src".to_string(), "|".to_string());

        // Test with Docker-style compose logs
        let (line, prefix) =
            extractor.extract_prefix("web_1    | 2024-07-27T12:34:56Z GET /health 200");
        assert_eq!(line, "2024-07-27T12:34:56Z GET /health 200");
        assert_eq!(prefix, Some("web_1".to_string()));

        // Test without separator
        let (line, prefix) = extractor.extract_prefix("Just a normal log line");
        assert_eq!(line, "Just a normal log line");
        assert_eq!(prefix, None);

        // Test with empty prefix
        let (line, prefix) = extractor.extract_prefix(" | Just the message");
        assert_eq!(line, "Just the message");
        assert_eq!(prefix, None);
    }

    #[test]
    fn test_extract_prefix_with_custom_separator() {
        let extractor = PrefixExtractor::new("service".to_string(), " :: ".to_string());

        let (line, prefix) = extractor.extract_prefix("auth-service :: User login successful");
        assert_eq!(line, "User login successful");
        assert_eq!(prefix, Some("auth-service".to_string()));
    }

    #[test]
    fn test_add_prefix_to_event() {
        let extractor = PrefixExtractor::new("src".to_string(), "|".to_string());
        let mut event = Event::default_with_line("test line".to_string());

        extractor.add_prefix_to_event(&mut event, Some("web_1".to_string()));

        assert_eq!(
            event
                .fields
                .get("src")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "web_1"
        );
    }

    #[test]
    fn test_add_none_prefix_to_event() {
        let extractor = PrefixExtractor::new("src".to_string(), "|".to_string());
        let mut event = Event::default_with_line("test line".to_string());

        extractor.add_prefix_to_event(&mut event, None);

        assert!(event.fields.get("src").is_none());
    }
}
