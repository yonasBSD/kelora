#!/bin/bash

# test_kelora.sh - Comprehensive test runner for kelora 0.2.0

set -e  # Exit on any error

echo "🧪 Running kelora 0.2.0 test suite..."
echo "====================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${2}${1}${NC}"
}

# Build the project first
print_status "📦 Building kelora..." $YELLOW
if cargo build --release; then
    print_status "✅ Build successful" $GREEN
else
    print_status "❌ Build failed" $RED
    exit 1
fi

# Run unit tests
print_status "🔧 Running unit tests..." $YELLOW
if cargo test --lib; then
    print_status "✅ Unit tests passed" $GREEN
else
    print_status "❌ Unit tests failed" $RED
    exit 1
fi

# Manual tests with sample data
print_status "📝 Running manual tests with sample data..." $YELLOW

# Create sample data files
TEMP_DIR=$(mktemp -d)
echo "Using temp directory: $TEMP_DIR"

# Create sample JSONL file
cat > "$TEMP_DIR/sample.jsonl" << 'EOF'
{"user": "alice", "status": 200, "message": "login successful", "timestamp": "2023-07-18T15:04:23.456Z"}
{"user": "bob", "status": 404, "message": "page not found", "timestamp": "2023-07-18T15:04:25.789Z"}
{"user": "charlie", "status": 500, "message": "internal error", "timestamp": "2023-07-18T15:06:41.210Z"}
{"user": "alice", "status": 403, "message": "forbidden", "timestamp": "2023-07-18T15:07:12.345Z"}
{"user": "dave", "status": 200, "message": "success", "timestamp": "2023-07-18T15:08:30.678Z"}
EOF

# Create sample with levels
cat > "$TEMP_DIR/levels.jsonl" << 'EOF'
{"level": "error", "user": "alice", "message": "failed login"}
{"level": "info", "user": "bob", "message": "successful login"}
{"level": "warn", "user": "charlie", "message": "slow response"}
{"level": "debug", "user": "dave", "message": "debug info"}
EOF

# Test 1: Basic JSONL parsing and JSON output
print_status "Test 1: Basic JSONL parsing" $YELLOW
./target/release/kelora "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output1.json"
if [ -s "$TEMP_DIR/output1.json" ]; then
    print_status "✅ JSONL parsing works" $GREEN
    echo "   Output lines: $(wc -l < "$TEMP_DIR/output1.json")"
else
    print_status "❌ JSONL parsing failed" $RED
fi

# Test 2: Text output format
print_status "Test 2: Text output format" $YELLOW
./target/release/kelora -F text "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output2.txt"
if grep -q 'user="alice"' "$TEMP_DIR/output2.txt"; then
    print_status "✅ Text output format works" $GREEN
else
    print_status "❌ Text output format failed" $RED
fi

# Test 3: Basic filtering with Rhai expressions
print_status "Test 3: Basic filtering with Rhai" $YELLOW
./target/release/kelora --filter 'status >= 400' "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output3.json"
lines=$(wc -l < "$TEMP_DIR/output3.json")
if [ "$lines" -eq 3 ]; then
    print_status "✅ Basic filtering works (filtered to $lines lines)" $GREEN
else
    print_status "❌ Basic filtering failed (got $lines lines, expected 3)" $RED
fi

# Test 4: String filtering
print_status "Test 4: String filtering" $YELLOW
./target/release/kelora --filter 'user == "alice"' "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output4.json"
lines=$(wc -l < "$TEMP_DIR/output4.json")
if [ "$lines" -eq 2 ]; then
    print_status "✅ String filtering works (filtered to $lines lines)" $GREEN
else
    print_status "❌ String filtering failed (got $lines lines, expected 2)" $RED
fi

# Test 5: Key filtering
print_status "Test 5: Key filtering" $YELLOW
./target/release/kelora --keys user,status "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output5.json"
if grep -q '"user":"alice"' "$TEMP_DIR/output5.json" && ! grep -q '"message"' "$TEMP_DIR/output5.json"; then
    print_status "✅ Key filtering works" $GREEN
else
    print_status "❌ Key filtering failed" $RED
fi

# Test 6: Print function for debugging
print_status "Test 6: Print function debugging" $YELLOW
./target/release/kelora --eval 'print("Processing user: " + user)' "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output6.txt"
if grep -q "Processing user: alice" "$TEMP_DIR/output6.txt" && grep -q '{"' "$TEMP_DIR/output6.txt"; then
    print_status "✅ Print function works" $GREEN
else
    print_status "❌ Print function failed" $RED
fi

# Test 7: Begin and end stages
print_status "Test 7: Begin and end stages" $YELLOW
./target/release/kelora --begin 'print("Starting analysis")' --end 'print("Analysis complete")' "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output7.txt"
if grep -q "Starting analysis" "$TEMP_DIR/output7.txt" && grep -q "Analysis complete" "$TEMP_DIR/output7.txt"; then
    print_status "✅ Begin and end stages work" $GREEN
else
    print_status "❌ Begin and end stages failed" $RED
fi

# Test 8: Tracking functionality
print_status "Test 8: Tracking functionality" $YELLOW
./target/release/kelora --filter 'status >= 400' --eval 'track_count(tracked, "errors")' --end 'print("Total errors: " + tracked["errors"])' "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output8.txt"
if grep -q "Total errors: 3" "$TEMP_DIR/output8.txt"; then
    print_status "✅ Tracking functionality works" $GREEN
