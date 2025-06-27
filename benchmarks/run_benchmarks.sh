#!/usr/bin/env bash

# Kelora Performance Benchmark Suite
# Runs standardized performance tests to detect regressions

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="./target/release/kelora"
RESULTS_FILE="${BENCHMARK_DIR}/benchmark_results.json"
BASELINE_FILE="${BENCHMARK_DIR}/baseline_results.json"

# Test configurations (using functions for broader shell compatibility)
get_test_cmd() {
    case "$1" in
        "small_filter")
            echo "benchmarks/bench_100k.jsonl --filter \"level == 'ERROR'\""
            ;;
        "medium_processing") 
            echo "benchmarks/bench_500k.jsonl --filter \"component == 'api'\" --eval \"track_count('status_codes', status)\""
            ;;
        "large_parallel")
            echo "benchmarks/bench_500k.jsonl --filter \"response_time.sub_string(0,2).to_int() > 100\" --parallel --threads 4"
            ;;
        "sequential_throughput")
            echo "benchmarks/bench_500k.jsonl --on-error skip"
            ;;
        *)
            echo ""
            ;;
    esac
}

get_test_names() {
    echo "small_filter medium_processing large_parallel sequential_throughput"
}

# Ensure binary exists
check_binary() {
    if [ ! -f "$BINARY" ]; then
        echo -e "${RED}Error: Binary not found at $BINARY${NC}"
        echo "Please run: cargo build --release"
        exit 1
    fi
}

# Generate benchmark datasets if they don't exist
ensure_datasets() {
    local datasets=(
        "bench_50k.jsonl:50000"
        "bench_100k.jsonl:100000"
        "bench_500k.jsonl:500000"
    )
    
    for dataset_spec in "${datasets[@]}"; do
        local filename=$(echo "$dataset_spec" | cut -d: -f1)
        local lines=$(echo "$dataset_spec" | cut -d: -f2)
        local filepath="benchmarks/$filename"
        
        if [ ! -f "$filepath" ]; then
            echo -e "${YELLOW}Generating $filename ($lines lines)...${NC}" >&2
            if [ -f "benchmarks/generate_test_data.py" ]; then
                python3 benchmarks/generate_test_data.py "$lines" > "$filepath"
                echo -e "${GREEN}Generated $filename ($(ls -lh "$filepath" | awk '{print $5}'))${NC}" >&2
            else
                echo -e "${RED}Error: benchmarks/generate_test_data.py not found${NC}" >&2
                exit 1
            fi
        fi
    done
}

