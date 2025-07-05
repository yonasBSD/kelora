use crate::event::Event;
use crate::pipeline::EventParser;
use anyhow::{Context, Result};
use csv::ReaderBuilder;
use rhai::Dynamic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// CSV/TSV Parser that supports multiple dialects
///
/// Supported formats:
/// - CSV: Comma-separated values with headers
/// - TSV: Tab-separated values with headers  
/// - CSVNH: Comma-separated values without headers (generates col1, col2, col3...)
/// - TSVNH: Tab-separated values without headers (generates col1, col2, col3...)
pub struct CsvParser {
    delimiter: u8,
    has_headers: bool,
    headers: Arc<Mutex<Vec<String>>>,
    initialized: Arc<AtomicBool>,
}

impl CsvParser {
    /// Create a new CSV parser (comma-separated with headers)
    pub fn new_csv() -> Self {
        Self {
            delimiter: b',',
            has_headers: true,
            headers: Arc::new(Mutex::new(Vec::new())),
            initialized: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new TSV parser (tab-separated with headers)
    pub fn new_tsv() -> Self {
        Self {
            delimiter: b'\t',
            has_headers: true,
            headers: Arc::new(Mutex::new(Vec::new())),
            initialized: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new CSV parser without headers (comma-separated, generates col1, col2, col3...)
    pub fn new_csv_no_headers() -> Self {
        Self {
            delimiter: b',',
            has_headers: false,
            headers: Arc::new(Mutex::new(Vec::new())),
            initialized: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new TSV parser without headers (tab-separated, generates col1, col2, col3...)
    pub fn new_tsv_no_headers() -> Self {
        Self {
            delimiter: b'\t',
            has_headers: false,
            headers: Arc::new(Mutex::new(Vec::new())),
            initialized: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Parse the header line and extract column names
    fn parse_header_line(&self, line: &str) -> Result<Vec<String>> {
        let mut reader = ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(line.as_bytes());

        if let Some(result) = reader.records().next() {
            let record = result.context("Failed to parse CSV header line")?;
            let headers: Vec<String> = record.iter().map(|s| s.trim().to_string()).collect();
            Ok(headers)
        } else {
            Err(anyhow::anyhow!("Empty CSV header line"))
        }
    }

    /// Generate column names for headerless CSV (col1, col2, col3...)
    fn generate_column_names(&self, line: &str) -> Result<Vec<String>> {
        let mut reader = ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(line.as_bytes());

        if let Some(result) = reader.records().next() {
            let record = result.context("Failed to parse CSV data line for column count")?;
            let headers: Vec<String> = (1..=record.len()).map(|i| format!("col{}", i)).collect();
            Ok(headers)
        } else {
            Err(anyhow::anyhow!("Empty CSV data line"))
        }
    }

    /// Parse a data line using the stored headers
    fn parse_data_line(&self, line: &str) -> Result<Event> {
        let mut reader = ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(line.as_bytes());

        if let Some(result) = reader.records().next() {
            let record = result.context("Failed to parse CSV data line")?;
            let mut event = Event::with_capacity(line.to_string(), record.len());

            // Map CSV fields to event using stored headers
            let headers = self.headers.lock().unwrap();
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = headers.get(i) {
                    // Store field values as strings (consistent with other parsers)
                    event.set_field(header.clone(), Dynamic::from(field.to_string()));
                }
            }

            event.extract_core_fields();
            Ok(event)
        } else {
            Err(anyhow::anyhow!("Empty CSV record"))
        }
    }

    /// Create a special skip event for header lines
    fn create_skip_event(&self, line: &str) -> Event {
        let mut event = Event::default_with_line(line.to_string());
        event.set_field("__skip__".to_string(), Dynamic::from(true));
        event
    }
}

impl EventParser for CsvParser {
    fn parse(&self, line: &str) -> Result<Event> {
        // Skip empty lines
        if line.trim().is_empty() {
            return Ok(self.create_skip_event(line));
        }

        // Thread-safe header initialization
        if !self.initialized.load(Ordering::Relaxed) {
            let mut headers = self.headers.lock().unwrap();
            if headers.is_empty() {
                if self.has_headers {
                    // First line is headers - parse and store them
                    *headers = self
                        .parse_header_line(line)
                        .context("Failed to parse CSV headers")?;
                    self.initialized.store(true, Ordering::Relaxed);

                    // Return a skip event for the header line
                    return Ok(self.create_skip_event(line));
                } else {
                    // No headers - generate column names based on first data line
                    *headers = self
                        .generate_column_names(line)
                        .context("Failed to generate CSV column names")?;
                    self.initialized.store(true, Ordering::Relaxed);

                    // Fall through to parse this line as data
                }
            }
        }

        // Parse data line
        self.parse_data_line(line)
            .with_context(|| format!("Failed to parse CSV line: {}", line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_with_headers() {
        let parser = CsvParser::new_csv();

        // First line should be headers (skipped)
        let header_result = parser.parse("name,age,city").unwrap();
        assert!(header_result.fields.get("__skip__").is_some());

        // Second line should be data
        let data_result = parser.parse("Alice,25,New York").unwrap();
        assert_eq!(data_result.fields.get("name").unwrap().to_string(), "Alice");
        assert_eq!(data_result.fields.get("age").unwrap().to_string(), "25");
        assert_eq!(
            data_result.fields.get("city").unwrap().to_string(),
            "New York"
        );
    }

    #[test]
    fn test_csv_no_headers() {
        let parser = CsvParser::new_csv_no_headers();

        // First line should be data with generated column names
        let data_result = parser.parse("Alice,25,New York").unwrap();
        assert_eq!(data_result.fields.get("col1").unwrap().to_string(), "Alice");
        assert_eq!(data_result.fields.get("col2").unwrap().to_string(), "25");
        assert_eq!(
            data_result.fields.get("col3").unwrap().to_string(),
            "New York"
        );
    }

    #[test]
    fn test_tsv_with_headers() {
        let parser = CsvParser::new_tsv();

        // First line should be headers (skipped)
        let header_result = parser.parse("name\tage\tcity").unwrap();
        assert!(header_result.fields.get("__skip__").is_some());

        // Second line should be data
        let data_result = parser.parse("Alice\t25\tNew York").unwrap();
        assert_eq!(data_result.fields.get("name").unwrap().to_string(), "Alice");
        assert_eq!(data_result.fields.get("age").unwrap().to_string(), "25");
        assert_eq!(
            data_result.fields.get("city").unwrap().to_string(),
            "New York"
        );
    }

    #[test]
    fn test_csv_with_quotes() {
        let parser = CsvParser::new_csv();

        // Skip headers
        let _ = parser.parse("name,message").unwrap();

        // Parse quoted data
        let data_result = parser.parse("\"John Smith\",\"Hello, world!\"").unwrap();
        assert_eq!(
            data_result.fields.get("name").unwrap().to_string(),
            "John Smith"
        );
        assert_eq!(
            data_result.fields.get("message").unwrap().to_string(),
            "Hello, world!"
        );
    }

    #[test]
    fn test_csv_with_escaped_quotes() {
        let parser = CsvParser::new_csv();

        // Skip headers
        let _ = parser.parse("name,message").unwrap();

        // Parse data with escaped quotes
        let data_result = parser
            .parse("\"John\",\"He said \"\"hello\"\" to me\"")
            .unwrap();
        assert_eq!(data_result.fields.get("name").unwrap().to_string(), "John");
        assert_eq!(
            data_result.fields.get("message").unwrap().to_string(),
            "He said \"hello\" to me"
        );
    }

    #[test]
    fn test_csv_empty_fields() {
        let parser = CsvParser::new_csv();

        // Skip headers
        let _ = parser.parse("name,age,city").unwrap();

        // Parse data with empty fields
        let data_result = parser.parse("Alice,,Boston").unwrap();
        assert_eq!(data_result.fields.get("name").unwrap().to_string(), "Alice");
        assert_eq!(data_result.fields.get("age").unwrap().to_string(), "");
        assert_eq!(
            data_result.fields.get("city").unwrap().to_string(),
            "Boston"
        );
    }

    #[test]
    fn test_csv_variable_columns() {
        let parser = CsvParser::new_csv_no_headers();

        // Parse data with varying column counts
        let data_result1 = parser.parse("Alice,25").unwrap();
        assert_eq!(
            data_result1.fields.get("col1").unwrap().to_string(),
            "Alice"
        );
        assert_eq!(data_result1.fields.get("col2").unwrap().to_string(), "25");
        assert!(data_result1.fields.get("col3").is_none());

        // Second line with more columns (should still work)
        let data_result2 = parser.parse("Bob,30,Engineer").unwrap();
        assert_eq!(data_result2.fields.get("col1").unwrap().to_string(), "Bob");
        assert_eq!(data_result2.fields.get("col2").unwrap().to_string(), "30");
        // col3 won't exist because headers were set by first line
        assert!(data_result2.fields.get("col3").is_none());
    }
}
