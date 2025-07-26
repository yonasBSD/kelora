// tests/integration_tests.rs
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
fn run_kelora_with_file(args: &[&str], file_content: &str) -> (String, String, i32) {
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

#[test]
fn test_version_flag() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["--version"], "");
    assert_eq!(exit_code, 0, "kelora --version should exit successfully");
    assert!(
        stdout.contains("kelora 0.2.0"),
        "Version output should contain version number"
    );
}

#[test]
fn test_help_flag() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["--help"], "");
    assert_eq!(exit_code, 0, "kelora --help should exit successfully");
    assert!(
        stdout.contains("command-line log analysis tool"),
        "Help should describe the tool"
    );
    assert!(
        stdout.contains("--filter"),
        "Help should mention filter option"
    );
    assert!(
        stdout.contains("--parallel"),
        "Help should mention parallel option"
    );
}

#[test]
fn test_basic_jsonl_parsing() {
    let input = r#"{"level": "INFO", "message": "Hello world", "status": 200}
{"level": "ERROR", "message": "Something failed", "status": 500}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "-F", "jsonl"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines");

    // Parse JSON output
    let first_line: serde_json::Value =
        serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["level"], "INFO");
    assert_eq!(first_line["status"], 200);
}

#[test]
fn test_filter_expression() {
    let input = r#"{"level": "INFO", "status": 200}
{"level": "ERROR", "status": 500}
{"level": "DEBUG", "status": 404}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "jsonl", "-F", "jsonl", "--filter", "e.status >= 400"],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should filter to 2 lines (status >= 400)");

    // Check that filtered results have status >= 400
    for line in lines {
        let parsed: serde_json::Value =
            serde_json::from_str(line).expect("Line should be valid JSON");
        let status = parsed["status"]
            .as_i64()
            .expect("Status should be a number");
        assert!(status >= 400, "Filtered results should have status >= 400");
    }
}

#[test]
fn test_exec_script() {
    let input = r#"{"level": "INFO", "status": 200}
{"level": "ERROR", "status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--exec",
            "e.alert_level = if e.status >= 400 { \"high\" } else { \"low\" };",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines");

    // Check that exec script added alert_level field
    let first_line: serde_json::Value =
        serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["alert_level"], "low");

    let second_line: serde_json::Value =
        serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(second_line["alert_level"], "high");
}

#[test]
fn test_text_output_format() {
    let input = r#"{"level": "INFO", "message": "Hello world", "status": 200}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "-F", "default"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Text format should be key=value pairs
    assert!(
        stdout.contains("level=\"INFO\""),
        "Text output should contain level=\"INFO\""
    );
    assert!(
        stdout.contains("status=200"),
        "Text output should contain status=200"
    );
    assert!(
        stdout.contains("message=\"Hello world\""),
        "Text output should contain quoted message"
    );
}

#[test]
fn test_cols_input_format() {
    let input = "field1 field2 field3\none    two\tthree\nfour five six seven";

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "cols", "-k", "c1,c2,c3"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should have 3 output lines");

    // First line: field1 field2 field3
    assert!(
        lines[0].contains("c1=\"field1\""),
        "First line should contain c1=\"field1\""
    );
    assert!(
        lines[0].contains("c2=\"field2\""),
        "First line should contain c2=\"field2\""
    );
    assert!(
        lines[0].contains("c3=\"field3\""),
        "First line should contain c3=\"field3\""
    );

    // Second line: one two three (handles mixed whitespace)
    assert!(
        lines[1].contains("c1=\"one\""),
        "Second line should contain c1=\"one\""
    );
    assert!(
        lines[1].contains("c2=\"two\""),
        "Second line should contain c2=\"two\""
    );
    assert!(
        lines[1].contains("c3=\"three\""),
        "Second line should contain c3=\"three\""
    );

    // Third line: four five six seven (has more than 3 fields)
    assert!(
        lines[2].contains("c1=\"four\""),
        "Third line should contain c1=\"four\""
    );
    assert!(
        lines[2].contains("c2=\"five\""),
        "Third line should contain c2=\"five\""
    );
    assert!(
        lines[2].contains("c3=\"six\""),
        "Third line should contain c3=\"six\""
    );
}

#[test]
fn test_cols_format_with_filtering() {
    let input = "2023-01-01 10:30:00 ERROR database connection_failed\n2023-01-01 10:31:00 INFO user login_success";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "cols",
            "--filter",
            "e.c3 == \"ERROR\"",
            "-k",
            "c1,c2,c3,c4",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 1, "Should have 1 output line (ERROR only)");

    // Should only have the ERROR line
    assert!(
        lines[0].contains("c1=\"2023-01-01\""),
        "Should contain the date"
    );
    assert!(
        lines[0].contains("c2=\"10:30:00\""),
        "Should contain the time"
    );
    assert!(
        lines[0].contains("c3=\"ERROR\""),
        "Should contain the level"
    );
    assert!(
        lines[0].contains("c4=\"database\""),
        "Should contain the component"
    );
}

#[test]
fn test_cols_format_with_exec() {
    let input = "user1 200 1.23\nuser2 404 0.45\nuser3 200 2.10";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "cols",
            "--exec",
            "print(e.c1 + \" status=\" + e.c2 + \" time=\" + e.c3)",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Should contain both the print output and the formatted event
    assert!(
        stdout.contains("user1 status=200 time=1.23"),
        "Should contain exec print output for user1"
    );
    assert!(
        stdout.contains("user2 status=404 time=0.45"),
        "Should contain exec print output for user2"
    );
    assert!(
        stdout.contains("user3 status=200 time=2.10"),
        "Should contain exec print output for user3"
    );
}

#[test]
fn test_keys_filtering() {
    let input = r#"{"level": "INFO", "message": "Hello world", "status": 200, "timestamp": "2023-01-01T00:00:00Z"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "jsonl", "-F", "jsonl", "--keys", "level,status"],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("Output should be valid JSON");

    // Should only contain specified keys
    assert!(parsed.get("level").is_some(), "Should contain level");
    assert!(parsed.get("status").is_some(), "Should contain status");
    assert!(
        parsed.get("message").is_none(),
        "Should not contain message"
    );
    assert!(
        parsed.get("timestamp").is_none(),
        "Should not contain timestamp"
    );
}

#[test]
fn test_global_tracking() {
    let input = r#"{"level": "INFO", "status": 200}
{"level": "ERROR", "status": 500}
{"level": "ERROR", "status": 404}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--filter",
            "e.status >= 400",
            "--exec",
            "track_count(\"errors\")",
            "--end",
            "print(`Errors: ${tracked[\"errors\"]}`)",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // The end stage should print to stdout (Rhai print goes to stdout in this implementation)
    assert!(
        stdout.contains("Errors: 2"),
        "Should track filtered error lines"
    );
}

#[test]
fn test_begin_and_end_stages() {
    let input = r#"{"level": "INFO"}
{"level": "ERROR"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--begin",
            "print(\"Starting analysis...\")",
            "--end",
            "print(\"Analysis complete\")",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    assert!(
        stdout.contains("Starting analysis..."),
        "Begin stage should execute"
    );
    assert!(
        stdout.contains("Analysis complete"),
        "End stage should execute"
    );
}

#[test]
fn test_error_handling_resilient_mode() {
    let input = r#"{"level": "INFO", "status": 200}
invalid jsonl line
{"level": "ERROR", "status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl"], input);
    assert_eq!(
        exit_code, 1,
        "kelora should exit with error code when errors occur, even in resilient mode"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        2,
        "Should skip invalid line and output 2 valid lines"
    );
}

#[test]
fn test_error_handling_resilient_with_summary() {
    let input = r#"{"level": "INFO", "status": 200}
invalid jsonl line
{"level": "ERROR", "status": 500}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "-F", "jsonl"], input);
    assert_eq!(
        exit_code, 1,
        "kelora should exit with error code when errors occur, even in resilient mode"
    );

    let lines: Vec<&str> = stdout
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        2,
        "Should output 2 valid lines, skipping invalid line"
    );

    // In resilient mode, invalid lines are skipped, not emitted as events
    // Check that both valid lines are properly formatted JSON
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line).expect(&format!(
            "All output lines should be valid JSON, but got: '{}'",
            line
        ));
    }

    // In resilient mode, parsing errors are handled silently by skipping invalid lines
    // This behavior may or may not produce stderr output depending on implementation details
}

#[test]
fn test_parallel_mode() {
    let input = r#"{"level": "INFO", "status": 200}
{"level": "ERROR", "status": 500}
{"level": "DEBUG", "status": 404}
{"level": "WARN", "status": 403}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--parallel",
            "--threads",
            "2",
            "--filter",
            "e.status >= 400",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully in parallel mode"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should filter to 3 lines in parallel mode");

    // Verify all results have status >= 400
    for line in lines {
        let parsed: serde_json::Value =
            serde_json::from_str(line).expect("Line should be valid JSON");
        let status = parsed["status"]
            .as_i64()
            .expect("Status should be a number");
        assert!(
            status >= 400,
            "Parallel filtered results should have status >= 400"
        );
    }
}

#[test]
fn test_parallel_sequential_equivalence() {
    let input = r#"{"level": "INFO", "status": 200, "user": "alice"}
{"level": "ERROR", "status": 500, "user": "bob"}
{"level": "DEBUG", "status": 404, "user": "charlie"}
{"level": "WARN", "status": 403, "user": "david"}
{"level": "INFO", "status": 201, "user": "eve"}
{"level": "ERROR", "status": 502, "user": "frank"}"#;

    // Run sequential mode
    let (seq_stdout, _seq_stderr, seq_exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--filter",
            "e.status >= 400",
            "--exec",
            "let processed = true",
        ],
        input,
    );

    // Run parallel mode
    let (par_stdout, _par_stderr, par_exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--parallel",
            "--threads",
            "2",
            "--filter",
            "e.status >= 400",
            "--exec",
            "let processed = true",
        ],
        input,
    );

    // Both should exit successfully
    assert_eq!(seq_exit_code, 0, "Sequential mode should exit successfully");
    assert_eq!(par_exit_code, 0, "Parallel mode should exit successfully");

    // Parse and sort output lines for comparison (parallel may reorder)
    let mut seq_lines: Vec<&str> = seq_stdout
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty() && l.starts_with('{'))
        .collect();
    let mut par_lines: Vec<&str> = par_stdout
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty() && l.starts_with('{'))
        .collect();

    seq_lines.sort();
    par_lines.sort();

    // Should have same number of results
    assert_eq!(
        seq_lines.len(),
        par_lines.len(),
        "Sequential and parallel should produce same number of results"
    );

    // Results should be functionally equivalent (same filtered and processed records)
    for (seq_line, par_line) in seq_lines.iter().zip(par_lines.iter()) {
        let seq_json: serde_json::Value =
            serde_json::from_str(seq_line).expect("Sequential output should be valid JSON");
        let par_json: serde_json::Value =
            serde_json::from_str(par_line).expect("Parallel output should be valid JSON");

        // Check that key fields match
        assert_eq!(
            seq_json["status"], par_json["status"],
            "Status should match between modes"
        );
        assert_eq!(
            seq_json["user"], par_json["user"],
            "User should match between modes"
        );
        assert_eq!(
            seq_json["processed"], par_json["processed"],
            "Processed field should match between modes"
        );

        // Verify filtering worked correctly in both modes
        let status = seq_json["status"]
            .as_i64()
            .expect("Status should be a number");
        assert!(status >= 400, "Both modes should filter correctly");
    }

    // Verify both modes processed the same data successfully
    assert!(
        seq_lines.len() > 0,
        "Sequential mode should produce some output"
    );
    assert!(
        par_lines.len() > 0,
        "Parallel mode should produce some output"
    );
}

#[test]
fn test_file_input() {
    let file_content = r#"{"level": "INFO", "message": "File input test"}
{"level": "ERROR", "message": "Another line"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_file(&["-f", "jsonl"], file_content);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with file input"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines from file");
}

#[test]
fn test_empty_input() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl"], "");
    assert_eq!(exit_code, 0, "kelora should handle empty input gracefully");
    assert_eq!(stdout.trim(), "", "Empty input should produce no output");
}

