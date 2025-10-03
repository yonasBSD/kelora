#![allow(dead_code)]
use crate::event::Event;
use crate::parsers::type_conversion::{convert_value_to_type, parse_field_with_type, TypeMap};
use crate::pipeline::EventParser;
use anyhow::{Context, Result};
use csv::ReaderBuilder;
use rhai::Dynamic;

/// CSV/TSV Parser that supports multiple dialects
///
/// Supported formats:
/// - CSV: Comma-separated values with headers
/// - TSV: Tab-separated values with headers
/// - CSVNH: Comma-separated values without headers (generates c1, c2, c3...)
/// - TSVNH: Tab-separated values without headers (generates c1, c2, c3...)
pub struct CsvParser {
    delimiter: u8,
    has_headers: bool,
    headers: Vec<String>,
    type_map: TypeMap,
    strict: bool,
}

impl CsvParser {
    /// Create a new CSV parser (comma-separated with headers)
    pub fn new_csv() -> Self {
        Self {
            delimiter: b',',
            has_headers: true,
            headers: Vec::new(),
            type_map: TypeMap::new(),
            strict: false,
        }
    }

    /// Create a new TSV parser (tab-separated with headers)
    pub fn new_tsv() -> Self {
        Self {
            delimiter: b'\t',
            has_headers: true,
            headers: Vec::new(),
            type_map: TypeMap::new(),
            strict: false,
        }
    }

    /// Create a new CSV parser without headers (comma-separated, generates c1, c2, c3...)
    pub fn new_csv_no_headers() -> Self {
        Self {
            delimiter: b',',
            has_headers: false,
            headers: Vec::new(),
            type_map: TypeMap::new(),
            strict: false,
        }
    }

    /// Create a new TSV parser without headers (tab-separated, generates c1, c2, c3...)
    pub fn new_tsv_no_headers() -> Self {
        Self {
            delimiter: b'\t',
            has_headers: false,
            headers: Vec::new(),
            type_map: TypeMap::new(),
            strict: false,
        }
    }

    /// Create a CSV parser with pre-initialized headers (for parallel processing)
    pub fn new_csv_with_headers(headers: Vec<String>) -> Self {
        Self {
            delimiter: b',',
            has_headers: true,
            headers,
            type_map: TypeMap::new(),
            strict: false,
        }
    }

    /// Create a TSV parser with pre-initialized headers (for parallel processing)
    pub fn new_tsv_with_headers(headers: Vec<String>) -> Self {
        Self {
            delimiter: b'\t',
            has_headers: true,
            headers,
            type_map: TypeMap::new(),
            strict: false,
        }
    }

    /// Create a CSV parser without headers but with pre-generated column names
    pub fn new_csv_no_headers_with_columns(headers: Vec<String>) -> Self {
        Self {
            delimiter: b',',
            has_headers: false,
            headers,
            type_map: TypeMap::new(),
            strict: false,
        }
    }

    /// Create a TSV parser without headers but with pre-generated column names
    pub fn new_tsv_no_headers_with_columns(headers: Vec<String>) -> Self {
        Self {
            delimiter: b'\t',
            has_headers: false,
            headers,
            type_map: TypeMap::new(),
            strict: false,
        }
    }

    /// Get the headers (for extracting initialized headers)
    pub fn get_headers(&self) -> Vec<String> {
        self.headers.clone()
    }

    /// Parse a field spec string and populate the type map
    /// Field spec format: "field1:int field2:float field3:bool field4"
    pub fn with_field_spec(mut self, field_spec: &str) -> Result<Self> {
        for spec in field_spec.split_whitespace() {
            let (field_name, field_type) = parse_field_with_type(spec)
                .map_err(|e| anyhow::anyhow!("Invalid field spec '{}': {}", spec, e))?;

            if let Some(ftype) = field_type {
                self.type_map.insert(field_name, ftype);
            }
        }
        Ok(self)
    }

