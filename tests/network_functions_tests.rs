mod common;
use common::*;

#[test]
fn test_is_ipv4() {
    let input = r#"{"ip": "192.168.1.1"}
{"ip": "10.0.0.1"}
{"ip": "2001:db8::1"}
{"ip": "not-an-ip"}
{"ip": ""}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "e.is_v4 = is_ipv4(e.ip);",
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Parse JSON output
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 5, "Should output 5 lines");

    let line1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(line1["is_v4"], true, "192.168.1.1 should be IPv4");

    let line2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(line2["is_v4"], true, "10.0.0.1 should be IPv4");

    let line3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(line3["is_v4"], false, "2001:db8::1 should not be IPv4");

    let line4: serde_json::Value = serde_json::from_str(lines[3]).unwrap();
    assert_eq!(line4["is_v4"], false, "not-an-ip should not be IPv4");

    let line5: serde_json::Value = serde_json::from_str(lines[4]).unwrap();
    assert_eq!(line5["is_v4"], false, "empty string should not be IPv4");
}

#[test]
fn test_is_ipv6() {
    let input = r#"{"ip": "2001:db8::1"}
{"ip": "::1"}
{"ip": "192.168.1.1"}
{"ip": "not-an-ip"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            "e.is_v6 = is_ipv6(e.ip);",
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Parse JSON output
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 4, "Should output 4 lines");

    let line1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(line1["is_v6"], true, "2001:db8::1 should be IPv6");

    let line2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(line2["is_v6"], true, "::1 should be IPv6");

    let line3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(line3["is_v6"], false, "192.168.1.1 should not be IPv6");

    let line4: serde_json::Value = serde_json::from_str(lines[3]).unwrap();
    assert_eq!(line4["is_v6"], false, "not-an-ip should not be IPv6");
}

#[test]
fn test_is_in_cidr_ipv4() {
    let input = r#"{"ip": "192.168.1.1"}
{"ip": "192.168.2.1"}
{"ip": "10.0.0.1"}
{"ip": "172.16.5.4"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            r#"e.in_subnet = is_in_cidr(e.ip, "192.168.1.0/24");"#,
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Parse JSON output
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 4, "Should output 4 lines");

    let line1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(
        line1["in_subnet"], true,
        "192.168.1.1 should match 192.168.1.0/24"
    );

    let line2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(
        line2["in_subnet"], false,
        "192.168.2.1 should not match 192.168.1.0/24"
    );

    let line3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(
        line3["in_subnet"], false,
        "10.0.0.1 should not match 192.168.1.0/24"
    );

    let line4: serde_json::Value = serde_json::from_str(lines[3]).unwrap();
    assert_eq!(
        line4["in_subnet"], false,
        "172.16.5.4 should not match 192.168.1.0/24"
    );
}

#[test]
fn test_is_in_cidr_ipv6() {
    let input = r#"{"ip": "2001:db8::1"}
{"ip": "2001:db9::1"}
{"ip": "fe80::1"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            r#"e.in_subnet = is_in_cidr(e.ip, "2001:db8::/32");"#,
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Parse JSON output
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should output 3 lines");

    let line1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(
        line1["in_subnet"], true,
        "2001:db8::1 should match 2001:db8::/32"
    );

    let line2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(
        line2["in_subnet"], false,
        "2001:db9::1 should not match 2001:db8::/32"
    );

    let line3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(
        line3["in_subnet"], false,
        "fe80::1 should not match 2001:db8::/32"
    );
}

#[test]
fn test_is_in_cidr_filter() {
    let input = r#"{"ip": "192.168.1.10"}
{"ip": "192.168.1.20"}
{"ip": "10.0.0.1"}
{"ip": "192.168.2.5"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--filter",
            r#"is_in_cidr(e.ip, "192.168.1.0/24")"#,
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Parse JSON output
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 2, "Should output 2 lines (only matching IPs)");

    let line1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(line1["ip"], "192.168.1.10");

    let line2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(line2["ip"], "192.168.1.20");
}

#[test]
fn test_is_in_cidr_classification() {
    let input = r#"{"ip": "192.168.1.5"}
{"ip": "10.0.5.1"}
{"ip": "8.8.8.8"}"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "json",
            "--exec",
            r#"e.network = if is_in_cidr(e.ip, "192.168.0.0/16") {
                "local"
            } else if is_in_cidr(e.ip, "10.0.0.0/8") {
                "internal"
            } else {
                "external"
            };"#,
            "-F",
            "json",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Parse JSON output
    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "Should output 3 lines");

    let line1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(line1["network"], "local", "192.168.1.5 should be 'local'");

    let line2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(
        line2["network"], "internal",
        "10.0.5.1 should be 'internal'"
    );

    let line3: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(line3["network"], "external", "8.8.8.8 should be 'external'");
}

#[test]
fn test_network_functions_with_combined_format() {
    let input = r#"192.168.1.100 - - [01/Jan/2024:12:00:00 +0000] "GET /index.html HTTP/1.1" 200 1234
10.0.0.50 - - [01/Jan/2024:12:00:01 +0000] "POST /api/data HTTP/1.1" 201 5678"#;

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "-f",
            "combined",
            "--exec",
            r#"e.is_private = is_in_cidr(e.ip, "192.168.0.0/16") || is_in_cidr(e.ip, "10.0.0.0/8");"#,
            "--filter",
            "e.is_private",
        ],
        input,
    );
    assert_eq!(exit_code, 0, "kelora should exit successfully");

    // Both IPs should pass the filter
    assert!(stdout.contains("192.168.1.100"));
    assert!(stdout.contains("10.0.0.50"));
    assert!(stdout.contains("is_private=true"));
}