#[test]
fn test_string_functions() {
    let input = r#"{"message": "Error: Something failed", "code": "123"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--exec",
            "e.has_error = e.message.contains(\"Error\"); e.code_num = e.code.to_int();",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("Output should be valid JSON");
    assert_eq!(parsed["has_error"], true, "contains() function should work");
    assert_eq!(parsed["code_num"], 123, "to_int() function should work");
}

#[test]
fn test_multiple_filters() {
    let input = r#"{"level": "INFO", "status": 200, "response_time": 50}
{"level": "ERROR", "status": 500, "response_time": 100}
{"level": "WARN", "status": 404, "response_time": 200}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--filter",
            "e.status >= 400",
            "--filter",
            "e.response_time > 150",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        1,
        "Should filter to 1 line matching both conditions"
    );

    let parsed: serde_json::Value =
        serde_json::from_str(lines[0]).expect("Line should be valid JSON");
    assert_eq!(parsed["level"], "WARN");
    assert_eq!(parsed["status"], 404);
    assert_eq!(parsed["response_time"], 200);
}

#[test]
fn test_status_class_function() {
    let input = r#"{"status": 200}
{"status": 404}
{"status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--exec",
            "e.class = e.status.status_class();",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should output 3 lines");

    let first: serde_json::Value =
        serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first["class"], "2xx");

    let second: serde_json::Value =
        serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(second["class"], "4xx");

    let third: serde_json::Value =
        serde_json::from_str(lines[2]).expect("Third line should be valid JSON");
    assert_eq!(third["class"], "5xx");
}

#[test]
fn test_complex_rhai_expressions() {
    let input = r#"{"user": "alice", "status": 404}
{"user": "bob", "status": 500}
{"user": "charlie", "status": 200}
{"user": "alice", "status": 403}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--filter",
            "e.status >= 400 && e.user.contains(\"a\")",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        2,
        "Should filter to 2 lines (alice with status >= 400)"
    );

    // Verify both results are alice with status >= 400
    for line in lines {
        let parsed: serde_json::Value =
            serde_json::from_str(line).expect("Line should be valid JSON");
        assert_eq!(parsed["user"], "alice");
        let status = parsed["status"].as_i64().unwrap();
        assert!(status >= 400);
    }
}

#[test]
fn test_print_function_output() {
    let input = r#"{"user": "alice", "level": "INFO"}
{"user": "bob", "level": "ERROR"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--exec",
            "print(\"Processing user: \" + e.user);",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    assert!(
        stdout.contains("Processing user: alice"),
        "Should print alice debug message"
    );
    assert!(
        stdout.contains("Processing user: bob"),
        "Should print bob debug message"
    );
    assert!(
        stdout.contains("\"user\":\"alice\""),
        "Should also output JSON data"
    );
}

#[test]
fn test_explicit_stdin_with_dash() {
    let input = r#"{"level": "info", "message": "test1"}
{"level": "error", "message": "test2"}
{"level": "info", "message": "test3"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "-"], input);

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("test1"));
    assert!(stdout.contains("test2"));
    assert!(stdout.contains("test3"));
}

#[test]
fn test_stdin_mixed_with_files() {
    // Create a temporary file
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(b"{\"level\": \"debug\", \"message\": \"from file\"}\n")
        .expect("Failed to write to temp file");

    let stdin_input = r#"{"level": "info", "message": "from stdin"}"#;

    // Test file first, then stdin
    let mut cmd = Command::new(if cfg!(debug_assertions) {
        "./target/debug/kelora"
    } else {
        "./target/release/kelora"
    })
    .args(&["-f", "jsonl", temp_file.path().to_str().unwrap(), "-"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("Failed to start kelora");

    if let Some(stdin) = cmd.stdin.as_mut() {
        stdin
            .write_all(stdin_input.as_bytes())
            .expect("Failed to write to stdin");
    }

    let output = cmd.wait_with_output().expect("Failed to read output");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("from file"));
    assert!(stdout.contains("from stdin"));
}

#[test]
fn test_multiple_stdin_rejected() {
    let (stdout, stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "-", "-"], "test");

    assert_ne!(exit_code, 0);
    assert!(stderr.contains("stdin (\"-\") can only be specified once"));
    assert!(stdout.is_empty());
}

#[test]
fn test_stdin_with_parallel_processing() {
    let input = r#"{"level": "info", "message": "test1"}
{"level": "error", "message": "test2"}
{"level": "info", "message": "test3"}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "--parallel", "-"], input);

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("test1"));
    assert!(stdout.contains("test2"));
    assert!(stdout.contains("test3"));
}

#[test]
fn test_stdin_large_input_performance() {
    // Generate 1000 log entries to test performance
    let mut large_input = String::new();
    for i in 1..=1000 {
        large_input.push_str(&format!(
            "{{\"user\":\"user{}\",\"status\":{},\"message\":\"Message {}\",\"id\":{}}}\n",
            i,
            200 + (i % 300),
            i,
            i
        ));
    }

    let start_time = std::time::Instant::now();
    let (stdout, _, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--filter",
            "e.status >= 400",
            "--exec",
            "track_count(\"errors\");",
            "--end",
            "print(`Errors: ${tracked[\"errors\"]}`);",
        ],
        &large_input,
    );
    let duration = start_time.elapsed();

    assert_eq!(
        exit_code, 0,
        "kelora should handle large input successfully"
    );
    assert!(
        stdout.contains("Errors:"),
        "Should count errors in large dataset"
    );

    // Performance check: should process 1000 lines in reasonable time
    assert!(
        duration.as_millis() < 5000,
        "Should process 1000 lines in less than 5 seconds, took {}ms",
        duration.as_millis()
    );
}

#[test]
fn test_error_handling_resilient_mixed_input() {
    let input = r#"{"valid": "json", "status": 200}
{malformed json line}
{"another": "valid", "status": 404}
not jsonl at all
{"final": "entry", "status": 500}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "-F", "jsonl"], input);
    assert_eq!(
        exit_code, 1,
        "kelora should exit with error code when errors occur, even in resilient mode"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        3,
        "Should output 3 valid JSON lines, skipping malformed ones"
    );

    // Verify all output lines are valid JSON
    for line in lines {
        serde_json::from_str::<serde_json::Value>(line)
            .expect("All output lines should be valid JSON");
    }
}

#[test]
fn test_error_handling_strict_mode() {
    let input = r#"{"level": "INFO", "status": 200}
invalid jsonl line
{"level": "ERROR", "status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "--strict"], input);
    assert_ne!(
        exit_code, 0,
        "kelora should exit with error code in strict mode when encountering invalid input"
    );

    // Should only output the first valid line before failing
    let lines: Vec<&str> = stdout
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty())
        .collect();
    assert!(
        lines.len() <= 1,
        "Should output at most one line before failing in strict mode"
    );
}

#[test]
fn test_tracking_with_min_max() {
    let input = r#"{"response_time": 150, "status": 200}
{"response_time": 500, "status": 404}
{"response_time": 75, "status": 200}
{"response_time": 800, "status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "track_min(\"min_time\", e.response_time); track_max(\"max_time\", e.response_time);",
            "--end",
            "print(`Min: ${tracked[\"min_time\"]}, Max: ${tracked[\"max_time\"]}`);",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    assert!(
        stdout.contains("Min: 75"),
        "Should track minimum response time"
    );
    assert!(
        stdout.contains("Max: 800"),
        "Should track maximum response time"
    );
}

#[test]
fn test_field_modification_and_addition() {
    let input = r#"{"user": "alice", "score": 85}
{"user": "bob", "score": 92}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--exec",
            "e.grade = if e.score >= 90 { \"A\" } else { \"B\" }; e.bonus_points = e.score * 0.1;",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines");

    let first: serde_json::Value =
        serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first["grade"], "B");
    assert_eq!(first["bonus_points"], 8.5);

    let second: serde_json::Value =
        serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(second["grade"], "A");
    assert_eq!(second["bonus_points"], 9.2);
}

#[test]
fn test_track_unique_function() {
    let input = r#"{"ip": "1.1.1.1", "user": "alice"}
{"ip": "2.2.2.2", "user": "bob"}
{"ip": "1.1.1.1", "user": "charlie"}
{"ip": "3.3.3.3", "user": "alice"}
{"ip": "2.2.2.2", "user": "dave"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--exec", "track_unique(\"unique_ips\", e.ip); track_unique(\"unique_users\", e.user);",
        "--end", "print(`IPs: ${tracked[\"unique_ips\"].len()}, Users: ${tracked[\"unique_users\"].len()}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Should collect 3 unique IPs and 4 unique users
    assert!(
        stdout.contains("IPs: 3"),
        "Should track 3 unique IP addresses"
    );
    assert!(stdout.contains("Users: 4"), "Should track 4 unique users");
}

#[test]
fn test_track_bucket_function() {
    let input = r#"{"status": "200", "method": "GET"}
{"status": "404", "method": "POST"}
{"status": "200", "method": "GET"}
{"status": "500", "method": "PUT"}
{"status": "404", "method": "GET"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--exec", "track_bucket(\"status_counts\", e.status); track_bucket(\"method_counts\", e.method);",
        "--end", "print(`Status 200: ${tracked[\"status_counts\"].get(\"200\") ?? 0}, GET requests: ${tracked[\"method_counts\"].get(\"GET\") ?? 0}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Should count 2 occurrences of status 200 and 3 GET requests
    assert!(
        stdout.contains("Status 200: 2"),
        "Should count 2 occurrences of status 200"
    );
    assert!(
        stdout.contains("GET requests: 3"),
        "Should count 3 GET requests"
    );
}

#[test]
fn test_track_unique_parallel_mode() {
    let input = r#"{"ip": "1.1.1.1"}
{"ip": "2.2.2.2"}
{"ip": "1.1.1.1"}
{"ip": "3.3.3.3"}
{"ip": "2.2.2.2"}
{"ip": "4.4.4.4"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--parallel",
            "--batch-size",
            "2",
            "--exec",
            "track_unique(\"ips\", e.ip);",
            "--end",
            "print(`Unique IPs: ${tracked[\"ips\"].len()}`);",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully in parallel mode"
    );

    // Should merge unique values from all workers
    assert!(
        stdout.contains("Unique IPs: 4"),
        "Should collect 4 unique IPs across parallel workers"
    );
}

#[test]
fn test_track_bucket_parallel_mode() {
    let input = r#"{"status": "200"}
{"status": "404"}
{"status": "200"}
{"status": "500"}
{"status": "404"}
{"status": "200"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--parallel",
        "--batch-size", "2",
        "--exec", "track_bucket(\"status_counts\", e.status);",
        "--end", "let counts = tracked[\"status_counts\"]; print(`200: ${counts.get(\"200\") ?? 0}, 404: ${counts.get(\"404\") ?? 0}, 500: ${counts.get(\"500\") ?? 0}`);"
    ], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully in parallel mode"
    );

    // Should merge bucket counts from all workers
    assert!(
        stdout.contains("200: 3"),
        "Should count 3 occurrences of status 200"
    );
    assert!(
        stdout.contains("404: 2"),
        "Should count 2 occurrences of status 404"
    );
    assert!(
        stdout.contains("500: 1"),
        "Should count 1 occurrence of status 500"
    );
}

#[test]
fn test_mixed_tracking_functions() {
    let input = r#"{"user": "alice", "response_time": 100, "status": "200"}
{"user": "bob", "response_time": 250, "status": "404"}
{"user": "alice", "response_time": 180, "status": "200"}
{"user": "charlie", "response_time": 50, "status": "500"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--exec", "track_count(\"total\"); track_unique(\"users\", e.user); track_bucket(\"status_dist\", e.status); track_min(\"min_time\", e.response_time); track_max(\"max_time\", e.response_time);",
        "--end", "print(`Total: ${tracked[\"total\"]}, Users: ${tracked[\"users\"].len()}, Min: ${tracked[\"min_time\"]}, Max: ${tracked[\"max_time\"]}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    assert!(stdout.contains("Total: 4"), "Should count 4 total records");
    assert!(stdout.contains("Users: 3"), "Should track 3 unique users");
    assert!(
        stdout.contains("Min: 50"),
        "Should track minimum response time"
    );
    assert!(
        stdout.contains("Max: 250"),
        "Should track maximum response time"
    );
}

#[test]
fn test_multiline_real_world_scenario() {
    let input = r#"{"timestamp": "2023-07-18T15:04:23.456Z", "user": "alice", "status": 200, "message": "login successful", "response_time": 45}
{"timestamp": "2023-07-18T15:04:25.789Z", "user": "bob", "status": 404, "message": "page not found", "response_time": 12}
{"timestamp": "2023-07-18T15:06:41.210Z", "user": "charlie", "status": 500, "message": "internal error", "response_time": 234}
{"timestamp": "2023-07-18T15:07:12.345Z", "user": "alice", "status": 403, "message": "forbidden", "response_time": 18}
{"timestamp": "2023-07-18T15:08:30.678Z", "user": "dave", "status": 200, "message": "success", "response_time": 67}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "-F", "jsonl",
        "--filter", "e.status >= 400",
        "--exec", "e.alert_level = if e.status >= 500 { \"critical\" } else { \"warning\" }; track_count(\"total_errors\");",
        "--end", "print(`Total errors processed: ${tracked[\"total_errors\"]}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout
        .trim()
        .lines()
        .filter(|line| line.starts_with('{'))
        .collect();
    assert_eq!(lines.len(), 3, "Should filter to 3 error lines");

    assert!(
        stdout.contains("Total errors processed: 3"),
        "Should count all error lines"
    );

    // Verify alert levels are correctly assigned
    for line in lines {
        let parsed: serde_json::Value =
            serde_json::from_str(line).expect("Line should be valid JSON");
        let status = parsed["status"].as_i64().unwrap();
        let alert_level = parsed["alert_level"].as_str().unwrap();

        if status >= 500 {
            assert_eq!(alert_level, "critical");
        } else {
            assert_eq!(alert_level, "warning");
        }
    }
}

#[test]
fn test_skip_lines_functionality() {
    // Test with headers in CSV-style data
    let input = r#"header1,header2,header3
description,more info,extra
alice,user,200
bob,admin,404
charlie,guest,500"#;

    // Test skipping first 2 lines (headers)
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "--skip-lines",
            "2",
            "--filter",
            "line.contains(\"user\") || line.contains(\"admin\")",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "Should have 2 lines after skipping headers and filtering"
    );
    assert!(
        stdout.contains("alice,user,200"),
        "Should contain alice line"
    );
    assert!(stdout.contains("bob,admin,404"), "Should contain bob line");
    assert!(!stdout.contains("header1"), "Should not contain header1");
    assert!(
        !stdout.contains("description"),
        "Should not contain description line"
    );

    // Test with parallel processing
    let (stdout_parallel, _stderr_parallel, exit_code_parallel) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "--skip-lines",
            "2",
            "--parallel",
            "--filter",
            "line.contains(\"user\") || line.contains(\"admin\")",
        ],
        input,
    );
    assert_eq!(
        exit_code_parallel, 0,
        "kelora should exit successfully in parallel mode"
    );

    let lines_parallel: Vec<&str> = stdout_parallel.trim().lines().collect();
    assert_eq!(
        lines_parallel.len(),
        2,
        "Parallel processing should give same result"
    );
}

#[test]
fn test_skip_lines_with_zero() {
    let input = r#"line1
line2
line3"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "line", "--skip-lines", "0"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(
        lines.len(),
        3,
        "Should process all lines when skip-lines is 0"
    );
}

#[test]
fn test_skip_lines_greater_than_input() {
    let input = r#"line1
line2"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "line", "--skip-lines", "5"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        0,
        "Should produce no output when skipping more lines than available"
    );
}

#[test]
fn test_syslog_rfc5424_parsing() {
    let input = r#"<165>1 2023-10-11T22:14:15.003Z server01 sshd 1234 ID47 - Failed password for user alice
<33>1 2023-10-11T22:14:16.123Z server01 nginx 5678 - - Request processed successfully"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "syslog", "-F", "jsonl"], input);
    assert_eq!(exit_code, 0, "syslog parsing should succeed");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should parse 2 syslog lines");

    // Check first line (SSH failure)
    let first_line: serde_json::Value =
        serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["pri"].as_i64().unwrap(), 165);
    assert_eq!(first_line["facility"].as_i64().unwrap(), 20); // 165 >> 3
    assert_eq!(first_line["severity"].as_i64().unwrap(), 5); // 165 & 7
    assert_eq!(first_line["host"].as_str().unwrap(), "server01");
    assert_eq!(first_line["prog"].as_str().unwrap(), "sshd");
    assert_eq!(first_line["pid"].as_i64().unwrap(), 1234);
    assert_eq!(
        first_line["msg"].as_str().unwrap(),
        "Failed password for user alice"
    );

    // Check second line (nginx success)
    let second_line: serde_json::Value =
        serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(second_line["pri"].as_i64().unwrap(), 33);
    assert_eq!(second_line["facility"].as_i64().unwrap(), 4); // 33 >> 3
    assert_eq!(second_line["severity"].as_i64().unwrap(), 1); // 33 & 7
    assert_eq!(second_line["prog"].as_str().unwrap(), "nginx");
    assert_eq!(second_line["pid"].as_i64().unwrap(), 5678);
}

