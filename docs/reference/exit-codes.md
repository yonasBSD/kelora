# Exit Codes

Complete reference for Kelora's exit codes and their meanings.

## Standard Exit Codes

Kelora uses standard Unix exit codes to indicate success or failure:

| Code | Name | Meaning | Cause |
|------|------|---------|-------|
| `0` | Success | No errors occurred | Clean processing, no parse/filter/exec errors |
| `1` | General Error | Processing errors occurred | Parse errors, filter errors, exec errors |
| `2` | Usage Error | Invalid command-line usage | Invalid flags, missing files, configuration errors |

## Signal Exit Codes

When Kelora is interrupted by signals:

| Code | Signal | Meaning |
|------|--------|---------|
| `130` | SIGINT | Interrupted by Ctrl+C |
| `141` | SIGPIPE | Broken pipe (normal in Unix pipelines) |
| `143` | SIGTERM | Terminated by system or user |

## Exit Code 0: Success

### When It Occurs

Exit code `0` indicates **successful processing** with no errors:

- All lines parsed successfully
- No filter errors
- No exec/transform errors
- Processing completed normally

### Example

```bash
kelora -j app.log --levels info
echo $?
0
```

**Note:** Filtering events is **not** considered an error. If all events are filtered out, exit code is still `0` (as long as no errors occurred).

### Filtering vs Errors

```bash
# This returns 0 (filtering is not an error)
kelora -j app.log --levels critical
echo $?
0

# This returns 1 (parse error occurred)
kelora -j malformed.log
echo $?
1
```

## Exit Code 1: Processing Errors

### When It Occurs

Exit code `1` indicates **processing errors** occurred:

- **Parse errors**: Lines that couldn't be parsed in specified format
- **Filter errors**: `--filter` expressions that failed during evaluation
- **Exec errors**: `--exec` scripts that failed during execution

### Resilient Mode (Default)

In resilient mode, errors are recorded but processing continues. Exit code `1` is returned at the end if any errors occurred:

```bash
kelora -j app.log --exec 'e.result = e.value.to_int()'
# ... processing continues despite errors ...
echo $?
1  # Errors occurred but processing completed
```

### Strict Mode

In strict mode (`--strict`), processing aborts immediately on first error:

```bash
kelora -j --strict app.log
# ... aborts on first error ...
echo $?
1  # Processing aborted due to error
```

### Parse Errors

**Example:**
```bash
echo '{"valid": "json"}' > test.log
echo '{invalid json}' >> test.log
kelora -j test.log
echo $?
1  # Parse error occurred
```

### Filter Errors

**Example:**
```bash
kelora -j app.log --filter 'e.timestamp.to_unix() > 1000000'
# If e.timestamp is missing or invalid
echo $?
1  # Filter error occurred
```

### Exec Errors

**Example:**
```bash
kelora -j app.log --exec 'e.result = e.value.to_int()'
# If e.value is not a valid integer
echo $?
1  # Exec error occurred
```

## Exit Code 2: Usage Errors

### When It Occurs

Exit code `2` indicates **command-line usage errors**:

- **Invalid flags**: Unknown or malformed options
- **Missing required arguments**: Required values not provided
- **File not found**: Input files don't exist
- **Configuration errors**: Invalid configuration file
- **Permission denied**: Can't read input files

### Invalid Flags

```bash
kelora --invalid-flag app.log
kelora: error: unrecognized option '--invalid-flag'
echo $?
2
```

### File Not Found

```bash
kelora -j nonexistent.log
kelora: error: failed to open file 'nonexistent.log': No such file or directory
echo $?
2
```

### Invalid Configuration

```bash
kelora --config-file invalid.ini app.log
kelora: error: failed to parse configuration file
echo $?
2
```

## Signal Exit Codes

### Exit Code 130: SIGINT (Ctrl+C)

User interrupted processing with Ctrl+C:

```bash
kelora -j large.log
# Press Ctrl+C
^C
echo $?
130
```

### Exit Code 141: SIGPIPE (Broken Pipe)

Output pipe closed (normal in Unix pipelines):

```bash
kelora -j large.log | head -n 10
# Kelora receives SIGPIPE when head closes
echo $?
141
```

**Note:** Exit code `141` is **normal** in pipelines and typically not an error condition.

### Exit Code 143: SIGTERM

Process terminated by system or user:

```bash
kelora -j large.log &
[1] 12345
kill 12345
[1]+  Terminated              kelora -j large.log
echo $?
143
```

## Using Exit Codes in Scripts

### Basic Success/Failure Check

```bash
if kelora -j app.log > /dev/null 2>&1; then
    echo "✓ No errors in logs"
else
    echo "✗ Errors detected in logs"
    exit 1
fi
```

### Check Specific Exit Codes

```bash
kelora -j app.log
exit_code=$?

case $exit_code in
    0)
        echo "✓ Success - no errors"
        ;;
    1)
        echo "✗ Processing errors occurred"
        exit 1
        ;;
    2)
        echo "✗ Usage error - check command syntax"
        exit 2
        ;;
    130)
        echo "⚠️  Interrupted by user"
        exit 130
        ;;
    *)
        echo "✗ Unknown error (exit code: $exit_code)"
        exit $exit_code
        ;;
esac
```

### With Quiet Mode

Use quiet modes to suppress output, rely on exit code:

```bash
# Level 1: Suppress diagnostics
kelora -q -j app.log > /dev/null
if [ $? -eq 0 ]; then
    echo "No errors"
fi

# Level 2: Suppress events too
kelora -qq -j app.log
if [ $? -eq 0 ]; then
    echo "No errors"
fi

# Level 3: Complete silence
kelora -qqq -j app.log
if [ $? -eq 0 ]; then
    echo "No errors"
fi
```

