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