#[test]
fn test_syslog_rfc3164_parsing() {
    let input = r#"Oct 11 22:14:15 server01 sshd[1234]: Failed password for user bob
Oct 11 22:14:16 server01 kernel: CPU0: Core temperature above threshold"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "syslog", "-F", "jsonl"], input);
    assert_eq!(exit_code, 0, "syslog parsing should succeed");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should parse 2 syslog lines");

    // Check first line (with PID)
    let first_line: serde_json::Value =
        serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["timestamp"].as_str().unwrap(), "Oct 11 22:14:15");
    assert_eq!(first_line["host"].as_str().unwrap(), "server01");
    assert_eq!(first_line["prog"].as_str().unwrap(), "sshd");
    assert_eq!(first_line["pid"].as_i64().unwrap(), 1234);
    assert_eq!(
        first_line["msg"].as_str().unwrap(),
        "Failed password for user bob"
    );

    // Check second line (no PID)
    let second_line: serde_json::Value =
        serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(
        second_line["timestamp"].as_str().unwrap(),
        "Oct 11 22:14:16"
    );
    assert_eq!(second_line["host"].as_str().unwrap(), "server01");
    assert_eq!(second_line["prog"].as_str().unwrap(), "kernel");
    assert_eq!(second_line["pid"], serde_json::Value::Null); // No PID for kernel messages
    assert_eq!(
        second_line["msg"].as_str().unwrap(),
        "CPU0: Core temperature above threshold"
    );
}

#[test]
fn test_syslog_filtering_and_analysis() {
    let input = r#"<165>1 2023-10-11T22:14:15.003Z server01 sshd 1234 ID47 - Failed password for user alice
<86>1 2023-10-11T22:14:16.456Z server01 postfix 9012 - - NOQUEUE: reject: RCPT from unknown
<33>1 2023-10-11T22:14:17.123Z server01 nginx 5678 - - Request processed successfully
Oct 11 22:14:18 server01 sshd[1234]: Failed password for user bob
Oct 11 22:14:19 server01 kernel: CPU0: Core temperature above threshold"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "syslog",
        "--filter", "e.msg.matches(\"Failed|reject\")",
        "--exec", "track_count(\"errors\"); track_unique(\"programs\", e.prog);",
        "--end", "print(`Total errors: ${tracked[\"errors\"]}, Programs: ${tracked[\"programs\"].len()}`);"
    ], input);
    assert_eq!(exit_code, 0, "syslog filtering should succeed");

    // Should find 3 error messages (2 failed passwords, 1 postfix reject)
    assert!(
        stdout.contains("Total errors: 3"),
        "Should count 3 error messages"
    );
    assert!(
        stdout.contains("Programs: 2"),
        "Should identify 2 different programs (sshd, postfix)"
    );
}

#[test]
fn test_syslog_severity_analysis() {
    let input = r#"<165>1 2023-10-11T22:14:15.003Z server01 sshd 1234 ID47 - Failed password for user alice
<30>1 2023-10-11T22:14:16.123Z server01 nginx 5678 - - Request processed successfully
<11>1 2023-10-11T22:14:17.456Z server01 postgres 2345 - - Database connection established"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "syslog",
        "--exec", "e.sev_name = if e.severity == 5 { \"notice\" } else if e.severity == 6 { \"info\" } else if e.severity == 3 { \"error\" } else { \"other\" }; track_bucket(\"severities\", e.sev_name);",
        "--end", "let counts = tracked[\"severities\"]; print(`notice: ${counts.get(\"notice\") ?? 0}, info: ${counts.get(\"info\") ?? 0}, error: ${counts.get(\"error\") ?? 0}`);"
    ], input);
    assert_eq!(exit_code, 0, "syslog severity analysis should succeed");

    // Verify severity distribution
    // 165 & 7 = 5 (notice), 30 & 7 = 6 (info), 11 & 7 = 3 (error)
    assert!(stdout.contains("notice: 1"), "Should have 1 notice message");
    assert!(stdout.contains("info: 1"), "Should have 1 info message");
    assert!(stdout.contains("error: 1"), "Should have 1 error message");
}

#[test]
fn test_syslog_with_file() {
    let syslog_content = std::fs::read_to_string("test_data/sample.syslog")
        .expect("Should be able to read sample syslog file");

    let (stdout, _stderr, exit_code) = run_kelora_with_file(
        &[
            "-f",
            "syslog",
            "--filter",
            "e.host == \"webserver\"",
            "-F",
            "jsonl",
        ],
        &syslog_content,
    );
    assert_eq!(exit_code, 0, "syslog file processing should succeed");

    // Should only show entries from webserver host
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        if line.starts_with('{') {
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("Should be valid JSON");
            assert_eq!(parsed["host"].as_str().unwrap(), "webserver");
        }
    }
}

#[test]
fn test_apache_combined_format_parsing() {
    let input = r#"192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234 "http://www.example.com/" "Mozilla/4.08"
127.0.0.1 - - [25/Dec/1995:10:00:01 +0000] "POST /api/data HTTP/1.1" 201 456 "-" "curl/7.68.0"
10.0.0.1 - admin [25/Dec/1995:10:00:02 +0000] "GET /admin/dashboard HTTP/1.1" 403 - "https://admin.example.com/" "Mozilla/5.0""#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "apache", "-F", "jsonl"], input);
    assert_eq!(exit_code, 0, "Apache parsing should succeed");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 3, "Should parse 3 Apache log lines");

    // Check first line (Combined format with all fields)
    let first_line: serde_json::Value =
        serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["ip"].as_str().unwrap(), "192.168.1.1");
    assert_eq!(first_line["user"].as_str().unwrap(), "user");
    assert_eq!(first_line["method"].as_str().unwrap(), "GET");
    assert_eq!(first_line["path"].as_str().unwrap(), "/index.html");
    assert_eq!(first_line["protocol"].as_str().unwrap(), "HTTP/1.0");
    assert_eq!(first_line["status"].as_i64().unwrap(), 200);
    assert_eq!(first_line["bytes"].as_i64().unwrap(), 1234);
    assert_eq!(
        first_line["referer"].as_str().unwrap(),
        "http://www.example.com/"
    );
    assert_eq!(first_line["user_agent"].as_str().unwrap(), "Mozilla/4.08");

    // Check second line (POST with dashes for user)
    let second_line: serde_json::Value =
        serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(second_line["ip"].as_str().unwrap(), "127.0.0.1");
    assert!(second_line.get("user").is_none()); // Should be null for "-"
    assert_eq!(second_line["method"].as_str().unwrap(), "POST");
    assert_eq!(second_line["path"].as_str().unwrap(), "/api/data");
    assert_eq!(second_line["status"].as_i64().unwrap(), 201);
    assert_eq!(second_line["user_agent"].as_str().unwrap(), "curl/7.68.0");

    // Check third line (403 error with no bytes)
    let third_line: serde_json::Value =
        serde_json::from_str(lines[2]).expect("Third line should be valid JSON");
    assert_eq!(third_line["status"].as_i64().unwrap(), 403);
    assert!(third_line.get("bytes").is_none()); // Should be null for "-"
}

