// tests/context_integration_test.rs
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

/// Helper function to run kelora with given arguments and input via stdin
fn run_kelora_with_input(args: &[&str], input: &str) -> (String, String, i32) {
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
fn _run_kelora_with_file(args: &[&str], file_content: &str) -> (String, String, i32) {
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

// Test data for context tests
const SAMPLE_JSON_LOGS: &str = r#"{"level": "info", "msg": "normal message 1", "ts": "2024-01-01T10:00:01Z"}
{"level": "debug", "msg": "debug message", "ts": "2024-01-01T10:00:02Z"}
{"level": "error", "msg": "ERROR: something went wrong", "ts": "2024-01-01T10:00:03Z"}
{"level": "info", "msg": "normal message 2", "ts": "2024-01-01T10:00:04Z"}
{"level": "warn", "msg": "warning message", "ts": "2024-01-01T10:00:05Z"}
{"level": "info", "msg": "normal message 3", "ts": "2024-01-01T10:00:06Z"}"#;

// VALIDATION TESTS

#[test]
fn test_context_requires_filtering() {
    let (stdout, stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-A", "1"], SAMPLE_JSON_LOGS);

    assert_ne!(exit_code, 0, "Context without filtering should fail");
    assert!(
        stderr.contains("Context options (-A, -B, -C) require active filtering"),
        "Should show context validation error message"
    );
    assert_eq!(stdout.trim(), "", "No output should be produced on validation error");
}

#[test]
fn test_context_before_requires_filtering() {
    let (_stdout, stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-B", "2"], SAMPLE_JSON_LOGS);

    assert_ne!(exit_code, 0, "Before context without filtering should fail");
    assert!(
        stderr.contains("Context options (-A, -B, -C) require active filtering"),
        "Should show context validation error message"
    );
}

#[test]
fn test_context_combined_requires_filtering() {
    let (_stdout, stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-C", "1"], SAMPLE_JSON_LOGS);

    assert_ne!(exit_code, 0, "Combined context without filtering should fail");
    assert!(
        stderr.contains("Context options (-A, -B, -C) require active filtering"),
        "Should show context validation error message"
    );
}

#[test]
fn test_context_with_filter_succeeds() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-A", "1"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with filtering should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output with filtering");
}

#[test]
fn test_context_with_levels_succeeds() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--levels", "error", "-B", "1"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with level filtering should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output with level filtering");
}

// FORMATTING AND PREFIX TESTS

#[test]
fn test_context_prefix_formatting() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-A", "1", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context formatting should succeed");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert!(!lines.is_empty(), "Should have output lines");

    // Check that the error line has a match prefix (*)
    let has_match_prefix = lines.iter().any(|line| {
        line.starts_with("* ") && line.contains("ERROR: something went wrong")
    });
    assert!(has_match_prefix, "Error line should have match prefix (*)");
}

#[test]
fn test_context_without_prefix_when_disabled() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Filtering without context should succeed");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert!(!lines.is_empty(), "Should have output lines");

    // Check that without context options, there should be no prefix
    let has_no_prefix = lines.iter().any(|line| {
        !line.starts_with("* ") && !line.starts_with("/ ") && !line.starts_with("\\ ")
        && line.contains("ERROR: something went wrong")
    });
    assert!(has_no_prefix, "Without context options, lines should have no prefix");
}

#[test]
fn test_after_context_option() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-A", "1", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "After context should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");

    // Verify that the match line has the * prefix
    assert!(stdout.contains("* "), "Should have match prefix");
}

#[test]
fn test_before_context_option() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-B", "1", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Before context should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");

    // Verify that the match line has the * prefix
    assert!(stdout.contains("* "), "Should have match prefix");
}

#[test]
fn test_combined_context_option() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-C", "1", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Combined context should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");

    // Verify that the match line has the * prefix
    assert!(stdout.contains("* "), "Should have match prefix");
}

// FILTERING MODES TESTS

#[test]
fn test_context_with_level_filtering() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "-l", "error,warn", "-A", "1", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with level filtering should succeed");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert!(!lines.is_empty(), "Should have output lines");

    // Should have both error and warning lines with match prefixes
    let has_error_match = lines.iter().any(|line| {
        line.starts_with("* ") && line.contains("ERROR: something went wrong")
    });
    let has_warn_match = lines.iter().any(|line| {
        line.starts_with("* ") && line.contains("warning message")
    });

    assert!(has_error_match, "Should have error line with match prefix");
    assert!(has_warn_match, "Should have warning line with match prefix");
}

#[test]
fn test_context_with_exclude_levels() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "-L", "debug,info", "-B", "1", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with exclude levels should succeed");

    let output = stdout.trim();
    assert!(!output.is_empty(), "Should have output");

    // Should exclude debug and info, so only error and warn should appear
    assert!(output.contains("ERROR: something went wrong"), "Should include error");
    assert!(output.contains("warning message"), "Should include warning");
    assert!(!output.contains("debug message"), "Should exclude debug");
    assert!(!output.contains("normal message"), "Should exclude info messages");
}

