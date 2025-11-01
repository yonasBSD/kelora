mod common;
use common::*;

#[test]
fn test_ilike_filters_unicode_case_fold() {
    let input = r#"{"message":"Timeout in service"}
{"message":"user timeout"}
{"message":"all good"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", r#"e.message.ilike("*timeout*")"#],
        input,
    );

    assert_eq!(exit_code, 0, "kelora should exit successfully: {stderr}");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "Two events should match ilike pattern");
    assert!(
        lines.iter().any(|line| line.contains("Timeout in service")),
        "Uppercase variant should match"
    );
    assert!(
        lines.iter().any(|line| line.contains("user timeout")),
        "Lowercase variant should match"
    );
}

#[test]
fn test_like_respects_case_and_anchoring() {
    let input = r#"{"message":"Timeout in service"}
{"message":"user timeout"}
{"message":"Timeout"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", r#"e.message.like("Timeout*")"#],
        input,
    );

    assert_eq!(exit_code, 0, "kelora should exit successfully: {stderr}");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "Only messages starting with 'Timeout' should match"
    );
    assert!(
        lines.iter().all(|line| line.contains("Timeout")),
        "All matched lines should start with Timeout"
    );
    assert!(
        !lines.iter().any(|line| line.contains("user timeout")),
        "Lowercase timeout should not match like()"
    );
}

#[test]
fn test_has_ignores_unit_values() {
    let input = r#"{"user":"alice","message":"active"}
{"user":null,"message":"cleared"}
{"message":"missing"}"#;

    let (stdout, stderr, exit_code) =
        run_kelora_with_input(&["-f", "json", "--filter", r#"e.has("user")"#], input);

    assert_eq!(exit_code, 0, "kelora should exit successfully: {stderr}");
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "Only the non-null user should survive the filter"
    );
    assert!(
        lines[0].contains("user='alice'"),
        "First event should remain"
    );
}

#[test]
fn test_matches_errors_on_invalid_regex() {
    let input = r#"{"message":"user not found"}"#;
    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", r#"e.message.matches("(")"#],
        input,
    );

    assert_ne!(
        exit_code, 0,
        "Invalid regex should cause non-zero exit code. Stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Invalid regex pattern"),
        "stderr should mention invalid regex: {stderr}"
    );
}
