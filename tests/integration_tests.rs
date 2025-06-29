// tests/integration_tests.rs
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

/// Helper function to run kelora with given arguments and input via stdin
fn run_kelora_with_input(args: &[&str], input: &str) -> (String, String, i32) {
    let mut cmd = Command::new("cargo")
        .arg("run")
        .arg("--")
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

    let cmd = Command::new("cargo")
        .arg("run")
        .arg("--")
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
    assert!(stdout.contains("kelora 0.2.0"), "Version output should contain version number");
}

#[test] 
fn test_help_flag() {
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["--help"], "");
    assert_eq!(exit_code, 0, "kelora --help should exit successfully");
    assert!(stdout.contains("command-line log analysis tool"), "Help should describe the tool");
    assert!(stdout.contains("--filter"), "Help should mention filter option");
    assert!(stdout.contains("--parallel"), "Help should mention parallel option");
}

#[test]
fn test_basic_jsonl_parsing() {
    let input = r#"{"level": "INFO", "message": "Hello world", "status": 200}
{"level": "ERROR", "message": "Something failed", "status": 500}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "-F", "jsonl"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines");
    
    // Parse JSON output
    let first_line: serde_json::Value = serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["level"], "INFO");
    assert_eq!(first_line["status"], 200);
}

#[test]
fn test_filter_expression() {
    let input = r#"{"level": "INFO", "status": 200}
{"level": "ERROR", "status": 500}
{"level": "DEBUG", "status": 404}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "-F", "jsonl", "--filter", "status >= 400"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should filter to 2 lines (status >= 400)");
    
    // Check that filtered results have status >= 400
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).expect("Line should be valid JSON");
        let status = parsed["status"].as_i64().expect("Status should be a number");
        assert!(status >= 400, "Filtered results should have status >= 400");
    }
}

#[test]
fn test_exec_script() {
    let input = r#"{"level": "INFO", "status": 200}
{"level": "ERROR", "status": 500}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl", 
        "-F", "jsonl",
        "--exec", "let alert_level = if status >= 400 { \"high\" } else { \"low\" };"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines");
    
    // Check that exec script added alert_level field
    let first_line: serde_json::Value = serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["alert_level"], "low");
    
    let second_line: serde_json::Value = serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(second_line["alert_level"], "high");
}

#[test]
fn test_text_output_format() {
    let input = r#"{"level": "INFO", "message": "Hello world", "status": 200}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "jsonl", "-F", "default"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    // Text format should be key=value pairs
    assert!(stdout.contains("level=\"INFO\""), "Text output should contain level=\"INFO\"");
    assert!(stdout.contains("status=200"), "Text output should contain status=200");
    assert!(stdout.contains("message=\"Hello world\""), "Text output should contain quoted message");
}

#[test]
fn test_keys_filtering() {
    let input = r#"{"level": "INFO", "message": "Hello world", "status": 200, "timestamp": "2023-01-01T00:00:00Z"}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl", 
        "-F", "jsonl",
        "--keys", "level,status"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).expect("Output should be valid JSON");
    
    // Should only contain specified keys
    assert!(parsed.get("level").is_some(), "Should contain level");
    assert!(parsed.get("status").is_some(), "Should contain status");
    assert!(parsed.get("message").is_none(), "Should not contain message");
    assert!(parsed.get("timestamp").is_none(), "Should not contain timestamp");
}

#[test]
fn test_global_tracking() {
    let input = r#"{"level": "INFO", "status": 200}
{"level": "ERROR", "status": 500}
{"level": "ERROR", "status": 404}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--filter", "status >= 400",
        "--exec", "track_count(\"errors\")",
        "--end", "print(`Errors: ${tracked[\"errors\"]}`)"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    // The end stage should print to stdout (Rhai print goes to stdout in this implementation)
    assert!(stdout.contains("Errors: 2"), "Should track filtered error lines");
}

#[test]
fn test_begin_and_end_stages() {
    let input = r#"{"level": "INFO"}
{"level": "ERROR"}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--begin", "print(\"Starting analysis...\")",
        "--end", "print(\"Analysis complete\")"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    assert!(stdout.contains("Starting analysis..."), "Begin stage should execute");
    assert!(stdout.contains("Analysis complete"), "End stage should execute");
}

#[test]
fn test_error_handling_skip() {
    let input = r#"{"level": "INFO", "status": 200}
invalid jsonl line
{"level": "ERROR", "status": 500}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl", 
        "--on-error", "skip"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully with skip error handling");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should skip invalid line and output 2 valid lines");
}

