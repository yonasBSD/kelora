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
text.contains(pattern)                Check if text contains pattern (builtin)
text.count(pattern)                   Count occurrences of pattern in text
text.decode_b64()                     Decode base64 string to text
text.decode_hex()                     Decode hexadecimal string to text
text.decode_url()                     Decode URL-encoded string
text.encode_b64()                     Encode text to base64 string
text.encode_hex()                     Encode text to hexadecimal string
text.encode_url()                     URL-encode text (percent encoding)
text.ending_with(suffix)              Return substring from start to end of suffix, else empty
text.escape_html()                    Escape HTML special characters (&, <, >, ", ')
text.escape_json()                    Escape JSON special characters
text.extract_all_re(pattern [, group]) Extract all regex matches as array
text.extract_domain()                 Extract domain from URL or email address
text.extract_ip()                     Extract first IP address from text
text.extract_ips()                    Extract all IP addresses as array
text.extract_re_maps(pattern, field)  Extract regex matches as array of maps for fan-out
text.extract_re(pattern [, group])    Extract regex match or capture group
text.extract_url()                    Extract first URL from text
text.has_matches(pattern)             Check if text matches regex pattern
text.index_of(pattern)                Find position of substring (-1 if not found) (builtin)
text.is_digit()                       Check if text contains only digits
text.is_private_ip()                  Check if IP is in private ranges
text.len                              Get string length (builtin)
text.lower()                          Convert text to lowercase
text.mask_ip([octets])                Mask IP address (default: last octet)
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
text.parse_json()                     Parse JSON string into map/array
text.replace(pattern, replacement)    Replace all occurrences of pattern (builtin)
text.slice(spec)                      Slice text using Python notation (e.g., "1:5", ":3", "-2:")
text.split_re(pattern)                Split text by regex pattern
text.split(separator)                 Split string into array by delimiter (builtin)
text.starting_with(prefix)            Return substring from prefix to end, else empty
text.strip([chars])                   Remove whitespace or specified characters
text.sub_string(start [, length])     Extract substring from position (builtin)
text.to_float()                       Convert text to float (0 on error)
text.to_int()                         Convert text to integer (0 on error)
text.to_lower()                       Convert to lowercase (builtin)
text.to_upper()                       Convert to uppercase (builtin)
text.to_number([default])             Safe number conversion with fallback (default: 0)
text.to_bool([default])               Safe boolean conversion with fallback
text.trim()                           Remove whitespace from start and end (builtin)
text.unescape_html()                  Unescape HTML entities to text
text.unescape_json()                  Unescape JSON escape sequences
text.upper()                          Convert text to uppercase
  
ARRAY FUNCTIONS:
array.all(|item| condition)          Check if all elements match condition (builtin)
array.contains_any(search_array)      Check if array contains any search values
array.contains(value)                 Check if array contains value (builtin)
array.filter(|item| condition)       Keep elements matching condition (builtin)
array.flatten([style [, max_depth]])  Flatten nested arrays/objects
array.join(separator)                 Join array elements with separator
array.len                             Get array length (builtin)
array.map(|item| expression)          Transform each element (builtin)
array.parse_cols(spec [, sep])        Apply column spec to pre-split values
array.percentile(pct)                 Calculate percentile of numeric array
array.pop()                           Remove and return last item (builtin)
array.push(item)                      Add item to end of array (builtin)
array.reduce(|acc, item| expr, init)  Aggregate array into single value (builtin)
array.reversed()                      Return new array in reverse order
array.some(|item| condition)          Check if any element matches condition (builtin)
array.sort()                          Sort array in place (builtin)
array.sorted_by(field)                Sort array of objects by field name
array.sorted()                        Return new sorted array (numeric/lexicographic)
array.min()                           Find minimum value in array (no auto string-to-number coercion)
array.max()                           Find maximum value in array (no auto string-to-number coercion)
array.starts_with_any(search_array)   Check if array starts with any search values
  
MAP/OBJECT FUNCTIONS:
map.flatten([separator [, style]])    Flatten nested object to dot notation
map.get_path("field.path" [, default]) Safe nested field access with fallback
map.has_path("field.path")            Check if nested field path exists
map.merge(other_map)                  Merge another map into this one
map.path_equals("path", value)        Safe nested field comparison
map.to_cef()                          Convert map to Common Event Format (CEF) string
map.to_combined()                     Convert map to Apache/Nginx combined log format
map.to_kv([sep [, kv_sep]])           Convert map to key-value string with separators
map.to_logfmt()                       Convert map to logfmt format string
map.to_syslog()                       Convert map to syslog format string
map.to_json([pretty])                 Convert map to JSON string
map.unflatten([separator])            Reconstruct nested object from flat keys
  
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

MATH FUNCTIONS:
abs(x)                                Absolute value of number
floor(x)                              Round down to nearest integer
mod(a, b) / a % b                     Modulo operation with division-by-zero protection
rand()                                Random float between 0 and 1
rand_int(min, max)                    Random integer between min and max (inclusive)
round(x)                              Round to nearest integer

UTILITY FUNCTIONS:
type_of(value)                        Get type name as string (builtin)
eprint(message)                       Print to stderr (suppressed with -qqq)
exit(code)                            Exit kelora with given exit code
get_env(var [, default])              Get environment variable with optional default
print(message)                        Print to stdout (suppressed with -qqq)
read_file(path)                       Read file contents as string
read_lines(path)                      Read file as array of lines
window_numbers(field)                 Get numeric field values from current window
window_values(field)                  Get field values from current window

TRACKING/METRICS FUNCTIONS:
track_bucket(key, bucket)             Track values in buckets for histograms
track_count(key)                      Increment counter for key by 1
track_max(key, value)                 Track maximum value for key
track_min(key, value)                 Track minimum value for key
track_sum(key, value)                 Accumulate numeric values for key
track_unique(key, value)              Track unique values for key

FILE OUTPUT (REQUIRES --allow-fs-writes):
append_file(path, text_or_array)       Append line(s) to file; arrays append one line per element
mkdir(path [, recursive])              Create directory (set recursive=true to create parents)
truncate_file(path)                    Create or zero-length a file for fresh output

EVENT MANIPULATION:
emit_each(array [, base_map])         Fan out array elements as separate events
e = ()                                Clear entire event (remove all fields)
e.field = ()                          Remove individual field from event
  
Examples:
# String processing with builtin and custom functions
e.clean_url = e.url.extract_domain().to_lower()
e.parts = e.message.split("|")  # Use builtin split
e.word_count = e.text.trim().split(" ").len  # Chain builtin functions

# Array processing with builtins and fan-out
e.tag_count = e.tags.len  # Use builtin len
e.error_tags = e.tags.filter(|tag| tag.contains("error"))  # Builtin filter
emit_each(e.items)  # Creates separate event for each item

# Type-safe parsing and validation
e.status_code = e.status.to_int()  # Parse safely
if type_of(e.level) == "string" { e.log_level = e.level.to_upper() }

# JSON parsing and serialization
e.parsed_data = e.json_field.parse_json()
e.json_output = e.data.to_json()

# Math functions
e.abs_value = abs(e.negative_number)
e.rounded = round(e.decimal_value)
e.floored = floor(e.decimal_value)

# Safe nested field access
e.user_role = e.get_path("user.profile.role", "guest")

# Datetime manipulation
let now = now_utc()
e.hour = now.hour()
e.formatted = now.format("%Y-%m-%d %H:%M")

# Tracking metrics
track_count("http_requests")
track_bucket("response_time", "slow")

# Format conversion and serialization
e.logfmt_output = e.to_logfmt()                          # Convert to logfmt
e.syslog_line = e.to_syslog()                            # Convert to syslog
e.cef_event = e.to_cef()                                 # Convert to CEF
e.access_log = e.to_combined()                           # Convert to combined log
e.custom_kv = e.to_kv("|", ":")                          # Custom key-value format

# Bidirectional processing (roundtrip)
let logfmt_string = e.to_logfmt()
e.parsed_back = logfmt_string.parse_logfmt()

# Random values
e.sample_rate = rand()
e.random_id = rand_int(1000, 9999)

Rhai lets you call functions as either `value.method(args)` or
`function(value, args)`. Examples prefer method calls, though some
function-style listings remain for clarity. Use 'e' to access the current event.
For more examples, see the documentation or use --help for general usage.
"#
}
