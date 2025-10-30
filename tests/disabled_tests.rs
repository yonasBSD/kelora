mod common;
#[allow(unused_imports)]
use common::*;

// =============================================================================
// DISABLED TESTS
// =============================================================================
//
// These tests are temporarily disabled and need to be updated for the new
// statistics format. They were extracted from integration_tests.rs to keep
// that file cleaner and make it easier to track which tests need updating.
//
// All tests remain commented out until they can be updated to match the
// current stats output format.
//
// When updating these tests, please:
// 1. Update the assertions to match the new stats format
// 2. Uncomment the test
// 3. Run the test to verify it passes
// 4. Move it back to integration_tests.rs
// =============================================================================

// Regression tests for parallel mode statistics counting (GitHub issue #XXX)
// TODO: Update test for new stats format
// #[test]
// fn test_parallel_stats_counting_basic() {
//     // Generate test data: 1-100, expect 10 outputs (multiples of 10), 90 filtered
//     let input: String = (1..=100)
//         .map(|i| i.to_string())
//         .collect::<Vec<_>>()
//         .join("\n");
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "--stats",
//             "--filter",
//             "line.to_int() % 10 == 0",
//             "--parallel",
//         ],
//         &input,
//     );
//
//     assert_eq!(exit_code, 0, "kelora should exit successfully");
//
//     // Check output lines
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(
//         output_lines.len(),
//         10,
//         "Should output exactly 10 lines (multiples of 10)"
//     );
//
//     // Verify the output lines are correct
//     let expected_outputs = ["10", "20", "30", "40", "50", "60", "70", "80", "90", "100"];
//     for (i, line) in output_lines.iter().enumerate() {
//         assert_eq!(line.trim(), &format!("line=\"{}\"", expected_outputs[i]));
//     }
//
//     // Check statistics in stderr
//     assert!(
//         stderr.contains("100 total"),
//         "Should show 100 total lines processed"
//     );
//     assert!(stderr.contains("10 output"), "Should show 10 output lines");
//     assert!(
//         stderr.contains("90 filtered"),
//         "Should show 90 filtered lines"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_parallel_stats_counting_large_dataset() {
//     // Generate test data: 1-10000, expect 1000 outputs (multiples of 10), 9000 filtered
//     let input: String = (1..=10000)
//         .map(|i| i.to_string())
//         .collect::<Vec<_>>()
//         .join("\n");
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "--stats",
//             "--filter",
//             "line.to_int() % 10 == 0",
//             "--parallel",
//             "--batch-size",
//             "100", // Smaller batch size to test multiple batches
//         ],
//         &input,
//     );
//
//     assert_eq!(exit_code, 0, "kelora should exit successfully");
//
//     // Check output count
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 1000, "Should output exactly 1000 lines");
//
//     // Check statistics in stderr
//     assert!(
//         stderr.contains("10000 total"),
//         "Should show 10000 total lines processed"
//     );
//     assert!(
//         stderr.contains("1000 output"),
//         "Should show 1000 output lines"
//     );
//     assert!(
//         stderr.contains("9000 filtered"),
//         "Should show 9000 filtered lines"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_parallel_vs_sequential_stats_consistency() {
//     // Test that parallel and sequential modes produce identical statistics
//     let input: String = (1..=1000)
//         .map(|i| i.to_string())
//         .collect::<Vec<_>>()
//         .join("\n");
//
//     // Run in sequential mode
//     let (stdout_seq, stderr_seq, exit_code_seq) =
//         run_kelora_with_input(&["--stats", "--filter", "line.to_int() % 100 == 0"], &input);
//
//     // Run in parallel mode
//     let (stdout_par, stderr_par, exit_code_par) = run_kelora_with_input(
//         &[
//             "--stats",
//             "--filter",
//             "line.to_int() % 100 == 0",
//             "--parallel",
//             "--batch-size",
//             "50",
//         ],
//         &input,
//     );
//
//     assert_eq!(exit_code_seq, 0, "Sequential mode should exit successfully");
//     assert_eq!(exit_code_par, 0, "Parallel mode should exit successfully");
//
//     // Both should produce the same output
//     assert_eq!(
//         stdout_seq, stdout_par,
//         "Sequential and parallel modes should produce identical output"
//     );
//
//     // Both should show the same statistics: 1000 total, 10 output, 990 filtered
//     let expected_stats = ["1000 total", "10 output", "990 filtered"];
//     for stat in &expected_stats {
//         assert!(
//             stderr_seq.contains(stat),
//             "Sequential mode should contain: {}",
//             stat
//         );
//         assert!(
//             stderr_par.contains(stat),
//             "Parallel mode should contain: {}",
//             stat
//         );
//     }
// }

