// tests/type_annotation_integration_test.rs
use std::io::Write;
use std::process::{Command, Stdio};

/// Helper function to run kelora with given arguments and input via stdin
fn run_kelora_with_input(args: &[&str], input: &str) -> (String, String, i32) {
    let binary_path = if cfg!(debug_assertions) {
        "./target/debug/kelora"
    } else {
        "./target/release/kelora"
    };

    let mut cmd = Command::new(binary_path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start kelora");

    if let Some(stdin) = cmd.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .expect("Failed to write to stdin");
    }

    let output = cmd.wait_with_output().expect("Failed to read output");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn test_csv_type_annotations() {
    let input = "status,bytes,active\n200,1024,true\n404,512,false\n500,2048,yes";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "csv status:int bytes:int active:bool", "-F", "json"],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should output 3 lines");

    // Parse first line - should have integer and boolean types
    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("Should parse JSON");
    assert_eq!(first["status"].as_i64().unwrap(), 200);
    assert_eq!(first["bytes"].as_i64().unwrap(), 1024);
    assert!(first["active"].as_bool().unwrap());

    // Parse second line
    let second: serde_json::Value = serde_json::from_str(lines[1]).expect("Should parse JSON");
    assert_eq!(second["status"].as_i64().unwrap(), 404);
    assert_eq!(second["bytes"].as_i64().unwrap(), 512);
    assert!(!second["active"].as_bool().unwrap());

    // Parse third line - "yes" should convert to true
    let third: serde_json::Value = serde_json::from_str(lines[2]).expect("Should parse JSON");
    assert_eq!(third["status"].as_i64().unwrap(), 500);
    assert_eq!(third["bytes"].as_i64().unwrap(), 2048);
    assert!(third["active"].as_bool().unwrap());
}

#[test]
fn test_csv_type_annotations_with_float() {
    let input = "price,quantity,total\n10.5,3,31.5\n25.99,2,51.98";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "csv price:float quantity:int total:float",
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0);

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2);

    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("Should parse JSON");
    assert!((first["price"].as_f64().unwrap() - 10.5).abs() < 0.01);
    assert_eq!(first["quantity"].as_i64().unwrap(), 3);
    assert!((first["total"].as_f64().unwrap() - 31.5).abs() < 0.01);
}

#[test]
fn test_tsv_type_annotations() {
    let input = "status\tbytes\tactive\n200\t1024\ttrue\n404\t512\tfalse";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "tsv status:int bytes:int active:bool", "-F", "json"],
        input,
    );
    assert_eq!(exit_code, 0);

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2);

    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("Should parse JSON");
    assert_eq!(first["status"].as_i64().unwrap(), 200);
    assert_eq!(first["bytes"].as_i64().unwrap(), 1024);
    assert!(first["active"].as_bool().unwrap());
}

#[test]
fn test_cols_type_annotations() {
    let input = "200 1024 true hello world\n404 512 false error occurred";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "cols:status:int bytes:int active:bool *msg",
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0);

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2);

    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("Should parse JSON");
    assert_eq!(first["status"].as_i64().unwrap(), 200);
    assert_eq!(first["bytes"].as_i64().unwrap(), 1024);
    assert!(first["active"].as_bool().unwrap());
    assert_eq!(first["msg"].as_str().unwrap(), "hello world");

    let second: serde_json::Value = serde_json::from_str(lines[1]).expect("Should parse JSON");
    assert_eq!(second["status"].as_i64().unwrap(), 404);
    assert_eq!(second["bytes"].as_i64().unwrap(), 512);
    assert!(!second["active"].as_bool().unwrap());
    assert_eq!(second["msg"].as_str().unwrap(), "error occurred");
}

#[test]
fn test_cols_type_annotations_with_separator() {
    let input = "2025-09-22|12:33:44|200|hello|world";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "cols:ts(2) level:int *msg:string",
            "--cols-sep",
            "|",
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0);

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 1);

    let parsed: serde_json::Value = serde_json::from_str(lines[0]).expect("Should parse JSON");
    assert_eq!(parsed["ts"].as_str().unwrap(), "2025-09-22|12:33:44");
    assert_eq!(parsed["level"].as_i64().unwrap(), 200);
    assert_eq!(parsed["msg"].as_str().unwrap(), "hello|world");
}

#[test]
fn test_type_conversion_resilient_mode() {
    // In resilient mode, invalid conversions should fallback to string
    let input = "status,bytes\n200,1024\ninvalid,not_a_number";

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["-f", "csv status:int bytes:int", "-F", "json"], input);
    assert_eq!(exit_code, 0);

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2);

    // First line should have proper integers
    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("Should parse JSON");
    assert_eq!(first["status"].as_i64().unwrap(), 200);
    assert_eq!(first["bytes"].as_i64().unwrap(), 1024);

    // Second line should fallback to strings in resilient mode
    let second: serde_json::Value = serde_json::from_str(lines[1]).expect("Should parse JSON");
    assert_eq!(second["status"].as_str().unwrap(), "invalid");
    assert_eq!(second["bytes"].as_str().unwrap(), "not_a_number");
}

#[test]
fn test_mixed_type_annotations() {
    // Mix of annotated and non-annotated fields
    let input = "status,message,bytes\n200,success,1024\n404,not found,512";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["-f", "csv status:int message bytes:int", "-F", "json"],
        input,
    );
    assert_eq!(exit_code, 0);

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2);

    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("Should parse JSON");
    assert_eq!(first["status"].as_i64().unwrap(), 200);
    assert_eq!(first["message"].as_str().unwrap(), "success"); // No type annotation, stays string
    assert_eq!(first["bytes"].as_i64().unwrap(), 1024);
}
