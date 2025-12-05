# Exit Codes

Complete reference for Kelora's exit codes and their meanings.

## Standard Exit Codes

Kelora uses standard Unix exit codes to indicate success or failure:

| Code | Name | Meaning | Cause |
|------|------|---------|-------|
| `0` | Success | No errors occurred | Clean processing, no parse/filter/exec/file errors |
| `1` | General Error | Processing errors occurred | Parse errors, filter errors, exec errors, file I/O failures |
| `2` | Usage Error | Invalid command-line usage | Invalid flags, incompatible options, configuration errors |

## Signal Exit Codes

When Kelora is interrupted by signals:

| Code | Signal | Meaning |
|------|--------|---------|
| `130` | SIGINT | Interrupted by Ctrl+C |
| `141` | SIGPIPE | Broken pipe (normal in Unix pipelines) |
| `143` | SIGTERM | Terminated by system or user |

## Exit Code 0: Success

Indicates **successful processing** with no errors. All lines parsed successfully, no filter/exec errors, and processing completed normally.

**Important:** Filtering events is **not** an error. If all events are filtered out, exit code is still `0`.

```bash
# Returns 0 - filtering is not an error
kelora -j app.log --levels critical
echo $?
0

# Returns 1 - parse error occurred
kelora -j malformed.log
echo $?
1
```

## Exit Code 1: Processing Errors

Indicates errors occurred during processing. Four types:

| Error Type | Cause | Example |
|------------|-------|---------|
| **Parse errors** | Lines couldn't be parsed in specified format | Invalid JSON/logfmt syntax |
| **Filter errors** | `--filter` expressions failed during evaluation | Missing field access, type errors |
| **Exec errors** | `--exec` scripts failed during execution | Runtime errors in Rhai code |
| **File I/O failures** | Individual files failed to open or decompress | Permission denied, file not readable, decompression failed |

### Resilient Mode (Default)

Errors are recorded but processing continues. Exit code `1` returned at the end if any errors occurred:

```bash
kelora -j app.log --exec 'e.result = e.value.to_int()'
# ... processing continues despite errors ...
echo $?
1  # Errors occurred but processing completed
```

### Strict Mode

Processing aborts immediately on first error with `--strict`:

```bash
kelora -j --strict app.log
# ... aborts on first parse/filter/exec/file error ...
echo $?
1  # Processing aborted

# With multiple files, strict mode aborts on first file failure
kelora -j --strict good.log missing.log another.log
⚠️ Failed to open file 'missing.log': No such file or directory
echo $?
1  # Aborted immediately, another.log not processed
```

### Examples

```bash
# Parse error
echo '{invalid json}' | kelora -j
# Filter error
kelora -j app.log --filter 'e.missing_field.to_int()'
# Exec error
kelora -j app.log --exec 'e.result = e.invalid.to_int()'
# File I/O error (some files failed)
kelora -j good.log missing.log
# All return exit code 1
```

## Exit Code 2: Usage Errors

Indicates **command-line usage errors** before processing begins:

| Error Type | Example |
|------------|---------|
| Invalid flags | `kelora --invalid-flag app.log` |
| Missing arguments | `kelora --filter` (no expression provided) |
| Incompatible flags | `kelora -I helper.rhai --filter 'e.level == "ERROR"'` |
| Invalid configuration | `kelora --config-file invalid.ini app.log` |
| No input provided | `kelora` (no files + stdin is TTY) |

**Note:** File I/O failures (unable to open files) are **processing errors** (exit 1), not usage errors, regardless of how many files fail. This is consistent with standard Unix tools like `cat`, `tail`, and `head`.

```bash
# File I/O failures are always exit 1
kelora -j nonexistent.log
⚠️ Failed to open file 'nonexistent.log': No such file or directory
echo $?
1  # Processing error, not usage error

# Even if all files fail
kelora -j missing1.log missing2.log
⚠️ Failed to open file 'missing1.log': No such file or directory
⚠️ Failed to open file 'missing2.log': No such file or directory
echo $?
1  # Still processing error (like parse errors)

# Usage error (incompatible flags)
kelora -I helper.rhai --filter 'e.level == "ERROR"'
⚠️ --include is not supported with --filter
echo $?
2  # Usage error
```

