mod common;

use common::{run_kelora_with_files, run_kelora_with_input};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_auto_per_file_detects_each_file_independently() {
    let mut json_file = NamedTempFile::new().expect("create temp json file");
    writeln!(json_file, r#"{{"msg":"json-one","n":1}}"#).expect("write json line");
    writeln!(json_file, r#"{{"msg":"json-two","n":2}}"#).expect("write json line");

    let mut logfmt_file = NamedTempFile::new().expect("create temp logfmt file");
    writeln!(logfmt_file, "msg=logfmt-one level=info").expect("write logfmt line");
    writeln!(logfmt_file, "msg=logfmt-two level=warn").expect("write logfmt line");

    let json_path = json_file
        .path()
        .to_str()
        .expect("json path utf-8")
        .to_string();
    let logfmt_path = logfmt_file
        .path()
        .to_str()
        .expect("logfmt path utf-8")
        .to_string();
    let files = vec![json_path.as_str(), logfmt_path.as_str()];

    let (stdout, stderr, exit_code) =
        run_kelora_with_files(&["-f", "auto-per-file", "-F", "json"], &files);

    assert_eq!(exit_code, 0, "auto-per-file should succeed: {}", stderr);
    assert!(
        stdout.contains("\"msg\":\"json-one\""),
        "json file should parse as json: {}",
        stdout
    );
    assert!(
        stdout.contains("\"msg\":\"logfmt-one\"") && stdout.contains("\"level\":\"info\""),
        "logfmt file should parse as logfmt: {}",
        stdout
    );
}

#[test]
fn test_auto_per_file_initializes_csv_parser_per_file() {
    let mut csv_file = NamedTempFile::new().expect("create temp csv file");
    writeln!(csv_file, "name,age,city").expect("write csv header");
    writeln!(csv_file, "Alice,30,Berlin").expect("write csv row");
    writeln!(csv_file, "Bob,40,Hamburg").expect("write csv row");

    let mut json_file = NamedTempFile::new().expect("create temp json file");
    writeln!(json_file, r#"{{"msg":"json-ok"}}"#).expect("write json line");

    let csv_path = csv_file
        .path()
        .to_str()
        .expect("csv path utf-8")
        .to_string();
    let json_path = json_file
        .path()
        .to_str()
        .expect("json path utf-8")
        .to_string();
    let files = vec![csv_path.as_str(), json_path.as_str()];

    let (stdout, stderr, exit_code) =
        run_kelora_with_files(&["-f", "auto-per-file", "-F", "json"], &files);

    assert_eq!(exit_code, 0, "auto-per-file should succeed: {}", stderr);
    assert!(
        stdout.contains("\"name\":\"Alice\"") && stdout.contains("\"city\":\"Berlin\""),
        "csv rows should parse with initialized headers: {}",
        stdout
    );
    assert!(
        stdout.contains("\"msg\":\"json-ok\""),
        "json file should still parse after csv file: {}",
        stdout
    );
}

#[test]
fn test_auto_per_file_preserves_state_across_files() {
    let mut json_file_one = NamedTempFile::new().expect("create first temp json file");
    writeln!(json_file_one, r#"{{"id":1}}"#).expect("write json line");
    writeln!(json_file_one, r#"{{"id":2}}"#).expect("write json line");

    let mut json_file_two = NamedTempFile::new().expect("create second temp json file");
    writeln!(json_file_two, r#"{{"id":3}}"#).expect("write json line");
    writeln!(json_file_two, r#"{{"id":4}}"#).expect("write json line");

    let file_one_path = json_file_one
        .path()
        .to_str()
        .expect("first json path utf-8")
        .to_string();
    let file_two_path = json_file_two
        .path()
        .to_str()
        .expect("second json path utf-8")
        .to_string();
    let files = vec![file_one_path.as_str(), file_two_path.as_str()];

    let (stdout, stderr, exit_code) = run_kelora_with_files(
        &[
            "-f",
            "auto-per-file",
            "-F",
            "json",
            "--exec",
            r#"state["count"] = (state["count"] ?? 0) + 1; e.count = state["count"];"#,
        ],
        &files,
    );

    assert_eq!(exit_code, 0, "auto-per-file should succeed: {}", stderr);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 4, "expected four output events: {}", stdout);
    assert!(
        lines[0].contains("\"count\":1"),
        "first event count: {}",
        lines[0]
    );
    assert!(
        lines[1].contains("\"count\":2"),
        "second event count: {}",
        lines[1]
    );
    assert!(
        lines[2].contains("\"count\":3"),
        "third event count: {}",
        lines[2]
    );
    assert!(
        lines[3].contains("\"count\":4"),
        "fourth event count: {}",
        lines[3]
    );
}

#[test]
fn test_auto_per_file_rejected_in_parallel_mode() {
    let (_stdout, stderr, exit_code) =
        run_kelora_with_input(&["-f", "auto-per-file", "--parallel"], r#"{"msg":"hello"}"#);

    assert_ne!(exit_code, 0, "parallel auto-per-file should fail");
    assert!(
        stderr.contains("auto-per-file") && stderr.to_lowercase().contains("parallel"),
        "error should mention auto-per-file and parallel: {}",
        stderr
    );
}