#[test]
fn test_error_handling_emit_errors() {
    let input = r#"{"level": "INFO", "status": 200}
invalid jsonl line
{"level": "ERROR", "status": 500}"#;
    
    let (stdout, stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl", 
        "--on-error", "emit-errors"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully with emit-errors");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 valid lines");
    assert!(stderr.contains("Parse error"), "Should emit parse error to stderr");
}

#[test]
fn test_parallel_mode() {
    let input = r#"{"level": "INFO", "status": 200}
{"level": "ERROR", "status": 500}
{"level": "DEBUG", "status": 404}
{"level": "WARN", "status": 403}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl", 
        "-F", "jsonl",
        "--parallel",
        "--threads", "2",
        "--filter", "status >= 400"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully in parallel mode");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should filter to 3 lines in parallel mode");
    
    // Verify all results have status >= 400
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).expect("Line should be valid JSON");
        let status = parsed["status"].as_i64().expect("Status should be a number");
        assert!(status >= 400, "Parallel filtered results should have status >= 400");
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
    let (seq_stdout, _seq_stderr, seq_exit_code) = run_kelora_with_input(&[
        "-f", "jsonl", 
        "-F", "jsonl",
        "--filter", "status >= 400",
        "--exec", "let processed = true"
    ], input);
    
    // Run parallel mode
    let (par_stdout, _par_stderr, par_exit_code) = run_kelora_with_input(&[
        "-f", "jsonl", 
        "-F", "jsonl",
        "--parallel",
        "--threads", "2",
        "--filter", "status >= 400", 
        "--exec", "let processed = true"
    ], input);
    
    // Both should exit successfully
    assert_eq!(seq_exit_code, 0, "Sequential mode should exit successfully");
    assert_eq!(par_exit_code, 0, "Parallel mode should exit successfully");
    
    // Parse and sort output lines for comparison (parallel may reorder)
    let mut seq_lines: Vec<&str> = seq_stdout.trim().split('\n').filter(|l| !l.is_empty() && l.starts_with('{')).collect();
    let mut par_lines: Vec<&str> = par_stdout.trim().split('\n').filter(|l| !l.is_empty() && l.starts_with('{')).collect();
    
    seq_lines.sort();
    par_lines.sort();
    
    // Should have same number of results
    assert_eq!(seq_lines.len(), par_lines.len(), "Sequential and parallel should produce same number of results");
    
    // Results should be functionally equivalent (same filtered and processed records)
    for (seq_line, par_line) in seq_lines.iter().zip(par_lines.iter()) {
        let seq_json: serde_json::Value = serde_json::from_str(seq_line).expect("Sequential output should be valid JSON");
        let par_json: serde_json::Value = serde_json::from_str(par_line).expect("Parallel output should be valid JSON");
        
        // Check that key fields match
        assert_eq!(seq_json["status"], par_json["status"], "Status should match between modes");
        assert_eq!(seq_json["user"], par_json["user"], "User should match between modes");
        assert_eq!(seq_json["processed"], par_json["processed"], "Processed field should match between modes");
        
        // Verify filtering worked correctly in both modes
        let status = seq_json["status"].as_i64().expect("Status should be a number");
        assert!(status >= 400, "Both modes should filter correctly");
    }
    
    // Verify both modes processed the same data successfully
    assert!(seq_lines.len() > 0, "Sequential mode should produce some output");
    assert!(par_lines.len() > 0, "Parallel mode should produce some output");
}

#[test]
fn test_file_input() {
    let file_content = r#"{"level": "INFO", "message": "File input test"}
{"level": "ERROR", "message": "Another line"}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_file(&["-f", "jsonl"], file_content);
    assert_eq!(exit_code, 0, "kelora should exit successfully with file input");
    
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
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "-F", "jsonl",
        "--exec", "let has_error = message.contains(\"Error\"); let code_num = code.to_int();"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).expect("Output should be valid JSON");
    assert_eq!(parsed["has_error"], true, "contains() function should work");
    assert_eq!(parsed["code_num"], 123, "to_int() function should work");
}

