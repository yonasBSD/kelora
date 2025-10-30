mod common;
use common::*;

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
            "none",
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

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-F", "json"], input);
    assert_eq!(exit_code, 0, "Should exit successfully with json format");

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
            "none",
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
            "none",
        ],
        input,
    );
    assert_eq!(exit_code_line, 0, "Line format should exit successfully");
    let line_count = stdout_line.trim().split('\n').collect::<Vec<&str>>().len();
    assert_eq!(
        line_count, 4,
        "Line format should process 4 lines including empty ones"
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
            "none",
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
