mod common;
use common::*;

#[test]
fn test_help_time_topic() {
    let (stdout, _stderr, exit_code) = run_kelora(&["--help-time"]);
    assert_eq!(exit_code, 0, "--help-time should exit successfully");
    assert!(stdout.contains("Time Format Reference for --ts-format:"));
    assert!(stdout.contains("Timestamp filtering with --since and --until:"));
}

#[test]
fn test_help_formats_topic() {
    let (stdout, _stderr, exit_code) = run_kelora(&["--help-formats"]);
    assert_eq!(exit_code, 0, "--help-formats should exit successfully");
    assert!(stdout.contains("Format Reference:"));
    assert!(stdout.contains("tailmap"));
}

#[test]
fn test_help_regex_topic() {
    let (stdout, _stderr, exit_code) = run_kelora(&["--help-regex"]);
    assert_eq!(exit_code, 0, "--help-regex should exit successfully");
    assert!(stdout.contains("Regex Format Parsing Reference for -f regex:PATTERN:"));
    assert!(stdout.contains("Named capture groups (REQUIRED):"));
}

#[test]
fn test_help_rhai_topic() {
    let (stdout, _stderr, exit_code) = run_kelora(&["--help-rhai"]);
    assert_eq!(exit_code, 0, "--help-rhai should exit successfully");
    assert!(stdout.contains("Rhai Language Guide:"));
    assert!(stdout.contains("KELORA PIPELINE STAGES:"));
}

#[test]
fn test_help_multiline_topic() {
    let (stdout, _stderr, exit_code) = run_kelora(&["--help-multiline"]);
    assert_eq!(exit_code, 0, "--help-multiline should exit successfully");
    assert!(stdout.contains("Multiline Strategy Reference for --multiline:"));
    assert!(stdout.contains("regex:match=REGEX[:end=REGEX]"));
}

#[test]
fn test_help_functions_topic() {
    let (stdout, _stderr, exit_code) = run_kelora(&["--help-functions"]);
    assert_eq!(exit_code, 0, "--help-functions should exit successfully");
    assert!(stdout.contains("Available Rhai Functions:"));
    assert!(stdout.contains("text.mask_ip([octets])"));
}

#[test]
fn test_help_examples_topic() {
    let (stdout, _stderr, exit_code) = run_kelora(&["--help-examples"]);
    assert_eq!(exit_code, 0, "--help-examples should exit successfully");
    assert!(stdout.contains("Common Log Analysis Patterns:"));
    assert!(stdout.contains("SECURITY & DATA PRIVACY:"));
}
