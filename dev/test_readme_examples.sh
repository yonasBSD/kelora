#!/bin/bash
# Test all README examples to ensure they work with the new examples/ files

set -e

KELORA="./target/release/kelora"

echo "Testing README examples..."

# Basics section
echo "✓ Test 1: Filter logfmt errors"
$KELORA -f logfmt -l error examples/simple_logfmt.log > /dev/null

echo "✓ Test 2: JSON with filter and exec"
$KELORA -j examples/simple_json.jsonl \
  --filter 'e.level == "ERROR"' \
  --exec 'e.retry_count = e.get_path("retry", 0)' \
  --keys timestamp,level,message,retry_count > /dev/null

echo "✓ Test 3: Combined format with stats"
$KELORA -f combined examples/web_access_large.log.gz \
  --keys ip,status,request_time,request \
  --stats > /dev/null 2>&1

echo "✓ Test 4: Context lines"
$KELORA -j examples/simple_json.jsonl \
  --filter 'e.level == "ERROR"' \
  --after-context 2 --before-context 1 > /dev/null

# Advanced section
echo "✓ Test 5: Logfmt with metrics"
$KELORA -f logfmt examples/simple_logfmt.log \
  --filter 'e.duration.to_int_or(0) >= 1000' \
  --exec 'track_count("slow_requests"); e.bucket = if e.duration.to_int_or(0) >= 2000 { "very_slow" } else { "slow" }' \
  --metrics > /dev/null 2>&1

echo "✓ Test 6: Complex pipeline"
$KELORA -j examples/simple_json.jsonl \
  --begin 'conf.error_levels = ["ERROR", "FATAL"]; conf.retry_threshold = 2' \
  --filter 'conf.error_levels.contains(e.level)' \
  --exec 'if e.get_path("retry", 0) >= conf.retry_threshold { track_count("retries"); }' \
  --window 1 \
  --exec 'let comps = window_values("component"); if comps.len() > 1 && comps[0] == comps[1] { e.context = "repeat_component"; }' \
  --metrics > /dev/null 2>&1

# Format recipes
echo "✓ Test 7: Raw format"
$KELORA -f raw examples/simple_line.log \
  --exec 'e.byte_len = e.raw.len()' > /dev/null

echo "✓ Test 8: Line format filtering"
$KELORA -f line examples/simple_line.log \
  --filter 'e.line.contains("ERROR")' > /dev/null

# Syslog examples - need to fix field name
echo "✓ Test 9: Syslog filtering"
$KELORA -f syslog examples/simple_syslog.log \
  --filter '"msg" in e && e.msg.contains("Failed")' > /dev/null

echo "✓ Test 10: Syslog to JSON"
$KELORA -f syslog examples/simple_syslog.log \
  --exec 'e.severity_label = if e.severity <= 3 { "critical" } else if e.severity <= 4 { "error" } else { "info" }; e.host = e.host.mask_ip(1);' \
  -J > /dev/null

echo ""
echo "All README examples passed!"
