//! Sampling strategies for log analysis
//!
//! Provides different approaches to sampling large log files without
//! loading them entirely into memory.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

/// Sampling strategy for reading log files
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SamplingStrategy {
    /// Read only the first N lines (fast, but biased toward start)
    Head(usize),

    /// Stratified sampling: read from beginning, middle, and end
    Stratified {
        head: usize,
        middle: usize,
        tail: usize,
    },
}

impl Default for SamplingStrategy {
    fn default() -> Self {
        Self::Stratified {
            head: 400,
            middle: 300,
            tail: 300,
        }
    }
}

/// Result of sampling operation
#[derive(Debug, Clone)]
pub struct Sample {
    /// Sampled lines
    pub lines: Vec<String>,
    /// Total lines in file (estimated for large files)
    pub total_lines_estimate: usize,
    /// Number of files sampled
    pub files_sampled: usize,
    /// Whether sampling was truncated
    pub truncated: bool,
}

impl Sample {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            total_lines_estimate: 0,
            files_sampled: 0,
            truncated: false,
        }
    }

    pub fn coverage_percent(&self) -> f64 {
        if self.total_lines_estimate == 0 {
            100.0
        } else {
            (self.lines.len() as f64 / self.total_lines_estimate as f64 * 100.0).min(100.0)
        }
    }
}

impl Default for Sample {
    fn default() -> Self {
        Self::new()
    }
}

/// Sample lines from multiple files
pub fn sample_files(
    paths: &[&Path],
    strategy: &SamplingStrategy,
    max_lines: usize,
) -> Result<Sample> {
    let mut sample = Sample::new();

    if paths.is_empty() {
        return Ok(sample);
    }

    // Distribute sample size across files
    let lines_per_file = max_lines / paths.len().max(1);

    for path in paths {
        let file_sample = sample_file(path, strategy, lines_per_file)?;
        sample.lines.extend(file_sample.lines);
        sample.total_lines_estimate += file_sample.total_lines_estimate;
        sample.files_sampled += 1;
        sample.truncated |= file_sample.truncated;
    }

    // Trim to max if we got too many
    if sample.lines.len() > max_lines {
        sample.lines.truncate(max_lines);
        sample.truncated = true;
    }

    Ok(sample)
}

/// Sample lines from a single file
pub fn sample_file(path: &Path, strategy: &SamplingStrategy, max_lines: usize) -> Result<Sample> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let file_size = file.metadata()?.len();

    match strategy {
        SamplingStrategy::Head(n) => sample_head(path, (*n).min(max_lines)),
        SamplingStrategy::Stratified { head, middle, tail } => {
            sample_stratified(path, file_size, *head, *middle, *tail, max_lines)
        }
    }
}

/// Read first N lines from file
fn sample_head(path: &Path, n: usize) -> Result<Sample> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut sample = Sample::new();

    for line in reader.lines().take(n) {
        sample.lines.push(line?);
    }

    sample.total_lines_estimate = estimate_total_lines(path, sample.lines.len())?;
    sample.truncated = sample.lines.len() >= n;
    sample.files_sampled = 1;

    Ok(sample)
}

/// Stratified sampling: read from head, middle, and tail of file
fn sample_stratified(
    path: &Path,
    file_size: u64,
    head_count: usize,
    middle_count: usize,
    tail_count: usize,
    max_lines: usize,
) -> Result<Sample> {
    let mut sample = Sample::new();

    // For small files, just read everything
    if file_size < 100_000 {
        // < 100KB
        return sample_head(path, max_lines);
    }

    // Read from head
    let head_lines = read_lines_from_offset(path, 0, head_count)?;
    sample.lines.extend(head_lines);

    // Read from middle (~50% into file)
    let middle_offset = file_size / 2;
    let middle_lines = read_lines_from_offset(path, middle_offset, middle_count)?;
    sample.lines.extend(middle_lines);

    // Read from tail (~90% into file to catch recent entries)
    let tail_offset = (file_size as f64 * 0.9) as u64;
    let tail_lines = read_lines_from_offset(path, tail_offset, tail_count)?;
    sample.lines.extend(tail_lines);

    sample.total_lines_estimate = estimate_total_lines(path, sample.lines.len())?;
    sample.truncated = true; // Stratified sampling always indicates truncation
    sample.files_sampled = 1;

    // Trim to max
    if sample.lines.len() > max_lines {
        sample.lines.truncate(max_lines);
    }

    Ok(sample)
}

/// Read lines starting from a byte offset
fn read_lines_from_offset(path: &Path, offset: u64, count: usize) -> Result<Vec<String>> {
    let mut file = File::open(path)?;

    if offset > 0 {
        file.seek(SeekFrom::Start(offset))?;
    }

    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut iter = reader.lines();

    // Skip partial first line if we seeked
    if offset > 0 {
        let _ = iter.next();
    }

    for line in iter.take(count) {
        lines.push(line?);
    }

    Ok(lines)
}

/// Estimate total lines in file based on sample
fn estimate_total_lines(path: &Path, sample_size: usize) -> Result<usize> {
    let file_size = std::fs::metadata(path)?.len();

    if file_size == 0 || sample_size == 0 {
        return Ok(0);
    }

    // Read a small sample to estimate average line length
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut total_bytes = 0usize;
    let mut line_count = 0usize;

    for line in reader.lines().take(100) {
        let line = line?;
        total_bytes += line.len() + 1; // +1 for newline
        line_count += 1;
    }

    if line_count == 0 {
        return Ok(0);
    }

    let avg_line_length = total_bytes as f64 / line_count as f64;
    let estimated_lines = (file_size as f64 / avg_line_length) as usize;

    Ok(estimated_lines.max(sample_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(lines: &[&str]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(file, "{}", line).unwrap();
        }
        file.flush().unwrap();
        file
    }

    #[test]
    fn sample_head_reads_first_n_lines() {
        let file = create_test_file(&["line1", "line2", "line3", "line4", "line5"]);
        let sample = sample_head(file.path(), 3).unwrap();

        assert_eq!(sample.lines.len(), 3);
        assert_eq!(sample.lines[0], "line1");
        assert_eq!(sample.lines[2], "line3");
    }

    #[test]
    fn sample_handles_empty_file() {
        let file = create_test_file(&[]);
        let sample = sample_head(file.path(), 10).unwrap();

        assert!(sample.lines.is_empty());
    }

    #[test]
    fn sample_handles_file_smaller_than_requested() {
        let file = create_test_file(&["a", "b"]);
        let sample = sample_head(file.path(), 100).unwrap();

        assert_eq!(sample.lines.len(), 2);
        assert!(!sample.truncated);
    }

    #[test]
    fn coverage_percent_calculated_correctly() {
        let mut sample = Sample::new();
        sample.lines = vec!["a".to_string(); 100];
        sample.total_lines_estimate = 1000;

        assert!((sample.coverage_percent() - 10.0).abs() < 0.01);
    }
}