#[test]
fn test_apache_common_format_parsing() {
    let input = r#"192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234
127.0.0.1 - - [25/Dec/1995:10:00:01 +0000] "POST /api/data HTTP/1.1" 201 456"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "apache", "-F", "jsonl"], input);
    assert_eq!(exit_code, 0, "Apache common format parsing should succeed");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should parse 2 Apache common log lines");

    // Check that referer and user_agent fields are not present (common format)
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).expect("Should be valid JSON");
        assert!(
            parsed.get("referer").is_none(),
            "Common format should not have referer"
        );
        assert!(
            parsed.get("user_agent").is_none(),
            "Common format should not have user_agent"
        );
        assert!(parsed.get("ip").is_some(), "Should have IP address");
        assert!(parsed.get("method").is_some(), "Should have HTTP method");
        assert!(parsed.get("status").is_some(), "Should have status code");
    }
}

#[test]
fn test_apache_filtering_and_analysis() {
    let input = r#"192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234 "http://www.example.com/" "Mozilla/4.08"
127.0.0.1 - - [25/Dec/1995:10:00:01 +0000] "POST /api/data HTTP/1.1" 404 0 "-" "curl/7.68.0"
10.0.0.1 - admin [25/Dec/1995:10:00:02 +0000] "GET /admin/dashboard HTTP/1.1" 403 - "https://admin.example.com/" "Mozilla/5.0"
192.168.1.50 - - [25/Dec/1995:10:00:03 +0000] "GET /favicon.ico HTTP/1.1" 500 1024 "http://www.site.com/" "Safari/537.36""#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "apache",
        "--filter", "e.status >= 400",
        "--exec", "track_count(\"errors\"); track_bucket(\"methods\", e.method);",
        "--end", "let methods = tracked[\"methods\"]; print(`Total errors: ${tracked[\"errors\"]}, GET: ${methods.get(\"GET\") ?? 0}, POST: ${methods.get(\"POST\") ?? 0}`);"
    ], input);
    assert_eq!(exit_code, 0, "Apache filtering should succeed");

    // Should find 3 error responses (404, 403, 500)
    assert!(
        stdout.contains("Total errors: 3"),
        "Should count 3 error responses"
    );
    assert!(stdout.contains("GET: 2"), "Should have 2 GET errors");
    assert!(stdout.contains("POST: 1"), "Should have 1 POST error");
}

#[test]
fn test_apache_status_code_analysis() {
    let input = r#"192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] "GET /index.html HTTP/1.0" 200 1234 "http://www.example.com/" "Mozilla/4.08"
127.0.0.1 - - [25/Dec/1995:10:00:01 +0000] "POST /api/data HTTP/1.1" 201 456 "-" "curl/7.68.0"
10.0.0.1 - admin [25/Dec/1995:10:00:02 +0000] "GET /admin/dashboard HTTP/1.1" 403 - "https://admin.example.com/" "Mozilla/5.0"
192.168.1.50 - - [25/Dec/1995:10:00:03 +0000] "GET /favicon.ico HTTP/1.1" 500 1024 "http://www.site.com/" "Safari/537.36""#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "apache",
        "--exec", "e.class = if e.status < 300 { \"2xx\" } else if e.status < 400 { \"3xx\" } else if e.status < 500 { \"4xx\" } else { \"5xx\" }; track_bucket(\"status_classes\", e.class);",
        "--end", "let classes = tracked[\"status_classes\"]; print(`2xx: ${classes.get(\"2xx\") ?? 0}, 4xx: ${classes.get(\"4xx\") ?? 0}, 5xx: ${classes.get(\"5xx\") ?? 0}`);"
    ], input);
    assert_eq!(exit_code, 0, "Apache status code analysis should succeed");

    // Verify status code distribution: 200, 201 (2xx), 403 (4xx), 500 (5xx)
    assert!(stdout.contains("2xx: 2"), "Should have 2 success responses");
    assert!(stdout.contains("4xx: 1"), "Should have 1 client error");
    assert!(stdout.contains("5xx: 1"), "Should have 1 server error");
}

#[test]
fn test_brief_output_mode() {
    let input = r#"{"level": "INFO", "message": "test message", "user": "alice"}
{"level": "ERROR", "message": "error occurred", "user": "bob"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "--brief"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with brief mode"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines");

    // Brief mode should output only values, space-separated, no keys
    assert_eq!(lines[0], "INFO test message alice");
    assert_eq!(lines[1], "ERROR error occurred bob");

    // Verify no key=value format is used
    assert!(
        !stdout.contains("level="),
        "Brief mode should not contain keys"
    );
    assert!(
        !stdout.contains("message="),
        "Brief mode should not contain keys"
    );
    assert!(
        !stdout.contains("user="),
        "Brief mode should not contain keys"
    );
}

#[test]
fn test_brief_output_mode_short_form() {
    let input = r#"{"level": "INFO", "message": "hello world"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "-b"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with -b short form"
    );

    // Brief mode should output only values, space-separated
    assert_eq!(stdout.trim(), "INFO hello world");
    assert!(
        !stdout.contains("level="),
        "Brief mode should not contain keys"
    );
}

#[test]
fn test_core_field_filtering() {
    let input = r#"{"timestamp": "2024-01-01T12:00:00Z", "level": "ERROR", "message": "Test message", "user": "alice", "status": 500}"#;

    let (stdout, _, exit_code) = run_kelora_with_input(&["-f", "jsonl", "--core"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully with --core");

    // Should only contain core fields
    assert!(
        stdout.contains("timestamp="),
        "Should contain timestamp field"
    );
    assert!(stdout.contains("level="), "Should contain level field");
    assert!(stdout.contains("message="), "Should contain message field");
    assert!(
        !stdout.contains("user="),
        "Should not contain non-core user field"
    );
    assert!(
        !stdout.contains("status="),
        "Should not contain non-core status field"
    );
}

#[test]
fn test_core_field_filtering_short_flag() {
    let input = r#"{"timestamp": "2024-01-01T12:00:00Z", "level": "ERROR", "message": "Test message", "user": "alice"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "-c"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with -c short flag"
    );

    // Should only contain core fields
    assert!(
        stdout.contains("timestamp="),
        "Should contain timestamp field"
    );
    assert!(stdout.contains("level="), "Should contain level field");
    assert!(stdout.contains("message="), "Should contain message field");
    assert!(
        !stdout.contains("user="),
        "Should not contain non-core user field"
    );
}

#[test]
fn test_core_field_with_alternative_names() {
    let input = r#"{"ts": "2024-01-01T12:00:00Z", "lvl": "WARN", "msg": "Alternative names", "extra_data": "ignored"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "--core"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with alternative core field names"
    );

    // Should include alternative core field names
    assert!(stdout.contains("ts="), "Should contain ts field");
    assert!(stdout.contains("lvl="), "Should contain lvl field");
    assert!(stdout.contains("msg="), "Should contain msg field");
    assert!(
        !stdout.contains("extra_data="),
        "Should not contain non-core field"
    );
}

#[test]
fn test_core_field_plus_additional_keys() {
    let input = r#"{"timestamp": "2024-01-01T12:00:00Z", "level": "ERROR", "message": "Test message", "user": "alice", "status": 500}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "--core", "--keys", "user,status"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --core and --keys"
    );

    // Should contain both core fields and specified keys
    assert!(
        stdout.contains("timestamp="),
        "Should contain timestamp field"
    );
    assert!(stdout.contains("level="), "Should contain level field");
    assert!(stdout.contains("message="), "Should contain message field");
    assert!(
        stdout.contains("user="),
        "Should contain user field from --keys"
    );
    assert!(
        stdout.contains("status="),
        "Should contain status field from --keys"
    );
}

#[test]
fn test_core_field_with_syslog() {
    let input = r#"<34>Jan 1 12:00:00 myhost myapp[1234]: Test syslog message"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "syslog", "--core"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with syslog and --core"
    );

    // Should contain syslog core fields
    assert!(
        stdout.contains("severity="),
        "Should contain severity field"
    );
    assert!(
        stdout.contains("timestamp="),
        "Should contain timestamp field"
    );
    assert!(stdout.contains("msg="), "Should contain msg field");
    // Should not contain non-core syslog fields
    assert!(
        !stdout.contains("facility="),
        "Should not contain facility field"
    );
    assert!(!stdout.contains("host="), "Should not contain host field");
    assert!(!stdout.contains("prog="), "Should not contain prog field");
}

#[test]
fn test_core_field_with_exec_created_fields() {
    let input = r#"{"original_time": "2024-01-01T12:00:00Z", "orig_level": "ERROR", "orig_msg": "Test message", "user": "alice"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "e.timestamp = e.original_time; e.level = e.orig_level; e.message = e.orig_msg",
            "--core",
            "--keys",
            "user",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with exec-created core fields"
    );

    // Should contain exec-created core fields and specified keys
    assert!(
        stdout.contains("user="),
        "Should contain user field from --keys"
    );
    assert!(
        stdout.contains("timestamp="),
        "Should contain exec-created timestamp field"
    );
    assert!(
        stdout.contains("level="),
        "Should contain exec-created level field"
    );
    assert!(
        stdout.contains("message="),
        "Should contain exec-created message field"
    );
    // Should not contain original fields
    assert!(
        !stdout.contains("original_time="),
        "Should not contain original_time field"
    );
    assert!(
        !stdout.contains("orig_level="),
        "Should not contain orig_level field"
    );
    assert!(
        !stdout.contains("orig_msg="),
        "Should not contain orig_msg field"
    );
}

#[test]
fn test_core_field_with_logfmt() {
    let input =
        r#"time=2024-01-01T12:00:00Z lvl=error msg="Test logfmt message" user=bob status=404"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "logfmt", "--core"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with logfmt and --core"
    );

    // Should contain logfmt core fields
    assert!(stdout.contains("time="), "Should contain time field");
    assert!(stdout.contains("lvl="), "Should contain lvl field");
    assert!(stdout.contains("msg="), "Should contain msg field");
    // Should not contain non-core fields
    assert!(!stdout.contains("user="), "Should not contain user field");
    assert!(
        !stdout.contains("status="),
        "Should not contain status field"
    );
}

#[test]
fn test_core_field_multiple_timestamp_variants() {
    let input = r#"{"ts": "2024-01-01T12:00:00Z", "timestamp": "2024-01-01T13:00:00Z", "time": "2024-01-01T14:00:00Z", "level": "INFO", "message": "Multiple timestamps", "other": "data"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "--core"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with multiple timestamp variants"
    );

    // Should include all timestamp field variants (current behavior: include all matching names)
    assert!(stdout.contains("ts="), "Should contain ts field");
    assert!(
        stdout.contains("timestamp="),
        "Should contain timestamp field"
    );
    assert!(stdout.contains("time="), "Should contain time field");
    assert!(stdout.contains("level="), "Should contain level field");
    assert!(stdout.contains("message="), "Should contain message field");
    assert!(
        !stdout.contains("other="),
        "Should not contain non-core other field"
    );
}

#[test]
fn test_ordered_filter_exec_stages() {
    // Test that filter and exec stages execute in the exact CLI order
    let input = r#"{"status": "200", "message": "OK"}"#;

    // Test correct order: exec (convert) -> filter -> filter -> exec (add field)
    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "e.status=e.status.to_int()",
            "--filter",
            "e.status > 100",
            "--filter",
            "e.status < 400",
            "--exec",
            "e.level=\"info\"",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert_eq!(stderr, "");
    assert!(stdout.contains("status=200"));
    assert!(stdout.contains("level=\"info\""));

    // Test wrong order: filter before conversion should fail
    let (stdout2, _stderr2, _exit_code2) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--filter",
            "e.status > 100", // This will fail on string "200"
            "--exec",
            "e.status=e.status.to_int()",
            "--filter",
            "e.status < 400",
            "--exec",
            "e.level=\"info\"",
        ],
        input,
    );

    // Should produce no output because string "200" > 100 comparison doesn't work as expected
    assert!(stdout2.trim().is_empty());
}

#[test]
fn test_complex_ordered_pipeline() {
    // Test a more complex pipeline with transformations and filtering
    let input = r#"{"value": 5}
{"value": 15}
{"value": 25}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "e.doubled = e.value * 2",
            "--filter",
            "e.doubled > 20",
            "--exec",
            "e.status = if e.doubled > 30 { \"high\" } else { \"medium\" }",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert_eq!(stderr, "");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2); // Should filter out value=5 (doubled=10)

    // Check first line (value=15, doubled=30, status="medium")
    assert!(lines[0].contains("value=15"));
    assert!(lines[0].contains("doubled=30"));
    assert!(lines[0].contains("status=\"medium\""));

    // Check second line (value=25, doubled=50, status="high")
    assert!(lines[1].contains("value=25"));
    assert!(lines[1].contains("doubled=50"));
    assert!(lines[1].contains("status=\"high\""));
}