### CI/CD Pipeline

```bash
#!/bin/bash
set -e  # Exit on any error

# Validate log format
kelora -j --strict app.log > /dev/null || {
    echo "✗ Log validation failed"
    exit 1
}

# Check for errors in logs
if ! kelora -qq -j app.log --levels error,critical; then
    echo "✗ Found error-level events in logs"
    exit 1
fi

echo "✓ All checks passed"
```

### Makefile Integration

```makefile
.PHONY: check-logs
check-logs:
	@kelora -j logs/*.json --levels error || \
		(echo "Errors found in logs" && exit 1)

.PHONY: validate-logs
validate-logs:
	@kelora -j --strict logs/*.json > /dev/null || \
		(echo "Log validation failed" && exit 1)
```

### GitHub Actions

```yaml
- name: Validate logs
  run: |
    kelora -j --strict app.log || exit 1

- name: Check for errors
  run: |
    if kelora -qq -j app.log --levels error,critical; then
      echo "✓ No critical errors"
    else
      echo "✗ Critical errors found"
      exit 1
    fi
```

## Exit Codes with Different Modes

### Sequential vs Parallel

Exit codes work the same in both modes:

```bash
# Sequential
kelora -j app.log
echo $?

# Parallel
kelora -j --parallel app.log
echo $?
```

Both return `1` if any errors occurred.

### Resilient vs Strict

**Resilient mode (default):**

- Collects all errors
- Returns `1` if any errors occurred
- Processing completes

```bash
kelora -j app.log
# Processes all lines despite errors
echo $?
1  # Errors occurred
```

**Strict mode:**

- Aborts on first error
- Returns `1` immediately
- Processing incomplete

```bash
kelora -j --strict app.log
# Aborts on first error
echo $?
1  # Aborted due to error
```

### With Quiet Modes

Exit codes are preserved at all quiet levels:

```bash
# Level 1: Suppress diagnostics
kelora -q -j app.log
echo $?
1  # Error occurred (not shown, but exit code preserved)

# Level 2: Suppress events
kelora -qq -j app.log
echo $?
1  # Error occurred

# Level 3: Complete silence
kelora -qqq -j app.log
echo $?
1  # Error occurred
```

## Common Patterns

### Validation Pipeline

```bash
#!/bin/bash
# Validate log files before processing

for file in logs/*.json; do
    if ! kelora -j --strict "$file" > /dev/null 2>&1; then
        echo "✗ Invalid format: $file"
        exit 1
    fi
done

echo "✓ All log files valid"
```

### Data Quality Check

```bash
#!/bin/bash
# Check for errors in logs

kelora -qq -j app.log --levels error,critical
exit_code=$?

if [ $exit_code -eq 1 ]; then
    echo "✗ Parse errors in log file"
    exit 1
elif [ $exit_code -eq 0 ]; then
    # Check if any critical events found
    count=$(kelora -j app.log --levels critical -F none --stats 2>&1 | grep -oP '\d+(?= events output)')
    if [ "$count" -gt 0 ]; then
        echo "✗ Found $count critical events"
        exit 1
    fi
fi

echo "✓ No critical issues"
```

### Automation with Retry

```bash
#!/bin/bash
# Process logs with retry on failure

max_attempts=3
attempt=1

while [ $attempt -le $max_attempts ]; do
    if kelora -j app.log -F json > output.json; then
        echo "✓ Processing succeeded"
        exit 0
    else
        echo "✗ Attempt $attempt failed"
        attempt=$((attempt + 1))
        sleep 5
    fi
done

echo "✗ Processing failed after $max_attempts attempts"
exit 1
```

### Conditional Processing

```bash
#!/bin/bash
# Process only if no errors

if kelora -qq -j --strict app.log; then
    echo "✓ No errors, processing..."
    kelora -j app.log --exec 'track_count(e.service)' --metrics
else
    echo "✗ Errors detected, skipping processing"
    exit 1
fi
```

## Troubleshooting

### Exit Code 1 When Expecting 0

**Problem:** Getting exit code `1` but don't see errors.

**Solution:** Use `--verbose` to see errors:
```bash
kelora -j --verbose app.log
```

Or check stats:
```bash
kelora -j --stats app.log
```

### Exit Code 141 in Pipelines

**Problem:** Getting exit code `141` (SIGPIPE) in pipelines.

**This is normal:** When downstream commands close the pipe (like `head`), Kelora receives SIGPIPE.

```bash
kelora -j large.log | head -n 10
# Exit code 141 is normal here
```

To ignore SIGPIPE:
```bash
kelora -j large.log | head -n 10 || [ $? -eq 141 ]
```

### Exit Code 2 with Valid Syntax

**Problem:** Getting exit code `2` but command looks correct.

**Check:**

- File exists and is readable
- Configuration file is valid
- Permissions are correct

```bash
ls -l app.log
kelora -j app.log
```

### Different Exit Codes in CI vs Local

**Problem:** Different exit codes in CI/CD vs local development.

**Possible causes:**

- Different file permissions
- Different file paths
- Different configuration files
- Different environment variables

**Solution:** Test with same conditions:
```bash
kelora --ignore-config -j app.log  # Ignore config files
```

## See Also

- [Error Handling](../concepts/error-handling.md) - Error handling modes and strategies
- [CLI Reference](cli-reference.md) - Complete flag documentation
- [Quiet Modes](../concepts/error-handling.md#quiet-modes) - Suppressing output for automation