## Mode Interactions

### Processing Modes

| Mode | Behavior | Exit Code 1 Timing | Use Case |
|------|----------|-------------------|----------|
| **Resilient** (default) | Continue on errors | At end if any errors occurred | Production, collect all errors |
| **Strict** (`--strict`) | Abort on first error | Immediately on first error | Validation, fail-fast |
| **Parallel** (`--parallel`) | Same as sequential | Same as non-parallel | Performance (exit code behavior unchanged) |

### Quiet Modes

Exit codes are preserved under all quiet/silent combinations. Use the new toggles to control output without changing exit semantics:

- `-q/--quiet`: suppress events only.
- `--no-diagnostics`: suppress diagnostics and summaries (fatal line still emitted).
- `--silent`: suppress pipeline terminal emitters (events/diagnostics/stats/terminal metrics); script output still allowed; metrics files still write.
- `-m/--metrics` / `-s/--stats`: data-only modes that already suppress events (no need for `-q`) and script output while emitting the selected channel.

## Using Exit Codes in Scripts

### Comprehensive Example

```bash
#!/bin/bash
# Production log processing script

# Check for usage errors first
if ! kelora -j --strict app.log > /dev/null 2>&1; then
    exit_code=$?
    case $exit_code in
        1)
            echo "✗ Parse errors detected, check log format"
            kelora -j --verbose app.log 2>&1 | grep "error:" | head -5
            exit 1
            ;;
        2)
            echo "✗ Usage error, check command syntax"
            exit 2
            ;;
        130)
            echo "⚠️  Interrupted by user"
            exit 130
            ;;
        141)
            # SIGPIPE is normal in pipelines
            exit 0
            ;;
        *)
            echo "✗ Unknown error (exit code: $exit_code)"
            exit $exit_code
            ;;
    esac
fi

# Process validated logs
if kelora -j app.log --levels error,critical -F json > errors.json; then
    echo "✓ No errors found"
else
    echo "✗ Processing failed"
    exit 1
fi
```

### Common Patterns

```bash
# Basic validation
kelora --silent -j --strict app.log && echo "✓ Valid" || echo "✗ Invalid"

# Ignore SIGPIPE in pipelines
kelora -j large.log | head -n 10 || [ $? -eq 141 ]

# CI/CD validation
kelora -j --strict logs/*.json > /dev/null || {
    echo "✗ Log validation failed"
    exit 1
}

# Check for critical events
if ! kelora -q -j app.log --levels error,critical; then
    echo "✗ Found critical errors"
    exit 1
fi

# Validation loop
for file in logs/*.json; do
    kelora -j --strict "$file" > /dev/null 2>&1 || {
        echo "✗ Invalid: $file"
        exit 1
    }
done
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

## Troubleshooting

### Exit Code 1 When Expecting 0

**Problem:** Getting exit code `1` but don't see errors.

**Solution:** Use `--verbose` to see all errors, or check stats:

```bash
kelora -j --verbose app.log
kelora -j --stats app.log
```

### Exit Code 141 (SIGPIPE)

**This is normal** in pipelines when downstream commands close early:

```bash
kelora -j large.log | head -n 10
# Exit code 141 is expected and normal

# Ignore SIGPIPE if needed
kelora -j large.log | head -n 10 || [ $? -eq 141 ]
```

### Exit Code 2 with Valid Syntax

**Check:**

- File exists and is readable: `ls -l app.log`
- Permissions are correct: `stat app.log`
- Configuration file is valid
- Try ignoring config: `kelora --ignore-config -j app.log`

### Different Exit Codes in CI vs Local

**Possible causes:**

- Different file permissions/paths
- Different configuration files
- Environment variables

**Solution:** Test with same conditions:
```bash
kelora --ignore-config -j app.log  # Ignore config files
```

## See Also

- [Error Handling](../concepts/error-handling.md) - Error handling modes and strategies
- [CLI Reference](cli-reference.md) - Complete flag documentation
- [Quiet/Silent Controls](../concepts/error-handling.md#quietsilent-controls) - Suppressing output for automation