#[test]
fn test_multiple_filters() {
    let input = r#"{"level": "INFO", "status": 200, "response_time": 50}
{"level": "ERROR", "status": 500, "response_time": 100}
{"level": "WARN", "status": 404, "response_time": 200}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "-F", "jsonl",
        "--filter", "status >= 400",
        "--filter", "response_time > 150"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 1, "Should filter to 1 line matching both conditions");
    
    let parsed: serde_json::Value = serde_json::from_str(lines[0]).expect("Line should be valid JSON");
    assert_eq!(parsed["level"], "WARN");
    assert_eq!(parsed["status"], 404);
    assert_eq!(parsed["response_time"], 200);
}

#[test]
fn test_status_class_function() {
    let input = r#"{"status": 200}
{"status": 404}
{"status": 500}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "-F", "jsonl",
        "--exec", "let class = status.status_class();"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should output 3 lines");
    
    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first["class"], "2xx");
    
    let second: serde_json::Value = serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(second["class"], "4xx");
    
    let third: serde_json::Value = serde_json::from_str(lines[2]).expect("Third line should be valid JSON");
    assert_eq!(third["class"], "5xx");
}

#[test]
fn test_complex_rhai_expressions() {
    let input = r#"{"user": "alice", "status": 404}
{"user": "bob", "status": 500}
{"user": "charlie", "status": 200}
{"user": "alice", "status": 403}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "-F", "jsonl",
        "--filter", "status >= 400 && user.contains(\"a\")"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should filter to 2 lines (alice with status >= 400)");
    
    // Verify both results are alice with status >= 400
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).expect("Line should be valid JSON");
        assert_eq!(parsed["user"], "alice");
        let status = parsed["status"].as_i64().unwrap();
        assert!(status >= 400);
    }
}

#[test]
fn test_print_function_output() {
    let input = r#"{"user": "alice", "level": "INFO"}
{"user": "bob", "level": "ERROR"}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "-F", "jsonl",
        "--exec", "print(\"Processing user: \" + user);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    assert!(stdout.contains("Processing user: alice"), "Should print alice debug message");
    assert!(stdout.contains("Processing user: bob"), "Should print bob debug message");
    assert!(stdout.contains("\"user\":\"alice\""), "Should also output JSON data");
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
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--filter", "status >= 400",
        "--exec", "track_count(\"errors\");",
        "--end", "print(`Errors: ${tracked[\"errors\"]}`);"
    ], &large_input);
    let duration = start_time.elapsed();
    
    assert_eq!(exit_code, 0, "kelora should handle large input successfully");
    assert!(stdout.contains("Errors:"), "Should count errors in large dataset");
    
    // Performance check: should process 1000 lines in reasonable time
    assert!(duration.as_millis() < 5000, "Should process 1000 lines in less than 5 seconds, took {}ms", duration.as_millis());
}

#[test]
fn test_error_handling_mixed_valid_invalid() {
    let input = r#"{"valid": "json", "status": 200}
{malformed json line}
{"another": "valid", "status": 404}
not jsonl at all
{"final": "entry", "status": 500}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "-F", "jsonl",
        "--on-error", "skip"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully with skip error handling");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should output 3 valid JSON lines, skipping malformed ones");
    
    // Verify all output lines are valid JSON
    for line in lines {
        serde_json::from_str::<serde_json::Value>(line).expect("All output lines should be valid JSON");
    }
}

