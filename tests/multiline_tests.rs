mod common;
use common::*;

#[test]
fn test_multiline_real_world_scenario() {
    let input = r#"{"timestamp": "2023-07-18T15:04:23.456Z", "user": "alice", "status": 200, "message": "login successful", "response_time": 45}
{"timestamp": "2023-07-18T15:04:25.789Z", "user": "bob", "status": 404, "message": "page not found", "response_time": 12}
{"timestamp": "2023-07-18T15:06:41.210Z", "user": "charlie", "status": 500, "message": "internal error", "response_time": 234}
{"timestamp": "2023-07-18T15:07:12.345Z", "user": "alice", "status": 403, "message": "forbidden", "response_time": 18}
{"timestamp": "2023-07-18T15:08:30.678Z", "user": "dave", "status": 200, "message": "success", "response_time": 67}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "json",
        "-F", "json",
        "--filter", "e.status >= 400",
        "--exec", "e.alert_level = if e.status >= 500 { \"critical\" } else { \"warning\" }; track_count(\"total_errors\");",
        "--end", "print(`Total errors processed: ${metrics[\"total_errors\"]}`);"
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
fn test_multiline_all_strategy_json() {
    // Test reading entire JSON file as single event
    let input = r#"{"users": [
  {"name": "alice", "age": 30, "status": "active"},
  {"name": "bob", "age": 25, "status": "inactive"},
  {"name": "charlie", "age": 35, "status": "active"}
], "total": 3, "timestamp": "2023-07-18T15:00:00Z"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "json",
        "-M", "all",
        "-F", "json",
        "--exec", "e.user_count = e.users.len(); e.active_users = e.users.filter(|user| user.status == \"active\").len();"
    ], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully with -M all");

    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("Output should be valid JSON");

    // Verify the original data is preserved
    assert_eq!(parsed["total"].as_i64().unwrap(), 3);
    assert_eq!(parsed["users"].as_array().unwrap().len(), 3);

    // Verify our transformations worked
    assert_eq!(parsed["user_count"].as_i64().unwrap(), 3);
    assert_eq!(parsed["active_users"].as_i64().unwrap(), 2);
}

#[test]
fn test_multiline_all_strategy_text() {
    // Test reading entire text content as single event
    let input = r#"Line 1 with some content
Line 2 with more content
Line 3 with even more content
Final line of the document"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&[
        "-f", "raw",
        "-M", "all",
        "--exec", "let lines = e.raw.split(\"\\n\"); e.line_count = lines.len(); e.word_count = e.raw.split(\" \").len();"
    ], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with -M all on text"
    );

    // The output may be wrapped across multiple lines due to the long line content
    // The important thing is that we have exactly one event processed

    // The output should contain our transformations
    assert!(stdout.contains("line_count=4"), "Should count 4 lines");
    assert!(stdout.contains("word_count=18"), "Should count 18 words");

    // Verify the content is there (the long line with newlines)
    assert!(
        stdout.contains("Line 1 with some content\\nLine 2"),
        "Should contain the joined content with newlines"
    );
}

#[test]
fn test_multiline_all_strategy_empty_input() {
    // Test -M all with empty input
    let input = "";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "-M",
            "all",
            "--exec",
            "e.is_empty = e.line.len() == 0;",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should handle empty input with -M all");

    // With empty input, there should be no output events
    assert_eq!(
        stdout.trim(),
        "",
        "Should produce no output for empty input"
    );
}

#[test]
fn test_multiline_all_strategy_with_stats() {
    // Test -M all with stats enabled - using line format with shorter content
    let input = r#"Log 1
Log 2  
Log 3"#;

    let (_stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "-M",
            "all",
            "--stats",
            "--exec",
            "e.line_count = e.line.split(\"\\n\").len();",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with -M all and stats"
    );

    // Should create exactly 1 event (entire input as single event)
    assert!(
        stderr.contains("Events created: 1"),
        "Should create exactly 1 event"
    );
    assert!(stderr.contains("1 output"), "Should output exactly 1 event");
}