#[test]
fn test_context_with_custom_filter() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.msg.contains(\"ERROR\")", "-C", "1", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with custom filter should succeed");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert!(!lines.is_empty(), "Should have output lines");

    // Should find the error message
    let has_error_match = lines.iter().any(|line| {
        line.starts_with("* ") && line.contains("ERROR: something went wrong")
    });
    assert!(has_error_match, "Should find error message with match prefix");
}

// DIFFERENT INPUT FORMATS TESTS

#[test]
fn test_context_with_line_format() {
    let line_input = "normal line 1\nerror occurred here\nnormal line 2\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "line", "--filter", "e.line.contains(\"error\")", "-A", "1", "--no-color"],
        line_input
    );

    assert_eq!(exit_code, 0, "Context with line format should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");
    assert!(stdout.contains("* "), "Should have match prefix");
}

#[test]
fn test_context_with_logfmt_format() {
    let logfmt_input = "level=info msg=\"normal message\"\nlevel=error msg=\"error occurred\"\nlevel=info msg=\"another message\"\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "logfmt", "--filter", "e.level == \"error\"", "-B", "1", "--no-color"],
        logfmt_input
    );

    assert_eq!(exit_code, 0, "Context with logfmt format should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");
    assert!(stdout.contains("* "), "Should have match prefix");
}

// STRUCTURED OUTPUT FORMATS TESTS

#[test]
fn test_context_preserves_json_output_structure() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "-F", "json", "--filter", "e.level == \"error\"", "-A", "1"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with JSON output should succeed");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert!(!lines.is_empty(), "Should have output lines");

    // All lines should be valid JSON (no prefixes in JSON output)
    for line in lines {
        if !line.trim().is_empty() {
            let parse_result: Result<serde_json::Value, _> = serde_json::from_str(line);
            assert!(parse_result.is_ok(), "JSON output should be valid JSON: {}", line);
        }
    }
}

#[test]
fn test_context_with_csv_output() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "-F", "csv", "--keys", "level,msg", "--filter", "e.level == \"error\"", "-A", "1"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with CSV output should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");

    // CSV output should not have context prefixes
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    for line in lines {
        assert!(!line.starts_with("* "), "CSV output should not have context prefixes");
        assert!(!line.starts_with("/ "), "CSV output should not have context prefixes");
        assert!(!line.starts_with("\\ "), "CSV output should not have context prefixes");
    }
}

// PARALLEL PROCESSING TESTS

#[test]
fn test_context_with_parallel_processing() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-A", "1", "--parallel", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with parallel processing should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output with parallel processing");

    // Should still have match prefixes in parallel mode
    assert!(stdout.contains("* "), "Should have match prefix in parallel mode");
}

#[test]
fn test_context_with_parallel_and_unordered() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-C", "1", "--parallel", "--unordered", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with parallel unordered should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");

    // Should still have match prefixes even with unordered output
    assert!(stdout.contains("* "), "Should have match prefix with unordered output");
}

// EDGE CASES AND ERROR HANDLING

#[test]
fn test_context_with_zero_value() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-A", "0"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with zero value should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");

    // With -A 0, context is disabled so no prefixes should appear
    assert!(!stdout.contains("* "), "Should have no prefix with -A 0 (context disabled)");
    assert!(!stdout.contains("/ "), "Should have no prefix with -A 0 (context disabled)");
    assert!(!stdout.contains("\\ "), "Should have no prefix with -A 0 (context disabled)");
}

#[test]
fn test_context_with_large_value() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-B", "100"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with large value should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");
}

#[test]
fn test_context_with_empty_input() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-C", "1"],
        ""
    );

    assert_eq!(exit_code, 0, "Context with empty input should succeed");
    assert_eq!(stdout.trim(), "", "Empty input should produce no output");
}

#[test]
fn test_context_with_no_matches() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"critical\"", "-A", "2"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with no matches should succeed");
    assert_eq!(stdout.trim(), "", "No matches should produce no output");
}

// INTEGRATION WITH OTHER FEATURES

#[test]
fn test_context_with_brief_mode() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-A", "1", "--brief", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with brief mode should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");
    assert!(stdout.contains("* "), "Should have match prefix in brief mode");
}

#[test]
fn test_context_with_take_limit() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level != \"\"", "-A", "1", "--take", "2", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with take limit should succeed");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert!(lines.len() <= 2, "Should respect take limit");
}

#[test]
fn test_context_with_window_option() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.level == \"error\"", "-C", "1", "--window", "5", "--no-color"],
        SAMPLE_JSON_LOGS
    );

    assert_eq!(exit_code, 0, "Context with window option should succeed");
    assert!(!stdout.trim().is_empty(), "Should produce output");
    assert!(stdout.contains("* "), "Should have match prefix with window option");
}