#[test]
fn test_tracking_with_min_max() {
    let input = r#"{"response_time": 150, "status": 200}
{"response_time": 500, "status": 404}
{"response_time": 75, "status": 200}
{"response_time": 800, "status": 500}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--exec", "track_min(\"min_time\", response_time); track_max(\"max_time\", response_time);",
        "--end", "print(`Min: ${tracked[\"min_time\"]}, Max: ${tracked[\"max_time\"]}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    assert!(stdout.contains("Min: 75"), "Should track minimum response time");
    assert!(stdout.contains("Max: 800"), "Should track maximum response time");
}

#[test]
fn test_field_modification_and_addition() {
    let input = r#"{"user": "alice", "score": 85}
{"user": "bob", "score": 92}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "-F", "jsonl",
        "--exec", "let grade = if score >= 90 { \"A\" } else { \"B\" }; let bonus_points = score * 0.1;"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines");
    
    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first["grade"], "B");
    assert_eq!(first["bonus_points"], 8.5);
    
    let second: serde_json::Value = serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
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
        "--exec", "track_unique(\"unique_ips\", ip); track_unique(\"unique_users\", user);",
        "--end", "print(`IPs: ${tracked[\"unique_ips\"].len()}, Users: ${tracked[\"unique_users\"].len()}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    // Should collect 3 unique IPs and 4 unique users
    assert!(stdout.contains("IPs: 3"), "Should track 3 unique IP addresses");
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
        "--exec", "track_bucket(\"status_counts\", status); track_bucket(\"method_counts\", method);",
        "--end", "print(`Status 200: ${tracked[\"status_counts\"].get(\"200\") ?? 0}, GET requests: ${tracked[\"method_counts\"].get(\"GET\") ?? 0}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    // Should count 2 occurrences of status 200 and 3 GET requests
    assert!(stdout.contains("Status 200: 2"), "Should count 2 occurrences of status 200");
    assert!(stdout.contains("GET requests: 3"), "Should count 3 GET requests");
}

#[test]
fn test_track_unique_parallel_mode() {
    let input = r#"{"ip": "1.1.1.1"}
{"ip": "2.2.2.2"}
{"ip": "1.1.1.1"}
{"ip": "3.3.3.3"}
{"ip": "2.2.2.2"}
{"ip": "4.4.4.4"}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--parallel",
        "--batch-size", "2",
        "--exec", "track_unique(\"ips\", ip);",
        "--end", "print(`Unique IPs: ${tracked[\"ips\"].len()}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully in parallel mode");
    
    // Should merge unique values from all workers
    assert!(stdout.contains("Unique IPs: 4"), "Should collect 4 unique IPs across parallel workers");
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
        "--exec", "track_bucket(\"status_counts\", status);",
        "--end", "let counts = tracked[\"status_counts\"]; print(`200: ${counts.get(\"200\") ?? 0}, 404: ${counts.get(\"404\") ?? 0}, 500: ${counts.get(\"500\") ?? 0}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully in parallel mode");
    
    // Should merge bucket counts from all workers
    assert!(stdout.contains("200: 3"), "Should count 3 occurrences of status 200");
    assert!(stdout.contains("404: 2"), "Should count 2 occurrences of status 404");
    assert!(stdout.contains("500: 1"), "Should count 1 occurrence of status 500");
}