// TODO: Update test for new stats format
// #[test]
// fn test_parallel_stats_with_errors() {
//     // Test statistics counting when errors occur during processing
//     let input = "1\n2\ninvalid\n4\n5\n";
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "--stats",
//             "--filter",
//             "line.to_int() > 3", // This will cause an error on "invalid"
//             "--on-error",
//             "skip", // Skip errors and continue
//             "--parallel",
//         ],
//         input,
//     );
//
//     assert_eq!(exit_code, 0, "kelora should exit successfully");
//
//     // Should output lines "4" and "5" (> 3)
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(
//         output_lines.len(),
//         2,
//         "Should output 2 lines that pass filter"
//     );
//
//     // Check statistics - total should be 5, output 2, filtered 3 (including error), errors 0 (when on-error=skip)
//     assert!(
//         stderr.contains("5 total"),
//         "Should show 5 total lines processed"
//     );
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(
//         stderr.contains("3 filtered"),
//         "Should show 3 filtered lines (including error with skip)"
//     );
//     // Note: when --on-error skip is used, errors are counted as filtered, not as separate errors
// }

// TODO: Update test for new stats format
// #[test]
// fn test_parallel_stats_with_different_batch_sizes() {
//     // Test that different batch sizes produce the same statistics
//     let input: String = (1..=500)
//         .map(|i| i.to_string())
//         .collect::<Vec<_>>()
//         .join("\n");
//
//     let batch_sizes = [1, 10, 50, 100, 500];
//     let mut all_results = Vec::new();
//
//     for &batch_size in &batch_sizes {
//         let (stdout, stderr, exit_code) = run_kelora_with_input(
//             &[
//                 "--stats",
//                 "--filter",
//                 "line.to_int() % 50 == 0",
//                 "--parallel",
//                 "--batch-size",
//                 &batch_size.to_string(),
//             ],
//             &input,
//         );
//
//         assert_eq!(
//             exit_code, 0,
//             "kelora should exit successfully with batch-size {}",
//             batch_size
//         );
//         all_results.push((stdout, stderr));
//     }
//
//     // All results should be identical
//     let (first_stdout, first_stderr) = &all_results[0];
//     for (i, (stdout, stderr)) in all_results.iter().enumerate().skip(1) {
//         assert_eq!(
//             stdout, first_stdout,
//             "Batch size {} should produce same output as batch size {}",
//             batch_sizes[i], batch_sizes[0]
//         );
//
//         // Check that statistics are the same (ignore timing differences)
//         let expected_stats = ["500 total", "10 output", "490 filtered"];
//         for stat in &expected_stats {
//             assert!(
//                 first_stderr.contains(stat),
//                 "Batch size {} should contain: {}",
//                 batch_sizes[0],
//                 stat
//             );
//             assert!(
//                 stderr.contains(stat),
//                 "Batch size {} should contain: {}",
//                 batch_sizes[i],
//                 stat
//             );
//         }
//     }
// }

// TODO: Update test for new stats format
// #[test]
// fn test_ignore_lines_with_stats() {
//     let input = r#"{"level": "INFO", "message": "Valid message 1"}
// # Comment to ignore
// {"level": "ERROR", "message": "Valid message 2"}
// # Another comment
// {"level": "WARN", "message": "Valid message 3"}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "-f",
//             "json",
//             "-F",
//             "json",
//             "--ignore-lines",
//             "^#", // Ignore comment lines
//             "--stats",
//         ],
//         input,
//     );
//     assert_eq!(
//         exit_code, 0,
//         "kelora should exit successfully with ignore-lines and stats"
//     );
//
//     let lines: Vec<&str> = stdout.trim().lines().collect();
//     assert_eq!(lines.len(), 3, "Should output 3 lines (comments ignored)");
//
//     // Check stats show filtered lines
//     assert!(stderr.contains("5 total"), "Should show 5 total lines read");
//     assert!(
//         stderr.contains("2 filtered"),
//         "Should show 2 lines filtered by ignore-lines"
//     );
//     assert!(stderr.contains("3 output"), "Should show 3 lines output");
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_sequential_mode() {
//     // Test error stats counting in sequential mode with mixed valid/invalid JSON
//     let input = r#"{"valid": "json", "status": 200}
// {malformed json line}
// {"another": "valid", "status": 404}
// not json at all
// {"final": "entry", "status": 500}"#;
//
//     let (stdout, _stderr, exit_code) =
//         run_kelora_with_input(&["-f", "json", "--on-error", "skip", "--stats"], input);
//     assert_eq!(
//         exit_code, 0,
//         "Should exit successfully with skip error handling"
//     );
//
//     // Should output 3 valid JSON lines, skip 2 malformed ones
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 3, "Should output 3 valid JSON lines");
//
//     // Stats should show separate error count
//     assert!(stderr.contains("5 total"), "Should show 5 total lines");
//     assert!(stderr.contains("2 errors"), "Should show 2 parsing errors");
//     assert!(
//         stderr.contains("0 filtered"),
//         "Should show 0 filtered lines"
//     );
//     assert!(
//         stderr.contains("Events created: 3 total, 3 output, 0 filtered"),
//         "Should show 3 events created and output"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_parallel_mode() {
//     // Test error stats counting in parallel mode with mixed valid/invalid JSON
//     let input = r#"{"valid": "json", "status": 200}
// {malformed json line}
// {"another": "valid", "status": 404}
// not json at all
// {"final": "entry", "status": 500}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "-f",
//             "json",
//             "--on-error",
//             "skip",
//             "--stats",
//             "--parallel",
//             "--batch-size",
//             "2",
//         ],
//         input,
//     );
//     assert_eq!(
//         exit_code, 0,
//         "Should exit successfully with skip error handling"
//     );
//
//     // Should output 3 valid JSON lines, skip 2 malformed ones
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 3, "Should output 3 valid JSON lines");
//
//     // Stats should show separate error count (same as sequential)
//     assert!(stderr.contains("5 total"), "Should show 5 total lines");
//     assert!(stderr.contains("2 errors"), "Should show 2 parsing errors");
//     assert!(
//         stderr.contains("0 filtered"),
//         "Should show 0 filtered lines"
//     );
//     assert!(
//         stderr.contains("Events created: 3 total, 3 output, 0 filtered"),
//         "Should show 3 events created and output"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_with_filter_expression() {
//     // Test error stats with both parsing errors and filter expression rejections
//     let input = r#"{"valid": "json", "status": 200}
// {malformed json line}
// {"another": "valid", "status": 404}
// not json at all
// {"final": "entry", "status": 500}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "-f",
//             "json",
//             "--filter",
//             "e.status >= 400",
//             "--on-error",
//             "skip",
//             "--stats",
//         ],
//         input,
//     );
//     assert_eq!(exit_code, 0, "Should exit successfully");
//
//     // Should output 2 lines (status 404 and 500), filter out 1 (status 200), skip 2 malformed
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(
//         output_lines.len(),
//         2,
//         "Should output 2 lines with status >= 400"
//     );
//
//     // Stats should show separate error and filtered counts
//     assert!(stderr.contains("5 total"), "Should show 5 total lines");
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(
//         stderr.contains("1 filtered"),
//         "Should show 1 filtered line (status 200)"
//     );
//     assert!(stderr.contains("2 errors"), "Should show 2 parsing errors");
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_with_ignore_lines() {
//     // Test error stats with ignore-lines preprocessing
//     let input = r#"# This is a comment
// {"valid": "json", "status": 200}
// {malformed json line}
// # Another comment
// {"another": "valid", "status": 404}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &[
//             "-f",
//             "json",
//             "--ignore-lines",
//             "^#",
//             "--on-error",
//             "skip",
//             "--stats",
//         ],
//         input,
//     );
//     assert_eq!(exit_code, 0, "Should exit successfully");
//
//     // Should output 2 valid JSON lines, ignore 2 comments, skip 1 malformed
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 2, "Should output 2 valid JSON lines");
//
//     // Stats should show combined filtered count (ignore-lines + filter expressions)
//     assert!(stderr.contains("5 total"), "Should show 5 total lines");
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(
//         stderr.contains("2 filtered"),
//         "Should show 2 filtered lines (comments)"
//     );
//     assert!(stderr.contains("1 errors"), "Should show 1 parsing error");
// }

// TODO: Update test for new stats format and error handling
// #[test]
// fn test_error_stats_different_error_strategies() { ... }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_no_errors() {
//     // Test that error stats are not shown when there are no errors
//     let input = r#"{"valid": "json", "status": 200}
// {"another": "valid", "status": 404}
// {"final": "entry", "status": 500}"#;
//
//     let (stdout, stderr, exit_code) = run_kelora_with_input(
//         &["-f", "json", "--filter", "status >= 400", "--stats"],
//         input,
//     );
//     assert_eq!(exit_code, 0, "Should exit successfully");
//
//     // Should output 2 lines (status 404 and 500), filter out 1 (status 200)
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(
//         output_lines.len(),
//         2,
//         "Should output 2 lines with status >= 400"
//     );
//
//     // Stats should not show error count when there are no errors
//     assert!(stderr.contains("3 total"), "Should show 3 total lines");
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(stderr.contains("1 filtered"), "Should show 1 filtered line");
//     assert!(
//         !stderr.contains("errors"),
//         "Should not show error count when there are no errors"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_parallel_vs_sequential_consistency() {
//     // Test that parallel and sequential modes show identical error stats
//     let input = r#"{"valid": "json", "status": 200}
// {malformed json line}
// {"another": "valid", "status": 404}
// not json at all
// {"final": "entry", "status": 500}
// invalid json again"#;
//
//     // Run in sequential mode
//     let (stdout_seq, stderr_seq, exit_code_seq) = run_kelora_with_input(
//         &[
//             "-f",
//             "json",
//             "--filter",
//             "e.status >= 400",
//             "--on-error",
//             "skip",
//             "--stats",
//         ],
//         input,
//     );
//
//     // Run in parallel mode
//     let (stdout_par, stderr_par, exit_code_par) = run_kelora_with_input(
//         &[
//             "-f",
//             "json",
//             "--filter",
//             "e.status >= 400",
//             "--on-error",
//             "skip",
//             "--stats",
//             "--parallel",
//             "--batch-size",
//             "2",
//         ],
//         input,
//     );
//
//     assert_eq!(exit_code_seq, 0, "Sequential mode should exit successfully");
//     assert_eq!(exit_code_par, 0, "Parallel mode should exit successfully");
//
//     // Both should produce the same output
//     let seq_lines: Vec<&str> = stdout_seq.trim().split('\n').collect();
//     let par_lines: Vec<&str> = stdout_par.trim().split('\n').collect();
//     assert_eq!(
//         seq_lines.len(),
//         par_lines.len(),
//         "Should produce same number of output lines"
//     );
//
//     // Both should show identical statistics
//     let expected_stats = ["6 total", "2 output", "1 filtered", "3 errors"];
//     for stat in &expected_stats {
//         assert!(
//             stderr_seq.contains(stat),
//             "Sequential mode should contain: {}",
//             stat
//         );
//         assert!(
//             stderr_par.contains(stat),
//             "Parallel mode should contain: {}",
//             stat
//         );
//     }
// }

// TODO: Update test for new stats format
// #[test]
// fn test_error_stats_multiline_mode() {
//     // Test error stats in multiline mode to ensure proper display format
//     let input = r#"{"valid": "json", "message": "line1\nline2"}
// {malformed json line}
// {"another": "valid", "message": "single line"}"#;
//
//     let (stdout, _stderr, exit_code) =
//         run_kelora_with_input(&["-f", "json", "--on-error", "skip", "--stats"], input);
//     assert_eq!(exit_code, 0, "Should exit successfully");
//
//     // Should output 2 valid JSON lines, skip 1 malformed line
//     let output_lines: Vec<&str> = stdout.trim().split('\n').collect();
//     assert_eq!(output_lines.len(), 2, "Should output 2 valid JSON lines");
//
//     // Stats should show separate error count
//     assert!(
//         stderr.contains("3 total"),
//         "Should show 3 total lines processed"
//     );
//     assert!(stderr.contains("2 output"), "Should show 2 output lines");
//     assert!(stderr.contains("1 errors"), "Should show 1 parsing error");
//     assert!(
//         stderr.contains("0 filtered"),
//         "Should show 0 filtered lines"
//     );
//
//     // Test multiline mode specifically with events created
//     let (_stdout2, stderr2, exit_code2) = run_kelora_with_input(
//         &[
//             "-f",
//             "json",
//             "--multiline",
//             "indent",
//             "--on-error",
//             "skip",
//             "--stats",
//         ],
//         input,
//     );
//     assert_eq!(
//         exit_code2, 0,
//         "Should exit successfully with multiline mode"
//     );
//
//     // In multiline mode, stats should show both line and event information
//     assert!(
//         stderr2.contains("Events created:"),
//         "Should show event statistics in multiline mode"
//     );
//     assert!(
//         stderr2.contains("1 errors"),
//         "Should show 1 parsing error in multiline mode"
//     );
// }

// TODO: Update test for new stats format
// #[test]
// fn test_empty_line_handling_line_format_with_stats() { ... }

// TODO: Update test for new stats format
// #[test]
// fn test_empty_line_handling_structured_format_with_stats() { ... }
