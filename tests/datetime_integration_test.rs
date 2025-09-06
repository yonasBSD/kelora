use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn test_datetime_parsing_integration() {
    // Create a temporary file with some test data
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"{{"timestamp": "2023-07-04T12:34:56Z", "message": "Test message"}}"#
    )
    .unwrap();
    temp_file.flush().unwrap();

    // Run kelora with datetime parsing
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("-f")
        .arg("json")
        .arg(temp_file.path())
        .arg("--exec")
        .arg("let dt = parse_ts(e.timestamp); let year = dt.year(); if year == 2023 { print(\"YEAR_MATCH\") }")
        .output()
        .expect("Failed to run kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("YEAR_MATCH"),
        "Expected YEAR_MATCH in output. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_duration_parsing_integration() {
    // Create a temporary file with some test data
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"{{"duration": "1h 30m", "operation": "backup"}}"#
    )
    .unwrap();
    temp_file.flush().unwrap();

    // Run kelora with duration parsing
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("-f")
        .arg("json")
        .arg(temp_file.path())
        .arg("--exec")
        .arg("let dur = parse_dur(e.duration); let minutes = dur.as_minutes(); if minutes == 90 { print(\"DURATION_MATCH\") }")
        .output()
        .expect("Failed to run kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("DURATION_MATCH"),
        "Expected DURATION_MATCH in output: {}",
        stdout
    );
}

#[test]
fn test_datetime_arithmetic_integration() {
    // Create a temporary file with some test data
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"{{"start": "2023-07-04T12:00:00Z", "end": "2023-07-04T13:30:00Z"}}"#
    )
    .unwrap();
    temp_file.flush().unwrap();

    // Run kelora with datetime arithmetic
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("-f")
        .arg("json")
        .arg(temp_file.path())
        .arg("--exec")
        .arg("let start_dt = parse_ts(e.start); let end_dt = parse_ts(e.end); let diff = end_dt - start_dt; let minutes = diff.as_minutes(); if minutes == 90 { print(\"ARITHMETIC_MATCH\") }")
        .output()
        .expect("Failed to run kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ARITHMETIC_MATCH"),
        "Expected ARITHMETIC_MATCH in output: {}",
        stdout
    );
}

#[test]
fn test_current_time_functions_integration() {
    // Create a temporary file with some test data
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, r#"{{"message": "Current time test"}}"#).unwrap();
    temp_file.flush().unwrap();

    // Run kelora with current time functions
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("-f")
        .arg("json")
        .arg(temp_file.path())
        .arg("--exec")
        .arg("let now = now_utc(); let year = now.year(); if year >= 2023 { print(\"NOW_MATCH\") }")
        .output()
        .expect("Failed to run kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("NOW_MATCH"),
        "Expected NOW_MATCH in output. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_datetime_formatting_integration() {
    // Create a temporary file with some test data
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"{{"timestamp": "2023-07-04T12:34:56Z", "message": "Test message"}}"#
    )
    .unwrap();
    temp_file.flush().unwrap();

    // Run kelora with datetime formatting
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("-f")
        .arg("json")
        .arg(temp_file.path())
        .arg("--exec")
        .arg("let dt = parse_ts(e.timestamp); let formatted = dt.format(\"%Y-%m-%d\"); if formatted == \"2023-07-04\" { print(\"FORMAT_MATCH\") }")
        .output()
        .expect("Failed to run kelora");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("FORMAT_MATCH"),
        "Expected FORMAT_MATCH in output: {}",
        stdout
    );
}
