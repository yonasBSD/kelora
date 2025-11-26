#!/bin/bash
# Mixed Format Log Handling Examples
#
# This script demonstrates techniques for handling logs that contain multiple formats
# (JSON, plain text, syslog, etc.) in the same file.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MIXED_LOG="$SCRIPT_DIR/mixed_format.log"

echo "==== Mixed Format Log Handling Examples ===="
echo

# Example 1: Split by format - Process only JSON lines
echo "1. Extract and process only JSON lines:"
echo "   Command: grep '^{' mixed_format.log | kelora -f json --filter 'e.level == \"error\"'"
echo
grep '^{' "$MIXED_LOG" | kelora -f json --filter 'e.level == "error"' 2>/dev/null || echo "   (No matches)"
echo

# Example 2: Split by format - Process only plain text lines
echo "2. Extract and process only plain text lines:"
echo "   Command: grep -v '^{' mixed_format.log | kelora -f line --filter 'e.line.contains(\"text\")'"
echo
grep -v '^{' "$MIXED_LOG" | kelora -f line --filter 'e.line.contains("text")' 2>/dev/null
echo

# Example 3: Use process substitution to handle multiple formats
echo "3. Process JSON and plain text separately, then combine:"
echo "   (This example shows the concept - combining streams requires more complex handling)"
echo "   kelora -f json <(grep '^{' mixed_format.log)"
kelora -f json <(grep '^{' "$MIXED_LOG") 2>/dev/null | head -3
echo "   ..."
echo

# Example 4: Show what happens with auto-detection on mixed format
echo "4. Auto-detection on mixed format (demonstrates the problem):"
echo "   Command: kelora -f auto mixed_format.log -v"
echo "   Note: This will detect JSON from first line, then fail on non-JSON lines"
echo
kelora -f auto "$MIXED_LOG" -v 2>&1 | head -10 || true
echo "   ..."
echo

# Example 5: Use line format as a resilient fallback
echo "5. Process as plain lines (always works, but loses structure):"
echo "   Command: kelora -f line mixed_format.log --filter 'e.line.contains(\"error\") || e.line.contains(\"ERROR\")'"
echo
kelora -f line "$MIXED_LOG" --filter 'e.line.contains("error") || e.line.contains("ERROR")' 2>/dev/null
echo

# Example 6: Extract structured data from JSON lines only
echo "6. Extract specific fields from JSON lines:"
echo "   Command: grep '^{' mixed_format.log | kelora -f json --exec 'print(e.level + \": \" + e.msg)' -s"
echo
grep '^{' "$MIXED_LOG" | kelora -f json --exec 'print(e.level + ": " + e.msg)' -s 2>/dev/null
echo

echo "==== Key Takeaways ===="
echo
echo "For mixed-format logs, the recommended approach is:"
echo "  1. Split logs by format BEFORE processing with kelora"
echo "  2. Use grep/awk/sed to separate structured vs unstructured lines"
echo "  3. Process each format with the appropriate parser"
echo "  4. Combine results if needed using standard Unix tools"
echo
echo "Kelora excels at processing single-format structured logs."
echo "For mixed formats, preprocessing is the right solution."
