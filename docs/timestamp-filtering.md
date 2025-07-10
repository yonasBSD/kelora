# Timestamp Filtering with --since and --until

Kelora supports filtering events based on their parsed timestamps using `--since` and `--until` options, similar to journalctl.

## Basic Usage

```bash
# Show events since a specific timestamp
kelora --since "2023-12-01T10:00:00Z" access.log

# Show events until a specific timestamp  
kelora --until "2023-12-01T18:00:00Z" access.log

# Show events within a time range
kelora --since "2023-12-01T10:00:00Z" --until "2023-12-01T18:00:00Z" access.log
```

## Supported Timestamp Formats

### Absolute Timestamps
- ISO 8601: `2023-12-01T10:30:00Z`, `2023-12-01 10:30:00`
- Unix timestamps: `1700000000` (seconds), `1700000000000` (milliseconds)
- Date only: `2023-12-01` (assumes 00:00:00)
- Time only: `10:30:00` (assumes today's date)

### Special Values
- `now` - Current time
- `today` - Start of today (00:00:00)
- `yesterday` - Start of yesterday (00:00:00) 
- `tomorrow` - Start of tomorrow (00:00:00)

### Relative Times
- `-1h` - 1 hour ago
- `-30m` - 30 minutes ago
- `-2d` - 2 days ago
- `+1h` - 1 hour from now

Supported units: `s` (seconds), `m` (minutes), `h` (hours), `d` (days), `w` (weeks)

## Error Handling

When events don't have valid timestamps, behavior depends on the `--on-error` option:

- `--on-error skip` (default) - Filter out events without timestamps
- `--on-error print` - Pass through with warning message
- `--on-error abort` - Stop processing on missing timestamps
- `--on-error stub` - Pass through silently

## Examples

```bash
# Show events from the last hour
kelora -f jsonl --since "-1h" app.log

# Show today's events only
kelora --since "today" --until "tomorrow" access.log

# Show events with error handling
kelora --since "2023-12-01" --on-error skip mixed.log

# Combine with other filters
kelora --since "-2h" --levels error --filter 'msg.contains("timeout")' app.log
```

## Notes

- Timestamp filtering works with all log formats that have timestamp fields
- Common timestamp field names are automatically detected: `ts`, `timestamp`, `time`, `@timestamp`, etc.
- Custom timestamp fields via `--ts-field` are not yet fully supported
- Events without timestamps are handled according to `--on-error` strategy
- Filtered events are counted in statistics output (`--stats`)