// Regression tests for parallel mode statistics counting (GitHub issue #XXX)
// TODO: Update test for new stats format
// #[test]
// fn test_parallel_stats_counting_basic() {
//     // Generate test data: 1-100, expect 10 outputs (multiples of 10), 90 filtered
//     let input: String = (1..=100)
//         .map(|i| i.to_string())
//         .collect::<Vec<_>>()
//         .join("\n");
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "--stats",
//             "--filter",
//             "line.to_int() % 10 == 0",
//             "--parallel",
//         ],
//         &input,
//     );
//
//     assert_eq!(exit_code, 0, "kelora should exit successfully");
//
//     // Check output lines
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(
//         output_lines.len(),
//         10,
//         "Should output exactly 10 lines (multiples of 10)"
//     );
//
//     // Verify the output lines are correct
//     let expected_outputs = ["10", "20", "30", "40", "50", "60", "70", "80", "90", "100"];
//     for (i, line) in output_lines.iter().enumerate() {
//         assert_eq!(line.trim(), &format!("line=\"{}\"", expected_outputs[i]));
//     }
//
//     // Check statistics in stderr
//     assert!(
//         stderr.contains("100 total"),
//         "Should show 100 total lines processed"
//     );
//     assert!(stderr.contains("10 output"), "Should show 10 output lines");
//     assert!(
//         stderr.contains("90 filtered"),
//         "Should show 90 filtered lines"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_parallel_stats_counting_large_dataset() {
//     // Generate test data: 1-10000, expect 1000 outputs (multiples of 10), 9000 filtered
//     let input: String = (1..=10000)
//         .map(|i| i.to_string())
//         .collect::<Vec<_>>()
//         .join("\n");
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "--stats",
//             "--filter",
//             "line.to_int() % 10 == 0",
//             "--parallel",
//             "--batch-size",
//             "100", // Smaller batch size to test multiple batches
//         ],
//         &input,
//     );
//
//     assert_eq!(exit_code, 0, "kelora should exit successfully");
//
//     // Check output count
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 1000, "Should output exactly 1000 lines");
//
//     // Check statistics in stderr
//     assert!(
//         stderr.contains("10000 total"),
//         "Should show 10000 total lines processed"
//     );
//     assert!(
//         stderr.contains("1000 output"),
//         "Should show 1000 output lines"
//     );
//     assert!(
//         stderr.contains("9000 filtered"),
//         "Should show 9000 filtered lines"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_parallel_vs_sequential_stats_consistency() {
//     // Test that parallel and sequential modes produce identical statistics
//     let input: String = (1..=1000)
//         .map(|i| i.to_string())
//         .collect::<Vec<_>>()
//         .join("\n");
//
//     // Run in sequential mode
//     let (stdout_seq, stderr_seq, exit_code_seq) =
//         run_kelora_with_input(&["--stats", "--filter", "line.to_int() % 100 == 0"], &input);
//
//     // Run in parallel mode
//     let (stdout_par, stderr_par, exit_code_par) = run_kelora_with_input(
//         &[
//             "--stats",
//             "--filter",
//             "line.to_int() % 100 == 0",
//             "--parallel",
//             "--batch-size",
//             "50",
//         ],
//         &input,
//     );
//
//     assert_eq!(exit_code_seq, 0, "Sequential mode should exit successfully");
//     assert_eq!(exit_code_par, 0, "Parallel mode should exit successfully");
//
//     // Both should produce the same output
//     assert_eq!(
//         stdout_seq, stdout_par,
//         "Sequential and parallel modes should produce identical output"
//     );
//
//     // Both should show the same statistics: 1000 total, 10 output, 990 filtered
//     let expected_stats = ["1000 total", "10 output", "990 filtered"];
//     for stat in &expected_stats {
//         assert!(
//             stderr_seq.contains(stat),
//             "Sequential mode should contain: {}",
//             stat
//         );
//         assert!(
//             stderr_par.contains(stat),
//             "Parallel mode should contain: {}",
//             stat
//         );
//     }
// }

// TODO: Update test for new stats format
// #[test]
// fn test_parallel_stats_with_errors() {
//     // Test statistics counting when errors occur during processing
//     let input = "1\n2\ninvalid\n4\n5\n";
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "--stats",
//             "--filter",
//             "line.to_int() > 3", // This will cause an error on "invalid"
//             "--on-error",
//             "skip", // Skip errors and continue
//             "--parallel",
//         ],
//         input,
//     );
//
//     assert_eq!(exit_code, 0, "kelora should exit successfully");
//
//     // Should output lines "4" and "5" (> 3)
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(
//         output_lines.len(),
//         2,
//         "Should output 2 lines that pass filter"
//     );
//
//     // Check statistics - total should be 5, output 2, filtered 3 (including error), errors 0 (when on-error=skip)
//     assert!(
//         stderr.contains("5 total"),
//         "Should show 5 total lines processed"
//     );
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(
//         stderr.contains("3 filtered"),
//         "Should show 3 filtered lines (including error with skip)"
//     );
//     // Note: when --on-error skip is used, errors are counted as filtered, not as separate errors
// }

// TODO: Update test for new stats format
// #[test]
// fn test_parallel_stats_with_different_batch_sizes() {
//     // Test that different batch sizes produce the same statistics
//     let input: String = (1..=500)
//         .map(|i| i.to_string())
//         .collect::<Vec<_>>()
//         .join("\n");
//
//     let batch_sizes = [1, 10, 50, 100, 500];
//     let mut all_results = Vec::new();
//
//     for &batch_size in &batch_sizes {
//         let (stdout, stderr, exit_code) = run_kelora_with_input(
//             &[
//                 "--stats",
//                 "--filter",
//                 "line.to_int() % 50 == 0",
//                 "--parallel",
//                 "--batch-size",
//                 &batch_size.to_string(),
//             ],
//             &input,
//         );
//
//         assert_eq!(
//             exit_code, 0,
//             "kelora should exit successfully with batch-size {}",
//             batch_size
//         );
//         all_results.push((stdout, stderr));
//     }
//
//     // All results should be identical
//     let (first_stdout, first_stderr) = &all_results[0];
//     for (i, (stdout, stderr)) in all_results.iter().enumerate().skip(1) {
//         assert_eq!(
//             stdout, first_stdout,
//             "Batch size {} should produce same output as batch size {}",
//             batch_sizes[i], batch_sizes[0]
//         );
//
//         // Check that statistics are the same (ignore timing differences)
//         let expected_stats = ["500 total", "10 output", "490 filtered"];
//         for stat in &expected_stats {
//             assert!(
//                 first_stderr.contains(stat),
//                 "Batch size {} should contain: {}",
//                 batch_sizes[0],
//                 stat
//             );
//             assert!(
//                 stderr.contains(stat),
//                 "Batch size {} should contain: {}",
//                 batch_sizes[i],
//                 stat
//             );
//         }
//     }
// }

#[test]
fn test_ignore_lines_functionality() {
    let input = r#"{"level": "INFO", "message": "This is an info message"}
# This is a comment line
{"level": "ERROR", "message": "This is an error message"}

{"level": "DEBUG", "message": "This is a debug message"}
# Another comment
{"level": "WARN", "message": "This is a warning"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--ignore-lines",
            "^#.*|^$", // Ignore comments and empty lines
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with ignore-lines"
    );

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(
        lines.len(),
        4,
        "Should output 4 lines (comments and empty lines ignored)"
    );

    // Verify all lines are valid JSON (no comments or empty lines)
    for line in lines {
        let parsed: serde_json::Value =
            serde_json::from_str(line).expect("Line should be valid JSON");
        assert!(parsed.is_object(), "Each line should be a JSON object");
    }
}

#[test]
fn test_ignore_lines_with_specific_pattern() {
    let input = r#"{"level": "INFO", "message": "User login successful"}
{"level": "DEBUG", "message": "systemd startup complete"}
{"level": "ERROR", "message": "Failed to connect to database"}
{"level": "DEBUG", "message": "systemd service started"}
{"level": "WARN", "message": "High memory usage detected"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "-F",
            "jsonl",
            "--ignore-lines",
            "systemd", // Ignore lines containing "systemd"
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with ignore-lines pattern"
    );

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(
        lines.len(),
        3,
        "Should output 3 lines (systemd lines ignored)"
    );

    // Verify systemd lines are not present
    for line in lines {
        assert!(
            !line.contains("systemd"),
            "Output should not contain systemd lines"
        );
    }
}

// TODO: Update test for new stats format
// #[test]
// fn test_ignore_lines_with_stats() {
//     let input = r#"{"level": "INFO", "message": "Valid message 1"}
// # Comment to ignore
// {"level": "ERROR", "message": "Valid message 2"}
// # Another comment
// {"level": "WARN", "message": "Valid message 3"}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "-f",
//             "jsonl",
//             "-F",
//             "jsonl",
//             "--ignore-lines",
//             "^#", // Ignore comment lines
//             "--stats",
//         ],
//         input,
//     );
//     assert_eq!(
//         exit_code, 0,
//         "kelora should exit successfully with ignore-lines and stats"
//     );
//
//     let lines: Vec<&str> = stdout.trim().lines().collect();
//     assert_eq!(lines.len(), 3, "Should output 3 lines (comments ignored)");
//
//     // Check stats show filtered lines
//     assert!(stderr.contains("5 total"), "Should show 5 total lines read");
//     assert!(
//         stderr.contains("2 filtered"),
//         "Should show 2 lines filtered by ignore-lines"
//     );
//     assert!(stderr.contains("3 output"), "Should show 3 lines output");
// }

#[test]
fn test_get_path_function_basic_usage() {
    let input = r#"{"user": {"name": "alice", "age": 25, "scores": [100, 200, 300]}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "let name = get_path(e.user, \"name\", \"unknown\"); print(\"Name: \" + name)",
        ],
        input,
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Name: alice"),
        "Should extract nested value: {}",
        stdout
    );
}

#[test]
fn test_get_path_function_array_access() {
    let input = r#"{"user": {"name": "bob", "scores": [100, 200, 300]}}"#;

    let (stdout, _, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "let score = get_path(e.user, \"scores[1]\", 0); print(\"Second score: \" + score)",
        ],
        input,
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Second score: 200"),
        "Should access array element: {}",
        stdout
    );
}

#[test]
fn test_get_path_function_negative_indexing() {
    let input = r#"{"user": {"name": "charlie", "scores": [100, 200, 300]}}"#;

    let (stdout, _, exit_code) = run_kelora_with_input(
        &["-f", "jsonl", "--exec", "let last_score = get_path(e.user, \"scores[-1]\", 0); print(\"Last score: \" + last_score)"],
        input,
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Last score: 300"),
        "Should access last array element: {}",
        stdout
    );
}

#[test]
fn test_get_path_function_deeply_nested() {
    let input = r#"{"data": {"items": [{"id": 1, "meta": {"tags": ["urgent", "review"]}}]}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "jsonl", "--exec", "let tag = get_path(e.data, \"items[0].meta.tags[0]\", \"none\"); print(\"First tag: \" + tag)"],
        input,
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("First tag: urgent"),
        "Should extract deeply nested value: {}",
        stdout
    );
}

#[test]
fn test_get_path_function_with_default() {
    let input = r#"{"user": {"name": "david"}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "let age = get_path(e.user, \"age\", \"unknown\"); print(\"Age: \" + age)",
        ],
        input,
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Age: unknown"),
        "Should use default for missing key: {}",
        stdout
    );
}

#[test]
fn test_get_path_function_invalid_array_index() {
    let input = r#"{"user": {"scores": [100, 200]}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "let score = get_path(e.user, \"scores[99]\", \"not_found\"); print(\"Score: \" + score)",
        ],
        input,
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Score: not_found"),
        "Should use default for invalid index: {}",
        stdout
    );
}

