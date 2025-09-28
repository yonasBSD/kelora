/// Generate help text with comprehensive hand-written function documentation
pub fn generate_help_text() -> &'static str {
    r#"
Available Rhai Functions for Kelora:

STRING/TEXT FUNCTIONS:
  text.after(delimiter)                 Text after first occurrence of delimiter
  text.before(delimiter)                Text before first occurrence of delimiter
  text.between(start, end)              Text between start and end delimiters
  text.col("1,3,5" [, separator])       Extract multiple columns as concatenated string
  text.col("1:5" [, separator])         Extract column range as concatenated string
  text.col(index [, separator])         Extract column by index from whitespace/delimited text
  text.contains(pattern)                Check if text contains pattern
  text.count(pattern)                   Count occurrences of pattern in text
  text.decode_b64()                     Decode base64 string to text
  text.decode_hex()                     Decode hexadecimal string to text
  text.decode_url()                     Decode URL-encoded string
  text.encode_b64()                     Encode text to base64 string
  text.encode_hex()                     Encode text to hexadecimal string
  text.encode_url()                     URL-encode text (percent encoding)
  text.ending_with(suffix)              Return text if it ends with suffix, else empty
  text.escape_html()                    Escape HTML special characters (&, <, >, ", ')
  text.escape_json()                    Escape JSON special characters
  text.extract_all_re(pattern [, group]) Extract all regex matches as array
  text.extract_domain()                 Extract domain from URL or email address
  text.extract_ip()                     Extract first IP address from text
  text.extract_ips()                    Extract all IP addresses as array
  text.extract_re(pattern [, group])    Extract regex match or capture group
  text.extract_url()                    Extract first URL from text
  text.is_digit()                       Check if text contains only digits
  text.is_private_ip()                  Check if IP is in private ranges
  text.lower()                          Convert text to lowercase
  text.mask_ip([octets])                Mask IP address (default: last octet)
  text.matches(pattern)                 Check if text matches regex pattern
  text.parse_cef()                      Parse Common Event Format line into fields
  text.parse_cols(spec [, sep])         Parse columns according to spec
  text.parse_combined()                 Parse Apache/Nginx combined log line
  text.parse_content_disposition()      Parse Content-Disposition header parameters
  text.parse_email()                    Parse email address into parts
  text.parse_jwt()                      Parse JWT header/payload without verification
  text.parse_kv([sep [, kv_sep]])       Parse key-value pairs from text
  text.parse_logfmt()                   Parse logfmt line into structured fields
  text.parse_media_type()               Parse media type tokens and parameters
  text.parse_path()                     Parse filesystem path into components
  text.parse_syslog()                   Parse syslog line into structured fields
  text.parse_url()                      Parse URL into structured components
  text.parse_user_agent()               Parse common user-agent strings into components
  text.slice(spec)                      Slice text using Python notation (e.g., "1:5", ":3", "-2:")
  text.split_re(pattern)                Split text by regex pattern
  text.starting_with(prefix)            Return text if it starts with prefix, else empty
  text.strip([chars])                   Remove whitespace or specified characters
  text.unescape_html()                  Unescape HTML entities to text
  text.unescape_json()                  Unescape JSON escape sequences
  text.upper()                          Convert text to uppercase
  to_int(text)                          Convert text to integer (0 on error)
  to_float(text)                        Convert text to float (0 on error)

ARRAY FUNCTIONS:
  array.join(separator)                 Join array elements with separator
  array.flatten([style [, max_depth]])  Flatten nested arrays/objects
  array.parse_cols(spec [, sep])        Apply column spec to pre-split values
  reversed(array)                       Return new array in reverse order
  sorted(array)                         Return new sorted array (numeric/lexicographic)
  sorted_by(array, field)               Sort array of objects by field name
  contains_any(array, search_array)     Check if array contains any search values
  starts_with_any(array, search_array)  Check if array starts with any search values

MAP/OBJECT FUNCTIONS:
  map.flatten([separator [, style]])    Flatten nested object to dot notation
  map.unflatten([separator])            Reconstruct nested object from flat keys
  map.merge(other_map)                  Merge another map into this one
  map.get_path("field.path" [, default]) Safe nested field access with fallback
  map.has_path("field.path")            Check if nested field path exists
  map.path_equals("path", value)        Safe nested field comparison

VALUE SAFETY FUNCTIONS:
  to_number(value [, default])          Safe number conversion with fallback (default: 0)
  to_bool(value [, default])            Safe boolean conversion with fallback

EVENT MANIPULATION:
  emit_each(array [, base_map])         Fan out array elements as separate events
  e = ()                                Clear entire event (remove all fields)
  e.field = ()                          Remove individual field from event

DATETIME FUNCTIONS:
  now_utc()                             Current UTC timestamp (DateTimeWrapper)
  now_local()                           Current local timestamp (DateTimeWrapper)
  to_datetime(text [, fmt [, tz]])      Convert string into DateTimeWrapper with optional hints
  to_duration("1h30m")                  Convert duration string into DurationWrapper
  dt.format("format_string")            Format datetime using custom format string
  dt.year(), dt.month(), dt.day()       Extract date components
  dt.hour(), dt.minute(), dt.second()   Extract time components
  dt.to_utc(), dt.to_local()            Convert timezone
  dt + dur, dt - dur                    Add/subtract duration from datetime
  dt1 - dt2                             Get duration between datetimes
  dur.as_seconds(), dur.as_minutes()    Convert duration to numeric values

TRACKING/METRICS FUNCTIONS:
  track_count(key)                      Increment counter for key by 1
  track_sum(key, value)                 Accumulate numeric values for key
  track_min(key, value)                 Track minimum value for key
  track_max(key, value)                 Track maximum value for key
  track_unique(key, value)              Track unique values for key
  track_bucket(key, bucket)             Track values in buckets for histograms

MATH/UTILITY FUNCTIONS:
  mod(a, b) / a % b                     Modulo operation with division-by-zero protection
  rand()                                Random float between 0 and 1
  rand_int(min, max)                    Random integer between min and max (inclusive)

UTILITY FUNCTIONS:
  print(message)                        Print to stdout (suppressed with -qqq)
  eprint(message)                       Print to stderr (suppressed with -qqq)
  get_env(var [, default])              Get environment variable with optional default
  read_file(path)                       Read file contents as string
  read_lines(path)                      Read file as array of lines
  exit(code)                            Exit kelora with given exit code
  percentile(array, pct)                Calculate percentile of numeric array
  window_values(field)                  Get field values from current window
  window_numbers(field)                 Get numeric field values from current window

FILE OUTPUT (REQUIRES --allow-fs-writes):
  mkdir(path [, recursive])              Create directory (set recursive=true to create parents)
  truncate_file(path)                    Create or zero-length a file for fresh output
  append_file(path, text_or_array)       Append line(s) to file; arrays append one line per element

Examples:
  # String processing with method syntax
  e.clean_url = e.url.extract_domain().lower()

  # Array processing and fan-out
  e.tag_count = e.tags.len()
  emit_each(e.items)  # Creates separate event for each item

  # Safe nested field access
  e.user_role = e.get_path("user.profile.role", "guest")

  # Datetime manipulation
  let now = now_utc()
  e.hour = now.hour()
  e.formatted = now.format("%Y-%m-%d %H:%M")

  # Tracking metrics
  track_count("http_requests")
  track_bucket("response_time", "slow")

  # Random values
  e.sample_rate = rand()
  e.random_id = rand_int(1000, 9999)

Rhai lets you call functions as either `value.method(args)` or
`function(value, args)`. Examples prefer method calls, though some
function-style listings remain for clarity. Use 'e' to access the current event.
For more examples, see the documentation or use --help for general usage.
"#
}
