mod common;

use common::run_kelora_with_input;

#[test]
fn test_discover_json_profiles_nested_input_fields() {
    let input = r#"{"level":"info","user":{"name":"alice","roles":["admin","ops"]},"bytes":1536}
{"level":"error","user":{"name":"bob","roles":["ops"]},"bytes":2048,"extra":{"nested":{"x":1}}}"#;

    let (stdout, stderr, exit_code) =
        run_kelora_with_input(&["-f", "json", "--discover=json"], input);

    assert_eq!(exit_code, 0, "discover should succeed: {}", stderr);
    assert!(
        stderr.is_empty(),
        "discover json mode should not emit stderr on success: {}",
        stderr
    );

    let doc: serde_json::Value =
        serde_json::from_str(&stdout).expect("discover output should be valid json");
    assert_eq!(doc["total_events"], 2);
    assert_eq!(doc["flatten_depth_limit"], 3);
    assert_eq!(doc["flatten_depth_capped"], false);
    assert_eq!(doc["truncated"], false);

    let fields = doc["fields"].as_array().expect("fields should be an array");

    let names: Vec<&str> = fields
        .iter()
        .map(|field| {
            field["name"]
                .as_str()
                .expect("field name should be a string")
        })
        .collect();

    assert!(
        names.contains(&"level"),
        "should include top-level fields: {:?}",
        names
    );
    assert!(
        names.contains(&"user.name"),
        "should flatten nested maps: {:?}",
        names
    );
    assert!(
        names.contains(&"user.roles[]"),
        "should flatten array elements: {:?}",
        names
    );
    assert!(
        names.contains(&"extra.nested.x"),
        "should flatten nested fields up to depth limit: {:?}",
        names
    );

    let roles = fields
        .iter()
        .find(|field| field["name"] == "user.roles[]")
        .expect("user.roles[] field should exist");
    assert_eq!(
        roles["seen"], 3,
        "array entries should be counted individually"
    );
    assert_eq!(roles["cardinality"]["count"], 2);

    let bytes = fields
        .iter()
        .find(|field| field["name"] == "bytes")
        .expect("bytes field should exist");
    assert_eq!(bytes["samples"][0], 1536);
}

#[test]
fn test_discover_output_scope_profiles_post_filter_post_exec_fields() {
    let input = r#"{"level":"info","keep":true,"bytes":1536}
{"level":"error","keep":false,"bytes":2048}"#;

    let (stdout, stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--filter",
            "e.keep",
            "--exec",
            r#"e.pretty = human_bytes(e.bytes); e = e.keep(["level","pretty"])"#,
            "--discover=json",
            "--discover-scope=output",
        ],
        input,
    );

    assert_eq!(
        exit_code, 0,
        "discover output scope should succeed: {}",
        stderr
    );
    assert!(
        stderr.is_empty(),
        "discover output scope should not emit stderr on success: {}",
        stderr
    );

    let doc: serde_json::Value =
        serde_json::from_str(&stdout).expect("discover output should be valid json");
    assert_eq!(
        doc["total_events"], 1,
        "filter should run before output discovery"
    );

    let fields = doc["fields"].as_array().expect("fields should be an array");
    let names: Vec<&str> = fields
        .iter()
        .map(|field| {
            field["name"]
                .as_str()
                .expect("field name should be a string")
        })
        .collect();

    assert_eq!(names.len(), 2, "only projected output fields should remain");
    assert!(
        names.contains(&"level"),
        "output discovery should keep level"
    );
    assert!(
        names.contains(&"pretty"),
        "output discovery should see exec output"
    );
    assert!(
        !names.contains(&"bytes") && !names.contains(&"keep"),
        "output discovery should not report filtered/projection-only input fields: {:?}",
        names
    );

    let pretty = fields
        .iter()
        .find(|field| field["name"] == "pretty")
        .expect("pretty field should exist");
    assert_eq!(pretty["samples"][0], "1.5 KiB");
}

#[test]
fn test_discover_rejects_parallel_mode_at_cli_validation() {
    let input = r#"{"level":"info"}"#;

    let (stdout, stderr, exit_code) =
        run_kelora_with_input(&["-f", "json", "--parallel", "--discover"], input);

    assert_eq!(
        exit_code, 2,
        "parallel discover should be a CLI usage error"
    );
    assert!(
        stdout.is_empty(),
        "validation errors should not emit stdout"
    );
    assert!(
        stderr.contains("--discover is not supported with --parallel"),
        "expected clear validation message, got: {}",
        stderr
    );
}
