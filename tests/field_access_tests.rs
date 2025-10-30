mod common;
use common::*;

#[test]
fn test_direct_field_access_basic_usage() {
    let input = r#"{"user": {"name": "alice", "age": 25, "scores": [100, 200, 300]}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "let name = e.user.name; print(\"Name: \" + name)",
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
fn test_direct_field_access_array_access() {
    let input = r#"{"user": {"name": "bob", "scores": [100, 200, 300]}}"#;

    let (stdout, _, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "let score = e.user.scores[1]; print(\"Second score: \" + score)",
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
fn test_direct_field_access_negative_indexing() {
    let input = r#"{"user": {"name": "charlie", "scores": [100, 200, 300]}}"#;

    let (stdout, _, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "let last_score = e.user.scores[-1]; print(\"Last score: \" + last_score)",
        ],
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
fn test_direct_field_access_deeply_nested() {
    let input = r#"{"data": {"items": [{"id": 1, "meta": {"tags": ["urgent", "review"]}}]}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "let tag = e.data.items[0].meta.tags[0]; print(\"First tag: \" + tag)",
        ],
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
fn test_direct_field_access_with_optional_chaining() {
    let input = r#"{"user": {"name": "david"}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "let age = if \"age\" in e.user { e.user.age } else { \"unknown\" }; print(\"Age: \" + age)",
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
fn test_direct_field_access_bounds_checking() {
    let input = r#"{"user": {"scores": [100, 200]}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "let score = if e.user.scores.len() > 99 { e.user.scores[99] } else { \"not_found\" }; print(\"Score: \" + score)",
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
fn test_direct_field_access_filtering() {
    let input = r#"{"level": "error", "user": {"role": "admin"}}
{"level": "info", "user": {"role": "user"}}
{"level": "error", "user": {"role": "user"}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "json", "--filter", "e.user.role == \"admin\""],
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
fn test_direct_field_access_with_real_world_log() {
    let input = r#"{"timestamp": "2023-01-01T10:00:00Z", "request": {"method": "GET", "url": "/api/users", "headers": {"user-agent": "Mozilla/5.0"}}, "response": {"status": 200, "size": 1024}}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "let method = e.request.method; \
           let status = e.response.status; \
           let user_agent = e.request.headers[\"user-agent\"]; \
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
