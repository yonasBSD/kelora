#!/usr/bin/env bash

# Generate comparison datasets in CSV and syslog formats
# These are used for benchmarking against tools like miller, qsv, grep, awk

set -e

BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Generating comparison datasets..."

# Ensure JSON datasets exist first
for size in 100k 500k; do
    file="$BENCHMARK_DIR/bench_${size}.jsonl"
    if [ ! -f "$file" ]; then
        lines=${size//k/000}
        echo "Generating $file ($lines lines)..."
        python3 "$BENCHMARK_DIR/generate_test_data.py" "$lines" > "$file"
    fi
done

# Generate CSV dataset (for miller/qsv)
echo "Generating CSV dataset (bench_100k.csv)..."
if command -v jq >/dev/null 2>&1; then
    {
        echo "timestamp,level,component,message,request_id,host,method,status"
        head -100000 "$BENCHMARK_DIR/bench_100k.jsonl" | \
            jq -r '[.timestamp, .level, .component, .message, .request_id, .host, (.method // ""), (.status // "")] | @csv'
    } > "$BENCHMARK_DIR/bench_100k.csv"
    echo "  ✓ Created bench_100k.csv ($(wc -l < "$BENCHMARK_DIR/bench_100k.csv") rows)"
else
    echo "  ✗ jq not found, skipping CSV generation"
fi

# Generate syslog-style text file (for grep/awk)
echo "Generating syslog-style dataset (bench_100k.log)..."
if command -v jq >/dev/null 2>&1; then
    head -100000 "$BENCHMARK_DIR/bench_100k.jsonl" | \
        jq -r '"\(.timestamp) \(.host) \(.component)[\(.level)]: \(.message)"' \
        > "$BENCHMARK_DIR/bench_100k.log"
    echo "  ✓ Created bench_100k.log ($(wc -l < "$BENCHMARK_DIR/bench_100k.log") lines)"
else
    echo "  ✗ jq not found, skipping log generation"
fi

echo "Done!"
echo ""
echo "Generated files:"
ls -lh "$BENCHMARK_DIR"/*.{csv,log} 2>/dev/null | awk '{print "  " $9, "(" $5 ")"}'