#[test]
fn test_get_path_function_filtering() {
    let input = r#"{"level": "error", "user": {"role": "admin"}}
{"level": "info", "user": {"role": "user"}}
{"level": "error", "user": {"role": "user"}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--filter",
            "get_path(e.user, \"role\") == \"admin\"",
        ],
        input,
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        1,
        "Should filter to one admin entry: {}",
        stdout
    );
    assert!(
        lines[0].contains("admin"),
        "Should contain admin role: {}",
        stdout
    );
}

#[test]
fn test_get_path_function_with_real_world_log() {
    let input = r#"{"timestamp": "2023-01-01T10:00:00Z", "request": {"method": "GET", "url": "/api/users", "headers": {"user-agent": "Mozilla/5.0"}}, "response": {"status": 200, "size": 1024}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "let method = get_path(e.request, \"method\", \"unknown\"); \
           let status = get_path(e.response, \"status\", 0); \
           let user_agent = get_path(e.request, \"headers.user-agent\", \"unknown\"); \
           print(method + \" \" + status + \" \" + user_agent)",
        ],
        input,
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("GET 200 Mozilla/5.0"),
        "Should extract multiple nested values: {}",
        stdout
    );
}

#[test]
fn test_filename_tracking_jsonl_sequential() {
    // Test filename tracking with JSONL format in sequential mode
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1
        .write_all(b"{\"message\": \"test1\"}\n")
        .expect("Failed to write to temp file");
    temp_file2
        .write_all(b"{\"message\": \"test2\"}\n")
        .expect("Failed to write to temp file");

    let (stdout, _stderr, exit_code) = run_kelora_with_files(
        &[
            "-f",
            "jsonl",
            "--exec",
            "print(\"File: \" + meta.filename + \", Message: \" + e.message)",
        ],
        &[
            temp_file1.path().to_str().unwrap(),
            temp_file2.path().to_str().unwrap(),
        ],
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("File: ") && stdout.contains("Message: test1"),
        "Should show filename and message for file1: {}",
        stdout
    );
    assert!(
        stdout.contains("File: ") && stdout.contains("Message: test2"),
        "Should show filename and message for file2: {}",
        stdout
    );
}

#[test]
fn test_filename_tracking_jsonl_parallel() {
    // Test filename tracking with JSONL format in parallel mode
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1
        .write_all(b"{\"message\": \"test1\"}\n")
        .expect("Failed to write to temp file");
    temp_file2
        .write_all(b"{\"message\": \"test2\"}\n")
        .expect("Failed to write to temp file");

    let (stdout, _stderr, exit_code) = run_kelora_with_files(
        &[
            "-f",
            "jsonl",
            "--parallel",
            "--exec",
            "print(\"File: \" + meta.filename + \", Message: \" + e.message)",
        ],
        &[
            temp_file1.path().to_str().unwrap(),
            temp_file2.path().to_str().unwrap(),
        ],
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("File: ") && stdout.contains("Message: test1"),
        "Should show filename and message for file1: {}",
        stdout
    );
    assert!(
        stdout.contains("File: ") && stdout.contains("Message: test2"),
        "Should show filename and message for file2: {}",
        stdout
    );
}

#[test]
fn test_filename_tracking_line_format() {
    // Test filename tracking with line format
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1
        .write_all(b"line from file1\n")
        .expect("Failed to write to temp file");
    temp_file2
        .write_all(b"line from file2\n")
        .expect("Failed to write to temp file");

    let (stdout, _stderr, exit_code) = run_kelora_with_files(
        &[
            "-f",
            "line",
            "--exec",
            "print(\"File: \" + meta.filename + \", Line: \" + line)",
        ],
        &[
            temp_file1.path().to_str().unwrap(),
            temp_file2.path().to_str().unwrap(),
        ],
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("File: ") && stdout.contains("Line: line from file1"),
        "Should show filename and content for file1: {}",
        stdout
    );
    assert!(
        stdout.contains("File: ") && stdout.contains("Line: line from file2"),
        "Should show filename and content for file2: {}",
        stdout
    );
}

#[test]
fn test_per_file_csv_schema_detection_sequential() {
    // Test per-file CSV schema detection in sequential mode
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1
        .write_all(b"name,age\nAlice,30\nBob,25\n")
        .expect("Failed to write to temp file");
    temp_file2
        .write_all(b"user,score,level\nCharlie,95,A\nDave,88,B\n")
        .expect("Failed to write to temp file");

    let (stdout, _stderr, exit_code) = run_kelora_with_files(
        &[
            "-f", "csv",
            "--exec", "let fields = e.keys(); print(\"File: \" + meta.filename + \", Fields: \" + fields.join(\",\"))"
        ],
        &[temp_file1.path().to_str().unwrap(), temp_file2.path().to_str().unwrap()],
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Fields: name,age") || stdout.contains("Fields: age,name"),
        "Should detect schema for file1: {}",
        stdout
    );
    assert!(
        stdout.contains("Fields: user,score,level")
            || stdout.contains("Fields: level,score,user")
            || stdout.contains("Fields: score,user,level"),
        "Should detect schema for file2: {}",
        stdout
    );
}

#[test]
fn test_per_file_csv_schema_detection_parallel() {
    // Test per-file CSV schema detection in parallel mode
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1
        .write_all(b"name,age\nAlice,30\nBob,25\n")
        .expect("Failed to write to temp file");
    temp_file2
        .write_all(b"user,score,level\nCharlie,95,A\nDave,88,B\n")
        .expect("Failed to write to temp file");

    let (stdout, _stderr, exit_code) = run_kelora_with_files(
        &[
            "-f", "csv",
            "--parallel",
            "--exec", "let fields = e.keys(); print(\"File: \" + meta.filename + \", Fields: \" + fields.join(\",\"))"
        ],
        &[temp_file1.path().to_str().unwrap(), temp_file2.path().to_str().unwrap()],
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Fields: name,age") || stdout.contains("Fields: age,name"),
        "Should detect schema for file1: {}",
        stdout
    );
    assert!(
        stdout.contains("Fields: user,score,level")
            || stdout.contains("Fields: level,score,user")
            || stdout.contains("Fields: score,user,level"),
        "Should detect schema for file2: {}",
        stdout
    );
}

#[test]
fn test_csv_with_different_column_counts() {
    // Test CSV files with different numbers of columns
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1
        .write_all(b"a,b\n1,2\n")
        .expect("Failed to write to temp file");
    temp_file2
        .write_all(b"x,y,z,w\n10,20,30,40\n")
        .expect("Failed to write to temp file");

    let (stdout, _stderr, exit_code) = run_kelora_with_files(
        &[
            "-f", "csv",
            "--exec", "let count = e.keys().len(); print(\"File: \" + meta.filename + \", Columns: \" + count)"
        ],
        &[temp_file1.path().to_str().unwrap(), temp_file2.path().to_str().unwrap()],
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Columns: 2"),
        "Should detect 2 columns in file1: {}",
        stdout
    );
    assert!(
        stdout.contains("Columns: 4"),
        "Should detect 4 columns in file2: {}",
        stdout
    );
}

#[test]
fn test_sequential_parallel_mode_parity() {
    // Test that sequential and parallel modes produce similar results
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1.write_all(b"{\"user\": \"alice\", \"status\": \"active\"}\n{\"user\": \"bob\", \"status\": \"inactive\"}\n").expect("Failed to write to temp file");
    temp_file2.write_all(b"{\"user\": \"charlie\", \"status\": \"active\"}\n{\"user\": \"dave\", \"status\": \"inactive\"}\n").expect("Failed to write to temp file");

    let files = &[
        temp_file1.path().to_str().unwrap(),
        temp_file2.path().to_str().unwrap(),
    ];

    // Test sequential mode
    let (stdout_seq, stderr_seq, exit_code_seq) = run_kelora_with_files(
        &[
            "-f",
            "jsonl",
            "--exec",
            "print(\"File: \" + meta.filename + \", User: \" + e.user + \", Status: \" + e.status)",
        ],
        files,
    );

    // Test parallel mode
    let (stdout_par, stderr_par, exit_code_par) = run_kelora_with_files(
        &[
            "-f",
            "jsonl",
            "--parallel",
            "--exec",
            "print(\"File: \" + meta.filename + \", User: \" + e.user + \", Status: \" + e.status)",
        ],
        files,
    );

    assert_eq!(
        exit_code_seq, 0,
        "Sequential mode should exit successfully, stderr: {}",
        stderr_seq
    );
    assert_eq!(
        exit_code_par, 0,
        "Parallel mode should exit successfully, stderr: {}",
        stderr_par
    );

    // Both modes should show filename tracking
    assert!(
        stdout_seq.contains("File: ") && stdout_seq.contains("User: alice"),
        "Sequential mode should show filename and user data: {}",
        stdout_seq
    );
    assert!(
        stdout_par.contains("File: ") && stdout_par.contains("User: alice"),
        "Parallel mode should show filename and user data: {}",
        stdout_par
    );
}

#[test]
fn test_filename_tracking_with_file_order() {
    // Test filename tracking with file ordering
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1
        .write_all(b"first\n")
        .expect("Failed to write to temp file");
    temp_file2
        .write_all(b"second\n")
        .expect("Failed to write to temp file");

    let (stdout, _stderr, exit_code) = run_kelora_with_files(
        &[
            "-f",
            "line",
            "--file-order",
            "name",
            "--exec",
            "print(\"Processing: \" + meta.filename + \" -> \" + line)",
        ],
        &[
            temp_file1.path().to_str().unwrap(),
            temp_file2.path().to_str().unwrap(),
        ],
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Processing: ") && stdout.contains("first"),
        "Should process first file: {}",
        stdout
    );
    assert!(
        stdout.contains("Processing: ") && stdout.contains("second"),
        "Should process second file: {}",
        stdout
    );
}

#[test]
fn test_csv_no_headers_with_filename_tracking() {
    // Test CSV without headers but with filename tracking
    let mut temp_file1 = NamedTempFile::new().expect("Failed to create temp file");
    let mut temp_file2 = NamedTempFile::new().expect("Failed to create temp file");

    temp_file1
        .write_all(b"alice,30\nbob,25\n")
        .expect("Failed to write to temp file");
    temp_file2
        .write_all(b"charlie,95,A\ndave,88,B\n")
        .expect("Failed to write to temp file");

    let (stdout, _stderr, exit_code) = run_kelora_with_files(
        &[
            "-f",
            "csvnh",
            "--exec",
            "print(\"File: \" + meta.filename + \", Col1: \" + e.c1 + \", Col2: \" + e.c2)",
        ],
        &[
            temp_file1.path().to_str().unwrap(),
            temp_file2.path().to_str().unwrap(),
        ],
    );

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("File: ") && stdout.contains("Col1: alice"),
        "Should show filename and data for file1: {}",
        stdout
    );
    assert!(
        stdout.contains("File: ") && stdout.contains("Col1: charlie"),
        "Should show filename and data for file2: {}",
        stdout
    );
}

