// tests/common/mod.rs
// Shared test utilities for integration tests
#![allow(dead_code)]

use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

/// Helper function to run kelora with given arguments and input via stdin
pub fn run_kelora_with_input(args: &[&str], input: &str) -> (String, String, i32) {
    // Use the built binary directly instead of cargo run to avoid compilation output
    let binary_path = if cfg!(debug_assertions) {
        "./target/debug/kelora"
    } else {
        "./target/release/kelora"
    };

    let mut cmd = Command::new(binary_path)
        .args(args)
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

    // Use the built binary directly instead of cargo run to avoid compilation output
    let binary_path = if cfg!(debug_assertions) {
        "./target/debug/kelora"
    } else {
        "./target/release/kelora"
    };

    let cmd = Command::new(binary_path)
        .args(&full_args)
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

    let binary_path = if cfg!(debug_assertions) {
        "./target/debug/kelora"
    } else {
        "./target/release/kelora"
    };

    let output = Command::new(binary_path)
        .args(&full_args)
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