# Run a single benchmark test
run_test() {
    local test_name="$1"
    local test_cmd="$2"
    local runs=3
    local total_time=0
    local times=()
    
    echo -e "${BLUE}Running test: $test_name${NC}" >&2
    
    for i in $(seq 1 $runs); do
        echo -n "  Run $i/$runs: " >&2
        
        # Run the test and capture time
        if command -v gtime >/dev/null 2>&1; then
            # Use GNU time for better precision
            time_result=$(gtime -f "%e" $BINARY $test_cmd >/dev/null 2>&1)
            time_taken="$time_result"
        else
            # Fall back to bash time
            start_time=$(date +%s.%N)
            $BINARY $test_cmd >/dev/null 2>&1
            end_time=$(date +%s.%N)
            time_taken=$(echo "$end_time - $start_time" | bc -l)
        fi
        
        times+=("$time_taken")
        total_time=$(echo "$total_time + $time_taken" | bc -l)
        echo "${time_taken}s" >&2
    done
    
    # Calculate statistics
    local avg_time=$(echo "scale=3; $total_time / $runs" | bc -l)
    local min_time=$(printf '%s\n' "${times[@]}" | sort -n | head -1)
    local max_time=$(printf '%s\n' "${times[@]}" | sort -n | tail -1)
    
    echo -e "  ${GREEN}Average: ${avg_time}s (min: ${min_time}s, max: ${max_time}s)${NC}" >&2
    
    # Return JSON result
    cat <<EOF
{
  "test": "$test_name",
  "runs": $runs,
  "times": [$(IFS=,; echo "${times[*]}")],
  "avg_time": $avg_time,
  "min_time": $min_time,
  "max_time": $max_time,
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
}

# Compare with baseline
compare_with_baseline() {
    local current_results="$1"
    
    if [ ! -f "$BASELINE_FILE" ]; then
        echo -e "${YELLOW}No baseline found. Saving current results as baseline.${NC}"
        echo "$current_results" > "$BASELINE_FILE"
        return
    fi
    
    echo -e "${BLUE}Comparing with baseline:${NC}"
    
    # Simple comparison using jq if available, otherwise basic comparison
    if command -v jq >/dev/null 2>&1; then
        # Use jq for JSON comparison
        echo "$current_results" | jq -r '.results[] | .test + ": " + (.avg_time | tostring) + "s"' | while read line; do
            test_name=$(echo "$line" | cut -d: -f1)
            current_time=$(echo "$line" | cut -d: -f2 | tr -d 's ')
            
            baseline_time=$(jq -r ".results[] | select(.test == \"$test_name\") | .avg_time" "$BASELINE_FILE" 2>/dev/null || echo "0")
            
            if [ "$baseline_time" != "0" ] && [ "$baseline_time" != "null" ]; then
                change=$(echo "scale=1; ($current_time - $baseline_time) / $baseline_time * 100" | bc -l)
                
                if (( $(echo "$change > 10" | bc -l) )); then
                    echo -e "  ${RED}$test_name: ${current_time}s (+${change}% REGRESSION)${NC}"
                elif (( $(echo "$change < -5" | bc -l) )); then
                    echo -e "  ${GREEN}$test_name: ${current_time}s (${change}% improvement)${NC}"
                else
                    echo -e "  $test_name: ${current_time}s (${change}% change)"
                fi
            else
                echo -e "  $test_name: ${current_time}s (no baseline)"
            fi
        done
    else
        echo "  jq not available for detailed comparison"
    fi
}

# Main benchmark runner
main() {
    local update_baseline=false
    local quick_mode=false
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --update-baseline)
                update_baseline=true
                shift
                ;;
            --quick)
                quick_mode=true
                shift
                ;;
            -h|--help)
                cat <<EOF
Usage: $0 [OPTIONS]

Options:
  --update-baseline  Update baseline results with current run
  --quick           Run quick tests only (10k dataset)
  --help            Show this help message

Examples:
  $0                    # Run all benchmarks
  $0 --quick            # Run quick benchmarks only  
  $0 --update-baseline  # Update performance baseline
EOF
                exit 0
                ;;
            *)
                echo "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    echo -e "${GREEN}=== Kelora Performance Benchmark Suite ===${NC}"
    echo "Binary: $BINARY"
    echo "Results: $RESULTS_FILE"
    echo ""
    
    check_binary
    ensure_datasets
    
    # Build current timestamp
    local timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    local results_json="{\"timestamp\": \"$timestamp\", \"results\": ["
    local first=true
    
    # Run tests
    for test_name in $(get_test_names); do
        # In quick mode, only run small_filter and use smaller datasets
        if [ "$quick_mode" = true ]; then
            case "$test_name" in
                small_filter)
                    # Use 50k dataset for quick mode
                    test_cmd="benchmarks/bench_50k.jsonl --filter \"level == 'ERROR'\""
                    ;;
                *medium_processing*|*large_parallel*|*sequential_throughput*)
                    continue
                    ;;
            esac
        else
            test_cmd=$(get_test_cmd "$test_name")
        fi
        if [ -n "$test_cmd" ]; then
            result=$(run_test "$test_name" "$test_cmd")
            
            if [ "$first" = true ]; then
                first=false
            else
                results_json+=","
            fi
            results_json+="$result"
            echo ""
        fi
    done
    
    results_json+="]}"
    
    # Save results
    echo "$results_json" > "$RESULTS_FILE"
    echo -e "${GREEN}Results saved to: $RESULTS_FILE${NC}"
    
    # Compare with baseline
    compare_with_baseline "$results_json"
    
    # Update baseline if requested
    if [ "$update_baseline" = true ]; then
        echo "$results_json" > "$BASELINE_FILE"
        echo -e "${GREEN}Baseline updated${NC}"
    fi
}

main "$@"