#[test]
fn test_multiline_indent_with_filters_and_stats() {
    let input = r#"ERROR connection failed
    at module.rs:42
    caused by network reset
WARN degraded performance
    while contacting replica
INFO recovered cleanly
"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "-M",
            "indent",
            "-F",
            "json",
            "--stats",
            "--filter",
            "e.line.contains(\"ERROR\") || e.line.contains(\"WARN\")",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with -M indent"
    );

    let events: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| serde_json::from_str(line).expect("Should parse JSON line"))
        .collect();

    assert_eq!(
        events.len(),
        2,
        "Filter should keep only ERROR and WARN events"
    );

    let first = events
        .first()
        .and_then(|event| event["line"].as_str())
        .expect("First event should contain a line field");
    assert!(
        first.contains("connection failed") && first.contains("module.rs:42"),
        "First event should contain the stack trace content"
    );

    let second = events
        .get(1)
        .and_then(|event| event["line"].as_str())
        .expect("Second event should contain a line field");
    assert!(
        second.contains("degraded performance") && second.contains("contacting replica"),
        "Second event should retain continuation lines"
    );

    let stats = extract_stats_lines(&stderr);
    assert!(
        !stats.is_empty(),
        "Stats output should be present when --stats is enabled"
    );
    assert_eq!(
        extract_events_created_from_stats(&stderr),
        3,
        "Three multiline events should be created before filtering"
    );
    assert_eq!(
        extract_events_filtered_from_stats(&stderr),
        1,
        "One event should be filtered out"
    );
}

#[test]
fn test_multiline_timestamp_with_format_hint_parallel_batches() {
    let input = r#"2023|07|18_15*04*23 INFO primary event
    stack line one
2023|07|18_15*04*24 INFO secondary event
    stack line two
2023|07|18_15*04*25 WARN final event
    last detail
"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "line",
            "-M",
            "timestamp:format=%Y|%m|%d_%H*%M*%S",
            "--parallel",
            "--batch-size",
            "1",
            "--batch-timeout",
            "1",
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with timestamp strategy"
    );

    let events: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| serde_json::from_str(line).expect("Should parse JSON line"))
        .collect();

    assert_eq!(
        events.len(),
        3,
        "Parallel batches should not split multiline events"
    );

    let first_line = events[0]["line"]
        .as_str()
        .expect("First event should contain aggregated text");
    assert!(
        first_line.contains("primary event") && first_line.contains("stack line one"),
        "First event should include both header and continuation text"
    );

    let second_line = events[1]["line"]
        .as_str()
        .expect("Second event should contain aggregated text");
    assert!(
        second_line.contains("secondary event") && second_line.contains("stack line two"),
        "Second event should keep its continuation line"
    );

    let third_line = events[2]["line"]
        .as_str()
        .expect("Third event should contain aggregated text");
    assert!(
        third_line.contains("final event") && third_line.contains("last detail"),
        "Third event should retain trailing detail lines"
    );
}

#[test]
fn test_multiline_regex_with_start_and_end_patterns() {
    let input = r#"START request 1
payload line a
payload line b
END
START request 2
payload line c
END
"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "raw",
            "-M",
            "regex:match=^START:end=^END",
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with regex mode"
    );

    let events: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| serde_json::from_str(line).expect("Should parse JSON line"))
        .collect();

    assert_eq!(events.len(), 2, "Expected two regex-delimited events");

    let first = events[0]["raw"]
        .as_str()
        .expect("Regex event should retain raw text");
    assert!(
        first.contains("START request 1")
            && first.contains("payload line b")
            && first.contains("END"),
        "Regex end pattern should keep the terminating line in the event"
    );

    let second = events[1]["raw"]
        .as_str()
        .expect("Regex event should retain raw text");
    assert!(
        second.contains("START request 2")
            && second.contains("payload line c")
            && second.ends_with("END"),
        "Second regex section should flush cleanly at END"
    );
}

#[test]
fn test_multiline_regex_invalid_pattern_surfaces_error() {
    let (_stdout, stderr, exit_code) =
        run_kelora_with_input(&["-f", "raw", "-M", "regex:match=[", "-F", "json"], "");

    assert_eq!(
        exit_code, 1,
        "Invalid regex configuration should propagate as an error"
    );
    assert!(
        stderr.contains("Invalid regex start pattern"),
        "Error output should mention the invalid regex start pattern"
    );
}
