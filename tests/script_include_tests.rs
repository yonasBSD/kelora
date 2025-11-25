mod common;
use common::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn include_functionality_basic() {
    // Create include file with helper function
    let mut include_file = NamedTempFile::new().expect("Failed to create include file");
    include_file
        .write_all(b"fn double_value(x) { return x * 2; }")
        .expect("Failed to write include file");
    let include_path = include_file.path().to_str().unwrap();

    let input = r#"{"value": 5}
{"value": 10}
{"value": 15}"#;

    let args = vec![
        "-f",
        "json",
        "-I",
        include_path,
        "--exec",
        "e.doubled = double_value(e.value);",
        "--with-stats",
    ];

    let (stdout, stderr, exit_code) = run_kelora_with_input(&args, input);
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Check that the function was applied
    assert!(stdout.contains("doubled=10"), "Should contain doubled=10");
    assert!(stdout.contains("doubled=20"), "Should contain doubled=20");
    assert!(stdout.contains("doubled=30"), "Should contain doubled=30");

    // Check processing stats
    let processed = extract_events_created_from_stats(&stderr);
    assert_eq!(processed, 3, "Should process 3 events");
}

#[test]
fn include_functionality_parallel_compatibility() {
    // Create include file with helper function
    let mut include_file = NamedTempFile::new().expect("Failed to create include file");
    include_file
        .write_all(b"fn calculate_score(base, multiplier) { return base * multiplier + 100; }")
        .expect("Failed to write include file");
    let include_path = include_file.path().to_str().unwrap();

    let input = r#"{"base": 1, "mult": 2}
{"base": 3, "mult": 4}
{"base": 5, "mult": 6}
{"base": 7, "mult": 8}
{"base": 9, "mult": 10}"#;

    // Run sequential version
    let args_seq = vec![
        "-f",
        "json",
        "-I",
        include_path,
        "--exec",
        "e.score = calculate_score(e.base, e.mult);",
        "--with-stats",
    ];

    let (stdout_seq, stderr_seq, exit_code_seq) = run_kelora_with_input(&args_seq, input);
    assert_eq!(
        exit_code_seq, 0,
        "Sequential kelora should exit successfully"
    );

    // Run parallel version
    let args_par = vec![
        "-f",
        "json",
        "-I",
        include_path,
        "--exec",
        "e.score = calculate_score(e.base, e.mult);",
        "--parallel",
        "--with-stats",
    ];

    let (stdout_par, stderr_par, exit_code_par) = run_kelora_with_input(&args_par, input);
    assert_eq!(exit_code_par, 0, "Parallel kelora should exit successfully");

    // Extract and compare stats
    let processed_seq = extract_events_created_from_stats(&stderr_seq);
    let processed_par = extract_events_created_from_stats(&stderr_par);
    assert_eq!(
        processed_seq, processed_par,
        "Processed counts should match"
    );
    assert_eq!(processed_seq, 5, "Should process 5 events");

    // Verify that both outputs contain the same calculated scores
    let expected_scores = ["102", "112", "130", "156", "190"];
    for score in &expected_scores {
        assert!(
            stdout_seq.contains(&format!("score={}", score)),
            "Sequential output should contain score={}",
            score
        );
        assert!(
            stdout_par.contains(&format!("score={}", score)),
            "Parallel output should contain score={}",
            score
        );
    }
}

#[test]
fn include_multiple_files_with_parallel() {
    // Create first include file with math utilities
    let mut include1 = NamedTempFile::new().expect("Failed to create include1 file");
    include1
        .write_all(b"fn add(a, b) { return a + b; }")
        .expect("Failed to write include1 file");
    let include1_path = include1.path().to_str().unwrap();

    // Create second include file with validation utilities
    let mut include2 = NamedTempFile::new().expect("Failed to create include2 file");
    include2
        .write_all(b"fn is_positive(x) { return x > 0; }")
        .expect("Failed to write include2 file");
    let include2_path = include2.path().to_str().unwrap();

    let input = r#"{"a": 5, "b": 3}
{"a": -2, "b": 8}
{"a": 10, "b": -1}
{"a": 0, "b": 0}"#;

    // Test with multiple includes on different stages
    let args = vec![
        "-f",
        "json",
        "-I",
        include1_path,
        "--exec",
        "e.sum = add(e.a, e.b);",
        "-I",
        include2_path,
        "--exec",
        "e.is_positive = is_positive(e.sum);",
        "--parallel",
        "--with-stats",
    ];

    let (stdout, stderr, exit_code) = run_kelora_with_input(&args, input);
    if exit_code != 0 {
        eprintln!("STDERR: {}", stderr);
        eprintln!("STDOUT: {}", stdout);
    }
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Verify both includes worked across different stages
    assert!(stdout.contains("sum=8"), "Should contain sum=8 (5+3)");
    assert!(stdout.contains("sum=6"), "Should contain sum=6 (-2+8)");
    assert!(stdout.contains("sum=9"), "Should contain sum=9 (10-1)");
    assert!(stdout.contains("sum=0"), "Should contain sum=0 (0+0)");

    // Verify the second include function worked
    assert!(
        stdout.contains("is_positive=true"),
        "Should contain is_positive=true for positive sums"
    );
    assert!(
        stdout.contains("is_positive=false"),
        "Should contain is_positive=false for zero sum"
    );

    // Check that all 4 events were processed
    let processed = extract_events_created_from_stats(&stderr);
    assert_eq!(processed, 4, "Should process 4 events");
}