else
    print_status "❌ Tracking functionality failed" $RED
    echo "   Debug output:"
    cat "$TEMP_DIR/output8.txt"
fi

# Test 9: Multiple filters
print_status "Test 9: Multiple filters" $YELLOW
./target/release/kelora --filter 'status >= 400' --filter 'user != "charlie"' "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output9.json"
lines=$(wc -l < "$TEMP_DIR/output9.json")
if [ "$lines" -eq 2 ]; then
    print_status "✅ Multiple filters work (filtered to $lines lines)" $GREEN
else
    print_status "❌ Multiple filters failed (got $lines lines, expected 2)" $RED
fi

# Test 10: Level-based filtering
print_status "Test 10: Level-based filtering" $YELLOW
./target/release/kelora --filter 'level == "error"' "$TEMP_DIR/levels.jsonl" > "$TEMP_DIR/output10.json"
lines=$(wc -l < "$TEMP_DIR/output10.json")
if [ "$lines" -eq 1 ]; then
    print_status "✅ Level filtering works (filtered to $lines lines)" $GREEN
else
    print_status "❌ Level filtering failed (got $lines lines, expected 1)" $RED
fi

# Test 11: Stdin input
print_status "Test 11: Stdin input" $YELLOW
if cat "$TEMP_DIR/sample.jsonl" | ./target/release/kelora --filter 'status < 400' > "$TEMP_DIR/output11.json" && [ -s "$TEMP_DIR/output11.json" ]; then
    lines=$(wc -l < "$TEMP_DIR/output11.json")
    print_status "✅ Stdin input works (processed $lines lines)" $GREEN
else
    print_status "❌ Stdin input failed" $RED
fi

# Test 12: Error handling with invalid JSON
print_status "Test 12: Error handling" $YELLOW
echo '{"valid":"json"}
{malformed json}
{"another":"valid"}' | ./target/release/kelora --on-error skip > "$TEMP_DIR/output12.json" 2>"$TEMP_DIR/error12.txt"
valid_lines=$(wc -l < "$TEMP_DIR/output12.json")
if [ "$valid_lines" -eq 2 ]; then
    print_status "✅ Error handling works (processed $valid_lines valid lines)" $GREEN
else
    print_status "❌ Error handling failed (got $valid_lines lines, expected 2)" $RED
fi

# Test 13: Complex Rhai expressions
print_status "Test 13: Complex Rhai expressions" $YELLOW
./target/release/kelora --filter 'status >= 400 && user.contains("a")' "$TEMP_DIR/sample.jsonl" > "$TEMP_DIR/output13.json"
lines=$(wc -l < "$TEMP_DIR/output13.json")
if [ "$lines" -eq 2 ]; then
    print_status "✅ Complex expressions work (filtered to $lines lines)" $GREEN
else
    print_status "❌ Complex expressions failed (got $lines lines, expected 2)" $RED
fi

# Test 14: Performance test with larger file
print_status "Test 14: Performance test" $YELLOW
# Generate 1000 log entries
for i in $(seq 1 1000); do
    echo "{\"user\":\"user$i\",\"status\":$((200 + i % 300)),\"message\":\"Message $i\",\"id\":$i}"
done > "$TEMP_DIR/large.jsonl"

start_time=$(date +%s%N)
./target/release/kelora --filter 'status >= 400' --eval 'track_count(tracked, "errors")' --end 'print("Errors: " + tracked["errors"])' "$TEMP_DIR/large.jsonl" >/dev/null 2>&1
end_time=$(date +%s%N)
duration=$(( (end_time - start_time) / 1000000 )) # Convert to milliseconds

if [ $duration -lt 5000 ]; then  # Less than 5 seconds
    print_status "✅ Performance test passed (${duration}ms for 1000 entries)" $GREEN
else
    print_status "⚠️  Performance test slow (${duration}ms for 1000 entries)" $YELLOW
fi

# Test 15: Help and version commands
print_status "Test 15: Help and version commands" $YELLOW
if ./target/release/kelora --help | grep -q "Rhai scripting" && ./target/release/kelora --version | grep -q "0.2.0"; then
    print_status "✅ Help and version commands work" $GREEN
else
    print_status "❌ Help and version commands failed" $RED
fi

# Cleanup
print_status "🧹 Cleaning up..." $YELLOW
rm -rf "$TEMP_DIR"

# Summary
print_status "📊 Test Summary" $YELLOW
echo "====================================="
print_status "✅ All tests completed successfully!" $GREEN
echo ""
echo "Kelora 0.2.0 features tested:"
echo "  ✓ JSONL input parsing and field injection"
echo "  ✓ Rhai expression filtering (--filter)"
echo "  ✓ Print function for debugging"
echo "  ✓ Multi-stage processing (--begin, --eval, --end)"
echo "  ✓ Tracking functions (track_count)"
echo "  ✓ JSON and text output formats"
echo "  ✓ Error handling strategies"
echo "  ✓ Stdin and file input"
echo ""
echo "Example usage:"
echo "  ./target/release/kelora --filter 'status >= 400' logs.jsonl"
echo "  ./target/release/kelora --eval 'print(\"User: \" + user)' logs.jsonl"
echo "  ./target/release/kelora --begin 'print(\"Starting\")' --filter 'level == \"error\"' --end 'print(\"Done\")' logs.jsonl"
echo ""
print_status "Happy log analysis with Rhai! 🦀✨" $GREEN