/// Helper function to run kelora with multiple files
fn run_kelora_with_files(args: &[&str], files: &[&str]) -> (String, String, i32) {
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

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_sequential_mode() {
//     // Test error stats counting in sequential mode with mixed valid/invalid JSON
//     let input = r#"{"valid": "json", "status": 200}
// {malformed json line}
// {"another": "valid", "status": 404}
// not jsonl at all
// {"final": "entry", "status": 500}"#;
//
//     let (stdout, _stderr, exit_code) =
//         run_kelora_with_input(&["-f", "jsonl", "--on-error", "skip", "--stats"], input);
//     assert_eq!(
//         exit_code, 0,
//         "Should exit successfully with skip error handling"
//     );
//
//     // Should output 3 valid JSON lines, skip 2 malformed ones
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 3, "Should output 3 valid JSON lines");
//
//     // Stats should show separate error count
//     assert!(stderr.contains("5 total"), "Should show 5 total lines");
//     assert!(stderr.contains("2 errors"), "Should show 2 parsing errors");
//     assert!(
//         stderr.contains("0 filtered"),
//         "Should show 0 filtered lines"
//     );
//     assert!(
//         stderr.contains("Events created: 3 total, 3 output, 0 filtered"),
//         "Should show 3 events created and output"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_parallel_mode() {
//     // Test error stats counting in parallel mode with mixed valid/invalid JSON
//     let input = r#"{"valid": "json", "status": 200}
// {malformed json line}
// {"another": "valid", "status": 404}
// not jsonl at all
// {"final": "entry", "status": 500}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "-f",
//             "jsonl",
//             "--on-error",
//             "skip",
//             "--stats",
//             "--parallel",
//             "--batch-size",
//             "2",
//         ],
//         input,
//     );
//     assert_eq!(
//         exit_code, 0,
//         "Should exit successfully with skip error handling"
//     );
//
//     // Should output 3 valid JSON lines, skip 2 malformed ones
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 3, "Should output 3 valid JSON lines");
//
//     // Stats should show separate error count (same as sequential)
//     assert!(stderr.contains("5 total"), "Should show 5 total lines");
//     assert!(stderr.contains("2 errors"), "Should show 2 parsing errors");
//     assert!(
//         stderr.contains("0 filtered"),
//         "Should show 0 filtered lines"
//     );
//     assert!(
//         stderr.contains("Events created: 3 total, 3 output, 0 filtered"),
//         "Should show 3 events created and output"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_with_filter_expression() {
//     // Test error stats with both parsing errors and filter expression rejections
//     let input = r#"{"valid": "json", "status": 200}
// {malformed json line}
// {"another": "valid", "status": 404}
// not jsonl at all
// {"final": "entry", "status": 500}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "-f",
//             "jsonl",
//             "--filter",
//             "e.status >= 400",
//             "--on-error",
//             "skip",
//             "--stats",
//         ],
//         input,
//     );
//     assert_eq!(exit_code, 0, "Should exit successfully");
//
//     // Should output 2 lines (status 404 and 500), filter out 1 (status 200), skip 2 malformed
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(
//         output_lines.len(),
//         2,
//         "Should output 2 lines with status >= 400"
//     );
//
//     // Stats should show separate error and filtered counts
//     assert!(stderr.contains("5 total"), "Should show 5 total lines");
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(
//         stderr.contains("1 filtered"),
//         "Should show 1 filtered line (status 200)"
//     );
//     assert!(stderr.contains("2 errors"), "Should show 2 parsing errors");
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_with_ignore_lines() {
//     // Test error stats with ignore-lines preprocessing
//     let input = r#"# This is a comment
// {"valid": "json", "status": 200}
// {malformed json line}
// # Another comment
// {"another": "valid", "status": 404}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "-f",
//             "jsonl",
//             "--ignore-lines",
//             "^#",
//             "--on-error",
//             "skip",
//             "--stats",
//         ],
//         input,
//     );
//     assert_eq!(exit_code, 0, "Should exit successfully");
//
//     // Should output 2 valid JSON lines, ignore 2 comments, skip 1 malformed
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 2, "Should output 2 valid JSON lines");
//
//     // Stats should show combined filtered count (ignore-lines + filter expressions)
//     assert!(stderr.contains("5 total"), "Should show 5 total lines");
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(
//         stderr.contains("2 filtered"),
//         "Should show 2 filtered lines (comments)"
//     );
//     assert!(stderr.contains("1 errors"), "Should show 1 parsing error");
// }

// TODO: Update test for new stats format and error handling
// #[test]
// fn test_error_stats_different_error_strategies() { ... }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_no_errors() {
//     // Test that error stats are not shown when there are no errors
//     let input = r#"{"valid": "json", "status": 200}
// {"another": "valid", "status": 404}
// {"final": "entry", "status": 500}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &["-f", "jsonl", "--filter", "status >= 400", "--stats"],
//         input,
//     );
//     assert_eq!(exit_code, 0, "Should exit successfully");
//
//     // Should output 2 lines (status 404 and 500), filter out 1 (status 200)
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(
//         output_lines.len(),
//         2,
//         "Should output 2 lines with status >= 400"
//     );
//
//     // Stats should not show error count when there are no errors
//     assert!(stderr.contains("3 total"), "Should show 3 total lines");
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(stderr.contains("1 filtered"), "Should show 1 filtered line");
//     assert!(
//         !stderr.contains("errors"),
//         "Should not show error count when there are no errors"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_parallel_vs_sequential_consistency() {
//     // Test that parallel and sequential modes show identical error stats
//     let input = r#"{"valid": "json", "status": 200}
// {malformed json line}
// {"another": "valid", "status": 404}
// not jsonl at all
// {"final": "entry", "status": 500}
// invalid json again"#;
//
//     // Run in sequential mode
//     let (stdout_seq, stderr_seq, exit_code_seq) = run_kelora_with_input(
//         &[
//             "-f",
//             "jsonl",
//             "--filter",
//             "e.status >= 400",
//             "--on-error",
//             "skip",
//             "--stats",
//         ],
//         input,
//     );
//
//     // Run in parallel mode
//     let (stdout_par, stderr_par, exit_code_par) = run_kelora_with_input(
//         &[
//             "-f",
//             "jsonl",
//             "--filter",
//             "e.status >= 400",
//             "--on-error",
//             "skip",
//             "--stats",
//             "--parallel",
//             "--batch-size",
//             "2",
//         ],
//         input,
//     );
//
//     assert_eq!(exit_code_seq, 0, "Sequential mode should exit successfully");
//     assert_eq!(exit_code_par, 0, "Parallel mode should exit successfully");
//
//     // Both should produce the same output
//     let seq_lines: Vec<&str> = stdout_seq.trim().split('\n').collect();
//     let par_lines: Vec<&str> = stdout_par.trim().split('\n').collect();
//     assert_eq!(
//         seq_lines.len(),
//         par_lines.len(),
//         "Should produce same number of output lines"
//     );
//
//     // Both should show identical statistics
//     let expected_stats = ["6 total", "2 output", "1 filtered", "3 errors"];
//     for stat in &expected_stats {
//         assert!(
//             stderr_seq.contains(stat),
//             "Sequential mode should contain: {}",
//             stat
//         );
//         assert!(
//             stderr_par.contains(stat),
//             "Parallel mode should contain: {}",
//             stat
//         );
//     }
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_multiline_mode() {
//     // Test error stats in multiline mode to ensure proper display format
//     let input = r#"{"valid": "json", "message": "line1\nline2"}
// {malformed json line}
// {"another": "valid", "message": "single line"}"#;
//
//     let (stdout, _stderr, exit_code) =
//         run_kelora_with_input(&["-f", "jsonl", "--on-error", "skip", "--stats"], input);
//     assert_eq!(exit_code, 0, "Should exit successfully");
//
//     // Should output 2 valid JSON lines, skip 1 malformed line
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 2, "Should output 2 valid JSON lines");
//
//     // Stats should show separate error count
//     assert!(
//         stderr.contains("3 total"),
//         "Should show 3 total lines processed"
//     );
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(stderr.contains("1 errors"), "Should show 1 parsing error");
//     assert!(
//         stderr.contains("0 filtered"),
//         "Should show 0 filtered lines"
//     );
//
//     // Test multiline mode specifically with events created
//     let (_stdout2, stderr2, exit_code2) = run_kelora_with_input(
//         &[
//             "-f",
//             "jsonl",
//             "--multiline",
//             "indent",
//             "--on-error",
//             "skip",
//             "--stats",
//         ],
//         input,
//     );
//     assert_eq!(
//         exit_code2, 0,
//         "Should exit successfully with multiline mode"
//     );
//
//     // In multiline mode, stats should show both line and event information
//     assert!(
//         stderr2.contains("Events created:"),
//         "Should show event statistics in multiline mode"
//     );
//     assert!(
//         stderr2.contains("1 errors"),
//         "Should show 1 parsing error in multiline mode"
//     );
// }

#[test]
fn test_empty_line_handling_line_format() {
    // Test that empty lines are processed as events in line format
    let input = "first line\n\nsecond line\n\n\nthird line\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "--exec",
            "print(\"Line: [\" + line + \"]\")",
            "-F",
            "hide",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "Should exit successfully with line format");

    // Should process all lines including empty ones
    let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        output_lines.len(),
        6,
        "Should process all 6 lines including empty ones"
    );

    // Check that empty lines are present
    assert!(
        stdout.contains("Line: []"),
        "Should process empty lines as events"
    );
    assert!(
        stdout.contains("Line: [first line]"),
        "Should process non-empty lines"
    );
    assert!(
        stdout.contains("Line: [second line]"),
        "Should process non-empty lines"
    );
    assert!(
        stdout.contains("Line: [third line]"),
        "Should process non-empty lines"
    );
}

#[test]
fn test_empty_line_handling_structured_formats() {
    // Test that empty lines are skipped in structured formats
    let input = r#"{"level": "INFO", "message": "First message"}

{"level": "ERROR", "message": "Second message"}

{"level": "DEBUG", "message": "Third message"}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "-F", "jsonl"], input);
    assert_eq!(exit_code, 0, "Should exit successfully with jsonl format");

    // Should skip empty lines and only process JSON lines
    let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        output_lines.len(),
        3,
        "Should process only 3 JSON lines, skipping empty ones"
    );

    // Verify all output lines are valid JSON
    for line in output_lines {
        let parsed: serde_json::Value =
            serde_json::from_str(line).expect("Line should be valid JSON");
        assert!(parsed.is_object(), "Each line should be a JSON object");
    }
}

#[test]
fn test_empty_line_handling_line_format_with_filter() {
    // Test that empty lines can be filtered in line format
    let input = "first line\n\nsecond line\n\n\nthird line\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "--filter",
            "line.len() > 0",
            "--exec",
            "print(\"Non-empty: \" + line)",
            "-F",
            "hide",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "Should exit successfully with line format and filter"
    );

    // Should filter out empty lines
    let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(output_lines.len(), 3, "Should filter to 3 non-empty lines");

    // Check that only non-empty lines are present
    assert!(
        stdout.contains("Non-empty: first line"),
        "Should contain first line"
    );
    assert!(
        stdout.contains("Non-empty: second line"),
        "Should contain second line"
    );
    assert!(
        stdout.contains("Non-empty: third line"),
        "Should contain third line"
    );

    // Check that there are no empty line entries (lines with just "Non-empty: " followed by newline)
    for line in output_lines {
        assert!(
            line.len() > "Non-empty: ".len(),
            "Should not have empty line entries: '{}'",
            line
        );
    }
}

// TODO: Update test for new stats format
// #[test]
// fn test_empty_line_handling_line_format_with_stats() { ... }

// TODO: Update test for new stats format
// #[test]
// fn test_empty_line_handling_structured_format_with_stats() { ... }

#[test]
fn test_empty_line_handling_consistency_across_formats() {
    // Test that empty line handling is consistent with format expectations
    let input = "line1\n\nline2\n\n";

    // Line format should process all lines
    let (stdout_line, _stderr_line, exit_code_line) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "--exec",
            "print(\"[\" + line + \"]\")",
            "-F",
            "hide",
        ],
        input,
    );
    assert_eq!(exit_code_line, 0, "Line format should exit successfully");
    let line_count = stdout_line.trim().split('\n').collect::<Vec<&str>>().len();
    assert_eq!(
        line_count, 4,
        "Line format should process 4 lines including empty ones"
    );

    // Structured format (cols) should skip empty lines
    let (stdout_cols, _stderr_cols, exit_code_cols) = run_kelora_with_input(
        &[
            "-f",
            "cols",
            "--exec",
            "print(\"[\" + e.c1 + \"]\")",
            "-F",
            "hide",
        ],
        input,
    );
    assert_eq!(exit_code_cols, 0, "Cols format should exit successfully");
    let cols_count = stdout_cols.trim().split('\n').collect::<Vec<&str>>().len();
    assert_eq!(
        cols_count, 2,
        "Cols format should process 2 lines, skipping empty ones"
    );
}

#[test]
fn test_empty_line_handling_parallel_mode_line_format() {
    // Test that empty lines are processed correctly in parallel mode with line format
    let input = "first line\n\nsecond line\n\n\nthird line\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "--parallel",
            "--batch-size",
            "2",
            "--exec",
            "print(\"Line: [\" + line + \"]\")",
            "-F",
            "hide",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "Should exit successfully with line format in parallel mode"
    );

    // Should process all lines including empty ones
    let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        output_lines.len(),
        6,
        "Should process all 6 lines including empty ones in parallel mode"
    );

    // Check that empty lines are present
    assert!(
        stdout.contains("Line: []"),
        "Should process empty lines as events in parallel mode"
    );
    assert!(
        stdout.contains("Line: [first line]"),
        "Should process non-empty lines in parallel mode"
    );
}

