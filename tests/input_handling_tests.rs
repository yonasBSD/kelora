mod common;
use common::*;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

#[test]
fn test_explicit_stdin_with_dash() {
    let input = r#"{"level": "info", "message": "test1"}
{"level": "error", "message": "test2"}
{"level": "info", "message": "test3"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-"], input);

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
    .args(["-f", "json", temp_file.path().to_str().unwrap(), "-"])
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
    let (stdout, stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-", "-"], "test");

    assert_ne!(exit_code, 0);
    assert!(stderr.contains("stdin (\"-\") can only be specified once"));
    assert!(stdout.is_empty());
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
            "json",
            "--filter",
            "e.status >= 400",
            "--exec",
            "track_count(\"errors\");",
            "--end",
            "print(`Errors: ${metrics[\"errors\"]}`);",
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
fn test_filename_tracking_json_sequential() {
    // Test filename tracking with JSON format in sequential mode
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
            "json",
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
fn test_filename_tracking_json_parallel() {
    // Test filename tracking with JSON format in parallel mode
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
            "json",
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
fn test_no_input_with_begin_only() {
    // Test --no-input with only --begin stage
    let (stdout, _stderr, exit_code) =
        run_kelora(&["--no-input", "--begin", "print(\"Hello, World!\")"]);

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Hello, World!"),
        "Should execute begin stage: {}",
        stdout
    );
}

#[test]
fn test_no_input_with_begin_and_end() {
    // Test --no-input with both --begin and --end stages
    let (stdout, _stderr, exit_code) = run_kelora(&[
        "--no-input",
        "--begin",
        "conf.counter = 0; for i in 0..5 { conf.counter += i; }",
        "--end",
        "print(`Sum: ${conf.counter}`)",
    ]);

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Sum: 10"),
        "Should execute begin and end stages: {}",
        stdout
    );
}

#[test]
fn test_no_input_conflicts_with_files() {
    // Test that --no-input conflicts with file arguments
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(b"test\n")
        .expect("Failed to write to temp file");

    let (stdout, stderr, exit_code) =
        run_kelora_with_files(&["--no-input"], &[temp_file.path().to_str().unwrap()]);

    assert_ne!(exit_code, 0, "Should fail with error");
    assert!(
        stderr.contains("--no-input cannot be used with input files"),
        "Should show conflict error: {}",
        stderr
    );
    assert!(stdout.is_empty());
}

#[test]
fn test_no_input_with_metrics() {
    // Test --no-input with metrics tracking in begin/end stages
    let (stdout, _stderr, exit_code) = run_kelora(&[
        "--no-input",
        "--begin",
        "for i in 0..10 { track_count(\"iterations\"); }",
        "--end",
        "print(`Total iterations: ${metrics[\"iterations\"]}`)",
    ]);

    assert_eq!(exit_code, 0, "Should exit successfully");
    assert!(
        stdout.contains("Total iterations: 10"),
        "Should track metrics across stages: {}",
        stdout
    );
}

#[test]
fn test_no_input_sequential_mode() {
    // Test --no-input in sequential mode (default)
    let (stdout, _stderr, exit_code) =
        run_kelora(&["--no-input", "--begin", "print(\"Sequential mode\")"]);

    assert_eq!(exit_code, 0, "Should work in sequential mode");
    assert!(stdout.contains("Sequential mode"));
}

#[test]
fn test_no_input_parallel_mode() {
    // Test --no-input with --parallel
    let (stdout, _stderr, exit_code) = run_kelora(&[
        "--no-input",
        "--parallel",
        "--begin",
        "print(\"Parallel mode\")",
    ]);

    assert_eq!(exit_code, 0, "Should work in parallel mode");
    assert!(stdout.contains("Parallel mode"));
}