    /// Set strict mode for type conversion
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Initialize headers from the first line if needed
    pub fn initialize_headers_from_line(&mut self, line: &str) -> Result<bool> {
        if !self.headers.is_empty() {
            // Headers already initialized
            return Ok(false);
        }

        if self.has_headers {
            // Parse headers from this line
            self.headers = self.parse_header_line(line)?;
            Ok(true) // This line was consumed as headers
        } else {
            // Generate column names based on this line
            self.headers = self.generate_column_names(line)?;
            Ok(false) // This line should still be processed as data
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

    /// Generate column names for headerless CSV (c1, c2, c3...)
    fn generate_column_names(&self, line: &str) -> Result<Vec<String>> {
        let mut reader = ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(line.as_bytes());

        if let Some(result) = reader.records().next() {
            let record = result.context("Failed to parse CSV data line for column count")?;
            let headers: Vec<String> = (1..=record.len()).map(|i| format!("c{}", i)).collect();
            Ok(headers)
        } else {
            Err(anyhow::anyhow!("Empty CSV data line"))
        }
    }

    /// Parse a data line using the initialized headers
    fn parse_data_line(&self, line: &str) -> Result<Event> {
        let mut reader = ReaderBuilder::new()
            .delimiter(self.delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(line.as_bytes());

        if let Some(result) = reader.records().next() {
            let record = result.context("Failed to parse CSV data line")?;
            let mut event = Event::with_capacity(line.to_string(), record.len());

            // Map CSV fields to event using local headers
            for (i, field) in record.iter().enumerate() {
                if let Some(header) = self.headers.get(i) {
                    // Check if this field has a type annotation
                    let value = if let Some(field_type) = self.type_map.get(header) {
                        // Apply type conversion
                        match convert_value_to_type(field, field_type, self.strict) {
                            Ok(converted) => converted,
                            Err(e) => {
                                if self.strict {
                                    return Err(anyhow::anyhow!(
                                        "Type conversion failed for field '{}': {}",
                                        header,
                                        e
                                    ));
                                } else {
                                    // In resilient mode, fall back to string
                                    Dynamic::from(field.to_string())
                                }
                            }
                        }
                    } else {
                        // No type annotation, store as string
                        Dynamic::from(field.to_string())
                    };

                    event.set_field(header.clone(), value);
                }
            }

            event.extract_timestamp();
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
        let line = line.trim_end_matches('\n').trim_end_matches('\r');
        // Skip empty lines
        if line.trim().is_empty() {
            return Ok(self.create_skip_event(line));
        }

        // If headers are not initialized, this is sequential mode - use old behavior
        if self.headers.is_empty() {
            return Err(anyhow::anyhow!(
                "CSV parser not properly initialized. This should not happen in normal usage."
            ));
        }

        // Parse data line using pre-initialized headers
        self.parse_data_line(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_with_headers() {
        let mut parser = CsvParser::new_csv();

        // Initialize headers from first line
        let header_line = "name,age,city";
        let was_consumed = parser.initialize_headers_from_line(header_line).unwrap();
        assert!(was_consumed); // Header line should be consumed

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
        let mut parser = CsvParser::new_csv_no_headers();

        // Initialize column names from first line
        let data_line = "Alice,25,New York";
        let was_consumed = parser.initialize_headers_from_line(data_line).unwrap();
        assert!(!was_consumed); // Data line should not be consumed

        // First line should be data with generated column names
        let data_result = parser.parse("Alice,25,New York").unwrap();
        assert_eq!(data_result.fields.get("c1").unwrap().to_string(), "Alice");
        assert_eq!(data_result.fields.get("c2").unwrap().to_string(), "25");
        assert_eq!(
            data_result.fields.get("c3").unwrap().to_string(),
            "New York"
        );
    }

    #[test]
    fn test_tsv_with_headers() {
        let mut parser = CsvParser::new_tsv();

        // Initialize headers from first line
        let header_line = "name\tage\tcity";
        let was_consumed = parser.initialize_headers_from_line(header_line).unwrap();
        assert!(was_consumed); // Header line should be consumed

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
        let mut parser = CsvParser::new_csv();

        // Initialize headers
        let _ = parser.initialize_headers_from_line("name,message").unwrap();

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
        let mut parser = CsvParser::new_csv();

        // Initialize headers
        let _ = parser.initialize_headers_from_line("name,message").unwrap();

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
        let mut parser = CsvParser::new_csv();

        // Initialize headers
        let _ = parser
            .initialize_headers_from_line("name,age,city")
            .unwrap();

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
        let mut parser = CsvParser::new_csv_no_headers();

        // Initialize column names from first line
        let first_line = "Alice,25";
        let _ = parser.initialize_headers_from_line(first_line).unwrap();

        // Parse data with initial column count
        let data_result1 = parser.parse("Alice,25").unwrap();
        assert_eq!(data_result1.fields.get("c1").unwrap().to_string(), "Alice");
        assert_eq!(data_result1.fields.get("c2").unwrap().to_string(), "25");
        assert!(data_result1.fields.get("c3").is_none());

        // Second line with more columns (extra columns ignored)
        let data_result2 = parser.parse("Bob,30,Engineer").unwrap();
        assert_eq!(data_result2.fields.get("c1").unwrap().to_string(), "Bob");
        assert_eq!(data_result2.fields.get("c2").unwrap().to_string(), "30");
        // c3 won't exist because headers were set by first line
        assert!(data_result2.fields.get("c3").is_none());
    }
}
