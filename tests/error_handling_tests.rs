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

#[test]
fn test_error_stats_sequential_mode() {
    let input = r#"{"valid": "json", "status": 200}
{malformed json line}
{"another": "valid", "status": 404}
not json at all
{"final": "entry", "status": 500}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(&["-f", "json", "--stats"], input);
    assert_eq!(
        exit_code, 1,
        "Sequential mode should return a non-zero exit status when parse errors occur"
    );

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 3, "Should emit only the valid JSON lines");

    let stats = extract_stats_lines(&stderr);
    assert_eq!(
        stats_line(&stats, "Lines processed:"),
        "Lines processed: 5 total, 0 filtered (0.0%), 0 errors (0.0%)"
    );
    assert_eq!(
        stats_line(&stats, "Events created:"),
        "Events created: 3 total, 3 output, 0 filtered (0.0%)"
    );
}

#[test]
fn test_error_stats_parallel_mode() {
    let input = r#"{"valid": "json", "status": 200}
{malformed json line}
{"another": "valid", "status": 404}
not json at all
{"final": "entry", "status": 500}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--stats", "--parallel", "--batch-size", "2"],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "Parallel mode should continue despite parse errors and report success"
    );

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 3, "Should emit only the valid JSON lines");

    let stats = extract_stats_lines(&stderr);
    assert_eq!(
        stats_line(&stats, "Lines processed:"),
        "Lines processed: 5 total, 0 filtered (0.0%), 0 errors (0.0%)"
    );
    assert_eq!(
        stats_line(&stats, "Events created:"),
        "Events created: 3 total, 3 output, 0 filtered (0.0%)"
    );
}

#[test]
fn test_error_stats_with_filter_expression() {
    let input = r#"{"valid": "json", "status": 200}
{malformed json line}
{"another": "valid", "status": 404}
not json at all
{"final": "entry", "status": 500}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.status >= 400", "--stats"],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "Filtering to valid events should still succeed despite parse errors"
    );

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should emit only events with status >= 400");

    let stats = extract_stats_lines(&stderr);
    assert_eq!(
        stats_line(&stats, "Lines processed:"),
        "Lines processed: 5 total, 0 filtered (0.0%), 0 errors (0.0%)"
    );
    assert_eq!(
        stats_line(&stats, "Events created:"),
        "Events created: 3 total, 2 output, 1 filtered (33.3%)"
    );
}

#[test]
fn test_error_stats_with_ignore_lines() {
    let input = r#"# This is a comment
{"valid": "json", "status": 200}
{malformed json line}
# Another comment
{"another": "valid", "status": 404}"#;

    let (stdout, stderr, exit_code) =
        run_kelora_with_input(&["-f", "json", "--ignore-lines", "^#", "--stats"], input);
    assert_eq!(
        exit_code, 1,
        "Ignoring comments still propagates parse errors in sequential mode"
    );

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should emit the two valid JSON lines");

    let stats = extract_stats_lines(&stderr);
    assert_eq!(
        stats_line(&stats, "Lines processed:"),
        "Lines processed: 5 total, 2 filtered (40.0%), 0 errors (0.0%)"
    );
    assert_eq!(
        stats_line(&stats, "Events created:"),
        "Events created: 2 total, 2 output, 0 filtered (0.0%)"
    );
}

#[test]
fn test_error_stats_no_errors() {
    let input = r#"{"valid": "json", "status": 200}
{"another": "valid", "status": 404}
{"final": "entry", "status": 500}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.status >= 400", "--stats"],
        input,
    );
    assert_eq!(exit_code, 0, "All-valid input should exit successfully");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should emit only the two matching events");

    let stats = extract_stats_lines(&stderr);
    assert_eq!(
        stats_line(&stats, "Lines processed:"),
        "Lines processed: 3 total, 0 filtered (0.0%), 0 errors (0.0%)"
    );
    assert_eq!(
        stats_line(&stats, "Events created:"),
        "Events created: 3 total, 2 output, 1 filtered (33.3%)"
    );
}

#[test]
fn test_error_stats_parallel_vs_sequential_consistency() {
    let input = r#"{"valid": "json", "status": 200}
{malformed json line}
{"another": "valid", "status": 404}
not json at all
{"final": "entry", "status": 500}
invalid json again"#;

    let (stdout_seq, stderr_seq, exit_code_seq) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.status >= 400", "--stats"],
        input,
    );
    let (stdout_par, stderr_par, exit_code_par) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--filter",
            "e.status >= 400",
            "--stats",
            "--parallel",
            "--batch-size",
            "2",
        ],
        input,
    );

    assert_eq!(
        exit_code_seq, 1,
        "Sequential mode should propagate parse errors via exit status"
    );
    assert_eq!(
        exit_code_par, 0,
        "Parallel mode should complete successfully in resilient mode"
    );

    let seq_lines: Vec<&str> = stdout_seq.trim().lines().collect();
    let par_lines: Vec<&str> = stdout_par.trim().lines().collect();
    assert_eq!(
        seq_lines.len(),
        par_lines.len(),
        "Sequential and parallel runs should emit the same number of events"
    );

    let stats_seq = extract_stats_lines(&stderr_seq);
    let stats_par = extract_stats_lines(&stderr_par);
    assert_eq!(
        stats_line(&stats_seq, "Lines processed:"),
        stats_line(&stats_par, "Lines processed:")
    );
    assert_eq!(
        stats_line(&stats_seq, "Events created:"),
        stats_line(&stats_par, "Events created:")
    );
    assert_eq!(
        stats_line(&stats_seq, "Lines processed:"),
        "Lines processed: 6 total, 0 filtered (0.0%), 0 errors (0.0%)"
    );
    assert_eq!(
        stats_line(&stats_seq, "Events created:"),
        "Events created: 3 total, 2 output, 1 filtered (33.3%)"
    );
}

#[test]
fn test_error_stats_multiline_mode() {
    let input = r#"{"valid": "json", "message": "line1\nline2"}
{malformed json line}
{"another": "valid", "message": "single line"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(&["-f", "json", "--stats"], input);
    assert_eq!(
        exit_code, 1,
        "Sequential mode should return an error when multiline input has parse failures"
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Should emit the two valid JSON events");

    let stats = extract_stats_lines(&stderr);
    assert_eq!(
        stats_line(&stats, "Lines processed:"),
        "Lines processed: 3 total, 0 filtered (0.0%), 0 errors (0.0%)"
    );
    assert_eq!(
        stats_line(&stats, "Events created:"),
        "Events created: 2 total, 2 output, 0 filtered (0.0%)"
    );

    let (_stdout_multi, stderr_multi, exit_code_multi) =
        run_kelora_with_input(&["-f", "json", "--multiline", "indent", "--stats"], input);
    assert_eq!(
        exit_code_multi, 1,
        "Multiline mode should still surface parse errors through the exit status"
    );
    let stats_multi = extract_stats_lines(&stderr_multi);
    assert_eq!(
        stats_line(&stats_multi, "Lines processed:"),
        "Lines processed: 3 total, 0 filtered (0.0%), 0 errors (0.0%)"
    );
    assert_eq!(
        stats_line(&stats_multi, "Events created:"),
        "Events created: 2 total, 2 output, 0 filtered (0.0%)"
    );
}
