mod common;
use common::*;

#[test]
fn test_brief_output_mode() {
    let input = r#"{"level": "INFO", "message": "test message", "user": "alice"}
{"level": "ERROR", "message": "error occurred", "user": "bob"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "--brief"], input);
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

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-b"], input);
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
    let input = r#"{"ts": "2024-01-01T12:00:00Z", "level": "ERROR", "message": "Test message", "user": "alice", "status": 500}"#;

    let (stdout, _, exit_code) = run_kelora_with_input(&["-f", "json", "--core"], input);
    assert_eq!(exit_code, 0, "kelora should exit successfully with --core");

    // Should only contain core fields
    assert!(stdout.contains("ts="), "Should contain ts field");
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
    let input = r#"{"ts": "2024-01-01T12:00:00Z", "level": "ERROR", "message": "Test message", "user": "alice"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "-c"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with -c short flag"
    );

    // Should only contain core fields
    assert!(stdout.contains("ts="), "Should contain ts field");
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

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "--core"], input);
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
    let input = r#"{"ts": "2024-01-01T12:00:00Z", "level": "ERROR", "message": "Test message", "user": "alice", "status": 500}"#;

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "json", "--core", "--keys", "user,status"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with --core and --keys"
    );

    // Should contain both core fields and specified keys
    assert!(stdout.contains("ts="), "Should contain ts field");
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
    assert!(stdout.contains("ts="), "Should contain ts field");
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
            "json",
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

    let (stdout, _stderr, exit_code) = run_kelora_with_input(&["-f", "json", "--core"], input);
    assert_eq!(
        exit_code, 0,
        "kelora should exit successfully with multiple timestamp variants"
    );

    // Should include all timestamp field variants (current behavior: include all matching names)
    assert!(stdout.contains("ts="), "Should contain ts field");
    assert!(stdout.contains("ts="), "Should contain ts field");
    assert!(stdout.contains("time="), "Should contain time field");
    assert!(stdout.contains("level="), "Should contain level field");
    assert!(stdout.contains("message="), "Should contain message field");
    assert!(
        !stdout.contains("other="),
        "Should not contain non-core other field"
    );
}

#[test]
fn test_quiet_level_0_normal_output() {
    // Test normal mode (level 0) - shows everything
    let input = r#"{"level": "info", "message": "test"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--stats",
            "--exec",
            "print(\"Script output\")",
        ],
        input,
    );
    assert_eq!(exit_code, 0);

    // Should show event output
    assert!(stdout.contains("level='info'"));
    assert!(stdout.contains("message='test'"));

    // Should show script output
    assert!(stdout.contains("Script output"));

    // Should show stats
    assert!(stderr.contains("Stats"));
    assert!(stderr.contains("Lines processed"));
}

#[test]
fn test_quiet_level_1_suppress_diagnostics() {
    // Test quiet level 1 (-q) - suppress diagnostics but show events and script output
    let input = r#"{"level": "info", "message": "test"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--stats",
            "--exec",
            "print(\"Script output\")",
            "-q",
        ],
        input,
    );
    assert_eq!(exit_code, 0);

    // Should show event output
    assert!(stdout.contains("level='info'"));
    assert!(stdout.contains("message='test'"));

    // Should show script output
    assert!(stdout.contains("Script output"));

    // Should NOT show stats
    assert!(!stderr.contains("Stats"));
    assert!(!stderr.contains("Lines processed"));
}

#[test]
fn test_quiet_level_2_suppress_events() {
    // Test quiet level 2 (-qq) - suppress diagnostics and events but show script output
    let input = r#"{"level": "info", "message": "test"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--stats",
            "--exec",
            "print(\"Script output\")",
            "-qq",
        ],
        input,
    );
    assert_eq!(exit_code, 0);

    // Should NOT show event output
    assert!(!stdout.contains("level='info'"));
    assert!(!stdout.contains("message='test'"));

    // Should still show script output
    assert!(stdout.contains("Script output"));

    // Should NOT show stats
    assert!(!stderr.contains("Stats"));
    assert!(!stderr.contains("Lines processed"));
}

#[test]
fn test_quiet_level_3_suppress_all() {
    // Test quiet level 3 (-qqq) - suppress everything including script output
    let input = r#"{"level": "info", "message": "test"}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--stats",
            "--exec",
            "print(\"Script output\")",
            "-qqq",
        ],
        input,
    );
    assert_eq!(exit_code, 0);

    // Should NOT show event output
    assert!(!stdout.contains("level='info'"));
    assert!(!stdout.contains("message='test'"));

    // Should NOT show script output
    assert!(!stdout.contains("Script output"));

    // Should NOT show stats
    assert!(!stderr.contains("Stats"));
    assert!(!stderr.contains("Lines processed"));

    // Should have no output at all
    assert_eq!(stdout.trim(), "");
}