#[test]
fn test_take_limit_basic() {
    let input = r#"{"level": "INFO", "message": "Line 1"}
{"level": "INFO", "message": "Line 2"}
{"level": "INFO", "message": "Line 3"}
{"level": "INFO", "message": "Line 4"}
{"level": "INFO", "message": "Line 5"}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "--take", "3"], input);

    assert_eq!(exit_code, 0, "kelora should exit successfully with --take");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        3,
        "Should output exactly 3 lines when --take 3 is specified"
    );

    // Check that it outputs the first 3 lines
    assert!(stdout.contains("Line 1"), "Should include first line");
    assert!(stdout.contains("Line 2"), "Should include second line");
    assert!(stdout.contains("Line 3"), "Should include third line");
    assert!(!stdout.contains("Line 4"), "Should not include fourth line");
    assert!(!stdout.contains("Line 5"), "Should not include fifth line");
}

#[test]
fn test_take_limit_with_filter() {
    let input = r#"{"level": "INFO", "message": "Good line 1"}
{"level": "ERROR", "message": "Bad line 1"}
{"level": "INFO", "message": "Good line 2"}
{"level": "ERROR", "message": "Bad line 2"}
{"level": "INFO", "message": "Good line 3"}
{"level": "ERROR", "message": "Bad line 3"}
{"level": "INFO", "message": "Good line 4"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--filter",
            "e.level == \"INFO\"",
            "--take",
            "2",
        ],
        input,
    );

    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --take and --filter"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        2,
        "Should output exactly 2 lines when --take 2 is specified with filter"
    );

    // Check that it outputs the first 2 INFO lines
    assert!(
        stdout.contains("Good line 1"),
        "Should include first INFO line"
    );
    assert!(
        stdout.contains("Good line 2"),
        "Should include second INFO line"
    );
    assert!(
        !stdout.contains("Good line 3"),
        "Should not include third INFO line due to --take 2"
    );
    assert!(
        !stdout.contains("Bad line"),
        "Should not include any ERROR lines due to filter"
    );
}

#[test]
fn test_take_limit_zero() {
    let input = r#"{"level": "INFO", "message": "Line 1"}
{"level": "INFO", "message": "Line 2"}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "--take", "0"], input);

    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --take 0"
    );

    let output = stdout.trim();
    assert!(
        output.is_empty(),
        "Should output no lines when --take 0 is specified"
    );
}

#[test]
fn test_take_limit_larger_than_input() {
    let input = r#"{"level": "INFO", "message": "Line 1"}
{"level": "INFO", "message": "Line 2"}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "--take", "10"], input);

    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --take larger than input"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        2,
        "Should output all available lines when --take is larger than input"
    );

    assert!(stdout.contains("Line 1"), "Should include first line");
    assert!(stdout.contains("Line 2"), "Should include second line");
}

#[test]
fn test_take_limit_parallel_mode() {
    let input = r#"{"level": "INFO", "message": "Line 1"}
{"level": "INFO", "message": "Line 2"}
{"level": "INFO", "message": "Line 3"}
{"level": "INFO", "message": "Line 4"}
{"level": "INFO", "message": "Line 5"}
{"level": "INFO", "message": "Line 6"}
{"level": "INFO", "message": "Line 7"}
{"level": "INFO", "message": "Line 8"}
{"level": "INFO", "message": "Line 9"}
{"level": "INFO", "message": "Line 10"}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "jsonl", "--take", "3", "--parallel"], input);

    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --take and --parallel"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        3,
        "Should output exactly 3 lines when --take 3 is specified in parallel mode"
    );

    // Check that it outputs the first 3 lines (order should be preserved by default)
    assert!(stdout.contains("Line 1"), "Should include first line");
    assert!(stdout.contains("Line 2"), "Should include second line");
    assert!(stdout.contains("Line 3"), "Should include third line");
    assert!(!stdout.contains("Line 4"), "Should not include fourth line");
    assert!(!stdout.contains("Line 10"), "Should not include tenth line");
}

#[test]
fn test_take_limit_parallel_small_batches() {
    let input = r#"{"level": "INFO", "message": "Line 1"}
{"level": "INFO", "message": "Line 2"}
{"level": "INFO", "message": "Line 3"}
{"level": "INFO", "message": "Line 4"}
{"level": "INFO", "message": "Line 5"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--take",
            "3",
            "--parallel",
            "--batch-size",
            "1",
        ],
        input,
    );

    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --take, --parallel, and small batch size"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        3,
        "Should output exactly 3 lines when --take 3 with batch-size 1 in parallel mode"
    );
}

#[test]
fn test_take_limit_parallel_with_filter() {
    let input = r#"{"level": "INFO", "message": "Good line 1"}
{"level": "ERROR", "message": "Bad line 1"}
{"level": "INFO", "message": "Good line 2"}
{"level": "ERROR", "message": "Bad line 2"}
{"level": "INFO", "message": "Good line 3"}
{"level": "ERROR", "message": "Bad line 3"}
{"level": "INFO", "message": "Good line 4"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--filter",
            "e.level == \"INFO\"",
            "--take",
            "2",
            "--parallel",
        ],
        input,
    );

    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --take, --filter, and --parallel"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        2,
        "Should output exactly 2 lines when --take 2 with filter in parallel mode"
    );

    // Check that it outputs the first 2 INFO lines
    assert!(
        stdout.contains("Good line 1"),
        "Should include first INFO line"
    );
    assert!(
        stdout.contains("Good line 2"),
        "Should include second INFO line"
    );
    assert!(
        !stdout.contains("Good line 3"),
        "Should not include third INFO line due to --take 2"
    );
    assert!(
        !stdout.contains("Bad line"),
        "Should not include any ERROR lines due to filter"
    );
}

#[test]
fn test_take_limit_parallel_unordered() {
    let input = r#"{"level": "INFO", "message": "Line 1"}
{"level": "INFO", "message": "Line 2"}
{"level": "INFO", "message": "Line 3"}
{"level": "INFO", "message": "Line 4"}
{"level": "INFO", "message": "Line 5"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "jsonl", "--take", "3", "--parallel", "--unordered"],
        input,
    );

    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --take, --parallel, and --unordered"
    );

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(
        lines.len(),
        3,
        "Should output exactly 3 lines when --take 3 in unordered parallel mode"
    );

    // In unordered mode, we can't guarantee which 3 lines we get, but we should get exactly 3
    // and they should all be from our input
    for line in lines {
        assert!(
            line.contains("Line"),
            "Each output line should contain 'Line'"
        );
    }
}

// =============================================================================
// METRICS REGRESSION TESTS
// =============================================================================
// These tests ensure that the --metrics functionality works correctly in both
// sequential and parallel modes. This prevents regression of the bug where
// metrics were broken due to incorrect thread-local vs global state handling.

#[test]
fn test_metrics_sequential_mode_basic() {
    let input = r#"{"level":"info","message":"test1"}
{"level":"error","message":"test2"}
{"level":"info","message":"test3"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "track_count(\"total\"); track_count(\"level_\" + e.level)",
            "--metrics",
        ],
        input,
    );

    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Check that metrics output appears in stderr
    assert!(
        stderr.contains("=== Kelora Metrics ==="),
        "Should contain metrics header"
    );
    assert!(
        stderr.contains("total        = 3"),
        "Should count total events"
    );
    assert!(
        stderr.contains("level_info   = 2"),
        "Should count info events"
    );
    assert!(
        stderr.contains("level_error  = 1"),
        "Should count error events"
    );

    // Check that main output still appears in stdout
    assert!(
        stdout.contains("level=\"info\""),
        "Should output processed events"
    );
    assert!(
        stdout.contains("level=\"error\""),
        "Should output processed events"
    );
}

#[test]
fn test_metrics_parallel_mode_basic() {
    let input = r#"{"level":"info","message":"test1"}
{"level":"error","message":"test2"}
{"level":"info","message":"test3"}
{"level":"warn","message":"test4"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "track_count(\"total\"); track_count(\"level_\" + e.level)",
            "--metrics",
            "--parallel",
            "--batch-size",
            "2",
        ],
        input,
    );

    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Check that metrics output appears in stderr (same as sequential)
    assert!(
        stderr.contains("=== Kelora Metrics ==="),
        "Should contain metrics header"
    );
    assert!(
        stderr.contains("total        = 4"),
        "Should count total events across workers"
    );
    assert!(
        stderr.contains("level_info   = 2"),
        "Should count info events across workers"
    );
    assert!(
        stderr.contains("level_error  = 1"),
        "Should count error events across workers"
    );
    assert!(
        stderr.contains("level_warn   = 1"),
        "Should count warn events across workers"
    );

    // Check that main output still appears in stdout
    assert!(
        stdout.contains("level=\"info\""),
        "Should output processed events"
    );
    assert!(
        stdout.contains("level=\"error\""),
        "Should output processed events"
    );
    assert!(
        stdout.contains("level=\"warn\""),
        "Should output processed events"
    );
}

#[test]
fn test_metrics_file_output() {
    let input = r#"{"level":"info","message":"test1"}
{"level":"error","message":"test2"}"#;

    // Create a temporary file for metrics output
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let metrics_file_path = temp_file.path().to_str().unwrap();

    let (_stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            "track_count(\"total\"); track_count(\"level_\" + e.level)",
            "--metrics-file",
            metrics_file_path,
        ],
        input,
    );

    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Read the metrics file content
    let metrics_content =
        std::fs::read_to_string(metrics_file_path).expect("Failed to read metrics file");

    // Parse as JSON to verify structure
    let metrics_json: serde_json::Value =
        serde_json::from_str(&metrics_content).expect("Metrics file should contain valid JSON");

    // Check metrics content
    assert_eq!(metrics_json["total"], 2, "Should have total count");
    assert_eq!(metrics_json["level_info"], 1, "Should have info count");
    assert_eq!(metrics_json["level_error"], 1, "Should have error count");

    // No metrics should appear in stderr when using file output only
    assert!(
        !stderr.contains("=== Kelora Metrics ==="),
        "Should not display metrics to stderr"
    );
}

#[test]
fn test_metrics_parallel_consistency() {
    // Test that parallel mode produces correct metrics with different batch sizes
    let input = r#"{"level":"info","message":"test1"}
{"level":"error","message":"test2"}
{"level":"info","message":"test3"}
{"level":"warn","message":"test4"}
{"level":"error","message":"test5"}"#;

    let exec_script = "track_count(\"total\"); track_count(\"level_\" + e.level)";

    // Run in parallel mode with batch-size 1
    let (_stdout1, stderr1, exit_code1) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            exec_script,
            "--metrics",
            "--parallel",
            "--batch-size",
            "1",
        ],
        input,
    );
    assert_eq!(
        exit_code1, 0,
        "Parallel mode batch-size 1 should exit successfully"
    );

    // Run in parallel mode with batch-size 2
    let (_stdout2, stderr2, exit_code2) = run_kelora_with_input(
        &[
            "-f",
            "jsonl",
            "--exec",
            exec_script,
            "--metrics",
            "--parallel",
            "--batch-size",
            "2",
        ],
        input,
    );
    assert_eq!(
        exit_code2, 0,
        "Parallel mode batch-size 2 should exit successfully"
    );

    // Both should have identical metrics
    assert!(
        stderr1.contains("total        = 5"),
        "Batch-size 1 should count all events"
    );
    assert!(
        stderr2.contains("total        = 5"),
        "Batch-size 2 should count all events"
    );

    assert!(
        stderr1.contains("level_info   = 2"),
        "Batch-size 1 should count info events"
    );
    assert!(
        stderr2.contains("level_info   = 2"),
        "Batch-size 2 should count info events"
    );

    assert!(
        stderr1.contains("level_error  = 2"),
        "Batch-size 1 should count error events"
    );
    assert!(
        stderr2.contains("level_error  = 2"),
        "Batch-size 2 should count error events"
    );

    assert!(
        stderr1.contains("level_warn   = 1"),
        "Batch-size 1 should count warn events"
    );
    assert!(
        stderr2.contains("level_warn   = 1"),
        "Batch-size 2 should count warn events"
    );
}
