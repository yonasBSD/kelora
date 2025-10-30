mod common;
use common::*;

#[test]
fn test_error_handling_resilient_mode() {
    let input = r#"{"level": "INFO", "status": 200}
invalid json line
{"level": "ERROR", "status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json"], input);
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
invalid json line
{"level": "ERROR", "status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-F", "json"], input);
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
        serde_json::from_str::<serde_json::Value>(line).unwrap_or_else(|_| {
            panic!("All output lines should be valid JSON, but got: '{}'", line)
        });
    }

    // In resilient mode, parsing errors are handled silently by skipping invalid lines
    // This behavior may or may not produce stderr output depending on implementation details
}

#[test]
fn test_error_handling_resilient_mixed_input() {
    let input = r#"{"valid": "json", "status": 200}
{malformed json line}
{"another": "valid", "status": 404}
not json at all
{"final": "entry", "status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-F", "json"], input);
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
invalid json line
{"level": "ERROR", "status": 500}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "--strict"], input);
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
fn test_quiet_levels_with_errors() {
    // Test that quiet levels still preserve exit codes for errors
    let input = r#"{"level": "info", "message": "test"}"#;

    // Test with a filter that would cause an error
    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--filter",
            "e.nonexistent.field == true",
            "--strict",
            "-qqq",
        ],
        input,
    );

    // Should have non-zero exit code due to error
    assert_ne!(exit_code, 0);

    // Should have no output in quiet mode
    assert_eq!(stdout.trim(), "");

    // In strict mode with -qqq, even error messages should be suppressed
    // but exit code should still indicate failure
}
