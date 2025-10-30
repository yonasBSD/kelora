mod common;
use common::*;

#[test]
fn test_section_from_with_before() {
    let input = "header line\n\
                 == Section A\n\
                 a1\n\
                 a2\n\
                 == Section B\n\
                 b1\n\
                 b2\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "--section-from",
            "^== Section A",
            "--section-before",
            "^==",
            "-f",
            "line",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Section A"));
    assert!(stdout.contains("a1"));
    assert!(stdout.contains("a2"));
    assert!(!stdout.contains("Section B"));
    assert!(!stdout.contains("b1"));
}

#[test]
fn test_section_from_only() {
    let input = "header\n\
                 == Section A\n\
                 a1\n\
                 == Section B\n\
                 b1\n\
                 footer\n";

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["--section-from", "^== Section B", "-f", "line"], input);

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Section B"));
    assert!(stdout.contains("b1"));
    assert!(stdout.contains("footer"));
    assert!(!stdout.contains("Section A"));
}

#[test]
fn test_max_sections_limit() {
    let input = "== Section 1\n\
                 s1\n\
                 == Section 2\n\
                 s2\n\
                 == Section 3\n\
                 s3\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &["--section-from", "^==", "--max-sections", "2", "-f", "line"],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Section 1"));
    assert!(stdout.contains("Section 2"));
    assert!(!stdout.contains("Section 3"));
    assert!(!stdout.contains("s3"));
}

#[test]
fn test_section_from_with_keep_lines() {
    let input = "header\n\
                 == Target Section\n\
                 ERROR: problem\n\
                 INFO: ok\n\
                 ERROR: another\n\
                 == Other Section\n\
                 ERROR: skip this\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "--section-from",
            "^== Target",
            "--section-before",
            "^==",
            "--keep-lines",
            "ERROR",
            "-f",
            "line",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("ERROR: problem"));
    assert!(stdout.contains("ERROR: another"));
    assert!(!stdout.contains("INFO: ok"));
    assert!(!stdout.contains("skip this"));
}

#[test]
fn test_section_from_with_json_format() {
    let input = "header\n\
                 == Important\n\
                 {\"level\":\"error\",\"msg\":\"failed\"}\n\
                 {\"level\":\"info\",\"msg\":\"ok\"}\n\
                 == Boring\n\
                 {\"level\":\"debug\",\"msg\":\"verbose\"}\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "--section-from",
            "^== Important",
            "--section-before",
            "^== Boring",
            "--ignore-lines",
            "^==",
            "-f",
            "json",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("failed"));
    assert!(!stdout.contains("verbose"));
}

#[test]
fn test_section_from_parallel_mode() {
    let input = "== Section A\n\
                 line1\n\
                 line2\n\
                 == Section B\n\
                 line3\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "--section-from",
            "^== Section A",
            "--section-before",
            "^==",
            "--parallel",
            "-f",
            "line",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Section A"));
    assert!(stdout.contains("line1"));
    assert!(stdout.contains("line2"));
    assert!(!stdout.contains("Section B"));
}

#[test]
fn test_section_after_excludes_marker() {
    let input = "== Section A\n\
                 line1\n\
                 == Section B\n\
                 line2\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "--section-after",
            "^== Section A",
            "--section-before",
            "^==",
            "-f",
            "line",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(!stdout.contains("Section A"));
    assert!(stdout.contains("line1"));
    assert!(!stdout.contains("Section B"));
}

#[test]
fn test_section_through_includes_end_line() {
    let input = "== Section A START\n\
                 body1\n\
                 == END SECTION\n\
                 tail\n";

    let (stdout, _stderr, exit_code) = run_kelora_with_input(
        &[
            "--section-from",
            "^== Section A",
            "--section-through",
            "^== END SECTION",
            "-f",
            "line",
        ],
        input,
    );

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("Section A START"));
    assert!(stdout.contains("body1"));
    assert!(stdout.contains("END SECTION")); // inclusive
    assert!(!stdout.contains("tail"));
}

#[test]
fn test_no_matching_section() {
    let input = "line1\nline2\nline3\n";

    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["--section-from", "^== NONEXISTENT", "-f", "line"], input);

    assert_eq!(exit_code, 0);
    assert!(stdout.is_empty());
}

#[test]
fn test_multiple_sections_unlimited() {
    let input = "== S1\n\
                 content1\n\
                 == S2\n\
                 content2\n\
                 == S3\n\
                 content3\n";

    // Default max-sections is -1 (unlimited)
    let (stdout, _stderr, exit_code) =
        run_kelora_with_input(&["--section-from", "^==", "-f", "line"], input);

    assert_eq!(exit_code, 0);
    assert!(stdout.contains("S1"));
    assert!(stdout.contains("S2"));
    assert!(stdout.contains("S3"));
    assert!(stdout.contains("content1"));
    assert!(stdout.contains("content2"));
    assert!(stdout.contains("content3"));
}