#[test]
fn test_mixed_tracking_functions() {
    let input = r#"{"user": "alice", "response_time": 100, "status": "200"}
{"user": "bob", "response_time": 250, "status": "404"}
{"user": "alice", "response_time": 180, "status": "200"}
{"user": "charlie", "response_time": 50, "status": "500"}"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "jsonl",
        "--exec", "track_count(\"total\"); track_unique(\"users\", user); track_bucket(\"status_dist\", status); track_min(\"min_time\", response_time); track_max(\"max_time\", response_time);",
        "--end", "print(`Total: ${tracked[\"total\"]}, Users: ${tracked[\"users\"].len()}, Min: ${tracked[\"min_time\"]}, Max: ${tracked[\"max_time\"]}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    assert!(stdout.contains("Total: 4"), "Should count 4 total records");
    assert!(stdout.contains("Users: 3"), "Should track 3 unique users");  
    assert!(stdout.contains("Min: 50"), "Should track minimum response time");
    assert!(stdout.contains("Max: 250"), "Should track maximum response time");
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
        "--filter", "status >= 400",
        "--exec", "let alert_level = if status >= 500 { \"critical\" } else { \"warning\" }; track_count(\"total_errors\");",
        "--end", "print(`Total errors processed: ${tracked[\"total_errors\"]}`);"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");
    
    let lines: Vec<&str> = stdout.trim().lines().filter(|line| line.starts_with('{')).collect();
    assert_eq!(lines.len(), 3, "Should filter to 3 error lines");
    
    assert!(stdout.contains("Total errors processed: 3"), "Should count all error lines");
    
    // Verify alert levels are correctly assigned
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).expect("Line should be valid JSON");
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
fn test_syslog_rfc5424_parsing() {
    let input = r#"<165>1 2023-10-11T22:14:15.003Z server01 sshd 1234 ID47 - Failed password for user alice
<33>1 2023-10-11T22:14:16.123Z server01 nginx 5678 - - Request processed successfully"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "syslog",
        "-F", "jsonl"
    ], input);
    assert_eq!(exit_code, 0, "syslog parsing should succeed");
    
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should parse 2 syslog lines");
    
    // Check first line (SSH failure)
    let first_line: serde_json::Value = serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["pri"].as_i64().unwrap(), 165);
    assert_eq!(first_line["facility"].as_i64().unwrap(), 20); // 165 >> 3
    assert_eq!(first_line["severity"].as_i64().unwrap(), 5);  // 165 & 7
    assert_eq!(first_line["host"].as_str().unwrap(), "server01");
    assert_eq!(first_line["prog"].as_str().unwrap(), "sshd");
    assert_eq!(first_line["pid"].as_i64().unwrap(), 1234);
    assert_eq!(first_line["msg"].as_str().unwrap(), "Failed password for user alice");
    
    // Check second line (nginx success)
    let second_line: serde_json::Value = serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
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
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "syslog",
        "-F", "jsonl"
    ], input);
    assert_eq!(exit_code, 0, "syslog parsing should succeed");
    
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should parse 2 syslog lines");
    
    // Check first line (with PID)
    let first_line: serde_json::Value = serde_json::from_str(lines[0]).expect("First line should be valid JSON");
    assert_eq!(first_line["timestamp"].as_str().unwrap(), "Oct 11 22:14:15");
    assert_eq!(first_line["host"].as_str().unwrap(), "server01");
    assert_eq!(first_line["prog"].as_str().unwrap(), "sshd");
    assert_eq!(first_line["pid"].as_i64().unwrap(), 1234);
    assert_eq!(first_line["msg"].as_str().unwrap(), "Failed password for user bob");
    
    // Check second line (no PID)
    let second_line: serde_json::Value = serde_json::from_str(lines[1]).expect("Second line should be valid JSON");
    assert_eq!(second_line["timestamp"].as_str().unwrap(), "Oct 11 22:14:16");
    assert_eq!(second_line["host"].as_str().unwrap(), "server01");
    assert_eq!(second_line["prog"].as_str().unwrap(), "kernel");
    assert_eq!(second_line["pid"], serde_json::Value::Null); // No PID for kernel messages
    assert_eq!(second_line["msg"].as_str().unwrap(), "CPU0: Core temperature above threshold");
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
        "--filter", "msg.matches(\"Failed|reject\")",
        "--exec", "track_count(\"errors\"); track_unique(\"programs\", prog);",
        "--end", "print(`Total errors: ${tracked[\"errors\"]}, Programs: ${tracked[\"programs\"].len()}`);"
    ], input);
    assert_eq!(exit_code, 0, "syslog filtering should succeed");
    
    // Should find 3 error messages (2 failed passwords, 1 postfix reject)
    assert!(stdout.contains("Total errors: 3"), "Should count 3 error messages");
    assert!(stdout.contains("Programs: 2"), "Should identify 2 different programs (sshd, postfix)");
}

#[test]
fn test_syslog_severity_analysis() {
    let input = r#"<165>1 2023-10-11T22:14:15.003Z server01 sshd 1234 ID47 - Failed password for user alice
<30>1 2023-10-11T22:14:16.123Z server01 nginx 5678 - - Request processed successfully
<11>1 2023-10-11T22:14:17.456Z server01 postgres 2345 - - Database connection established"#;
    
    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "syslog",
        "--exec", "let sev_name = if severity == 5 { \"notice\" } else if severity == 6 { \"info\" } else if severity == 3 { \"error\" } else { \"other\" }; track_bucket(\"severities\", sev_name);",
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
    
    let (stdout, _stderr, exit_code) = run_kelora_with_file(&[
        "-f", "syslog",
        "--filter", "host == \"webserver\"",
        "-F", "jsonl"
    ], &syslog_content);
    assert_eq!(exit_code, 0, "syslog file processing should succeed");
    
    // Should only show entries from webserver host
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        if line.starts_with('{') {
            let parsed: serde_json::Value = serde_json::from_str(line).expect("Should be valid JSON");
            assert_eq!(parsed["host"].as_str().unwrap(), "webserver");
        }
    }
}