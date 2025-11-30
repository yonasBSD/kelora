// tests/common/mod.rs
// Shared test utilities for integration tests (only a subset is exercised in current suites)
#![allow(dead_code)] // Helpers are used selectively by specific integration tests

use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

/// Helper function to run kelora with given arguments and input via stdin
pub fn run_kelora_with_input(args: &[&str], input: &str) -> (String, String, i32) {
    // Use CARGO_BIN_EXE_kelora env var set by cargo during test runs
    // This works correctly for regular builds, coverage builds, and custom target dirs
    let binary_path = env!("CARGO_BIN_EXE_kelora");

    let mut cmd = Command::new(binary_path)
        .args(args)
        .env("LLVM_PROFILE_FILE", "/dev/null") // Disable profraw generation for subprocesses
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start kelora");

    // Write input to stdin
    if let Some(stdin) = cmd.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .expect("Failed to write to stdin");
    }

    let output = cmd.wait_with_output().expect("Failed to read output");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Helper function to run kelora with a temporary file
pub fn run_kelora_with_file(args: &[&str], file_content: &str) -> (String, String, i32) {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(file_content.as_bytes())
        .expect("Failed to write to temp file");

    let mut full_args = args.to_vec();
    full_args.push(temp_file.path().to_str().unwrap());

    // Use CARGO_BIN_EXE_kelora env var set by cargo during test runs
    // This works correctly for regular builds, coverage builds, and custom target dirs
    let binary_path = env!("CARGO_BIN_EXE_kelora");

    let cmd = Command::new(binary_path)
        .args(&full_args)
        .env("LLVM_PROFILE_FILE", "/dev/null") // Disable profraw generation for subprocesses
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute kelora");

    (
        String::from_utf8_lossy(&cmd.stdout).to_string(),
        String::from_utf8_lossy(&cmd.stderr).to_string(),
        cmd.status.code().unwrap_or(-1),
    )
}

/// Helper function to run kelora with multiple files
pub fn run_kelora_with_files(args: &[&str], files: &[&str]) -> (String, String, i32) {
    let mut full_args = args.to_vec();
    full_args.extend(files);

    // Use CARGO_BIN_EXE_kelora env var set by cargo during test runs
    // This works correctly for regular builds, coverage builds, and custom target dirs
    let binary_path = env!("CARGO_BIN_EXE_kelora");

    let output = Command::new(binary_path)
        .args(&full_args)
        .env("LLVM_PROFILE_FILE", "/dev/null") // Disable profraw generation for subprocesses
        .output()
        .expect("Failed to execute kelora");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Helper function to run kelora without any input (for --no-input flag)
pub fn run_kelora(args: &[&str]) -> (String, String, i32) {
    // Use CARGO_BIN_EXE_kelora env var set by cargo during test runs
    // This works correctly for regular builds, coverage builds, and custom target dirs
    let binary_path = env!("CARGO_BIN_EXE_kelora");

    let output = Command::new(binary_path)
        .args(args)
        .env("LLVM_PROFILE_FILE", "/dev/null") // Disable profraw generation for subprocesses
        .output()
        .expect("Failed to execute kelora");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Helper to extract event count from stats stderr output
pub fn extract_events_created_from_stats(stderr: &str) -> i32 {
    for line in stderr.lines() {
        if line.contains("Events created:") {
            // Parse "Events created: X total, Y output, Z filtered"
            if let Some(events_part) = line.split("Events created:").nth(1) {
                if let Some(total_part) = events_part.trim().split(" total").next() {
                    return total_part.trim().parse().unwrap_or(0);
                }
            }
        }
    }
    0
}

/// Extract the lines from the stats block in stderr (emoji and plain prefixes supported)
pub fn extract_stats_lines(stderr: &str) -> Vec<String> {
    let mut in_stats = false;
    let mut stats_lines = Vec::new();

    for raw_line in stderr.lines() {
        let trimmed = raw_line.trim();

        if !in_stats {
            if trimmed == "📈 Stats:" || trimmed == "kelora: Stats:" || trimmed == "Stats:" {
                in_stats = true;
            }
            continue;
        }

        if trimmed.is_empty() {
            break;
        }

        stats_lines.push(trimmed.to_string());
    }

    stats_lines
}

/// Find a specific stats line by prefix within the extracted stats block
pub fn stats_line<'a>(stats: &'a [String], prefix: &str) -> &'a str {
    stats
        .iter()
        .find(|line| line.starts_with(prefix))
        .unwrap_or_else(|| {
            panic!(
                "Expected stats line starting with '{}', but stats were {:?}",
                prefix, stats
            )
        })
}

/// Helper to extract filtered count from stats stderr output
pub fn extract_events_filtered_from_stats(stderr: &str) -> i32 {
    for line in stderr.lines() {
        if line.contains("Events created:") {
            // Parse "Events created: X total, Y output, Z filtered"
            if let Some(filtered_part) = line.split(", ").nth(2) {
                if let Some(num_part) = filtered_part.split(" filtered").next() {
                    return num_part.trim().parse().unwrap_or(0);
                }
            }
        }
    }
    0
}
