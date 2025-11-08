# Absorb Functions

Absorb functions provide a streamlined way to extract structured data from event fields, merge it into the event, and (optionally) clean up the source field - all in a single operation.

## Overview

A common pattern in log processing is having mixed-content messages that contain both human-readable text and structured key-value data:

```
"Payment timeout order=1234 gateway=stripe duration=5s"
```

Traditionally, extracting this structured data requires multiple steps:

```rhai
// Traditional approach (3 steps)
let kv = e.msg.parse_kv()  // 1. Parse
e.merge(kv)                 // 2. Merge
e.msg = e.msg.before("order=")  // 3. Manually strip (complex!)
```

Absorb functions combine all these steps into one:

```rhai
// Absorb approach (1 step)
e.absorb_kv("msg")
// Result: e.msg = "Payment timeout", e.order = "1234", e.gateway = "stripe", e.duration = "5s"
```

## absorb_kv()

Parse key-value pairs from an event field, merge them into the event, and update the field with unparsed text. Returns a status record so scripts can react without guessing.

### Signatures

Kelora standardizes on a single, options-driven call:

```rhai
absorb_kv(field: string, options: map = #{}) -> AbsorbResult
```

All optional behavior is expressed through the `options` map—there is no positional `sep`/`kv_sep` overload.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `field` | string | Field name to parse (e.g., `"msg"`) |
| `options` | map | Optional behavior tweaks (see below) |

### Options

Absorb functions share a common options map so scripts can set behavior once and reuse it across formats. Options that do not apply to a format are simply ignored.

| Option | Type | Default | Applies to | Effect |
|--------|------|---------|-----------|--------|
| `sep` | string or `()` | Whitespace | Tokenized formats (KV, logfmt) | Token separator; use `()` for whitespace |
| `kv_sep` | string | `"="` | Tokenized formats (KV, logfmt) | Key-value separator |
| `keep_source` | bool | `false` | All | Leave the source field untouched; use the return value's `remainder` when you need the cleaned text |
| `overwrite` | bool | `true` | All | When `true`, parsed values overwrite existing event fields. When `false`, existing fields are preserved and conflicting keys are skipped during merge |

### Return Value

`AbsorbResult` is a record with the following fields:

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | One of `"applied"`, `"missing_field"`, `"not_string"`, `"empty"`, or `"parse_error"` |
| `data` | map | All parsed key-value pairs (only populated when `status == "applied"`) |
| `remainder` | string or `()` | The leftover text that was not parsed; `()` when no remainder |
| `removed_source` | bool | `true` when the field was deleted after parsing every token |
| `error` | string or `()` | Human-readable parse failure when `status == "parse_error"`; `()` otherwise |

**Status guide:**
- `applied`: At least one key-value pair was merged into the event.
- `missing_field`: The target field is absent.
- `not_string`: The field exists but is not a string.
- `empty`: The field is a string but produced no pairs after trimming (covers whitespace-only and “no pairs” scenarios).
- `parse_error`: Parser rejected the payload (all-or-nothing formats) and the field was left untouched; `error` contains the message.

**Note:** `AbsorbResult` is shared across all `absorb_*()` functions (JSON, logfmt, URL params, etc.). For all-or-nothing formats like JSON or URL parameters, `remainder` is always `()`, and `parse_error` includes a descriptive `error` string.

Method-style calls are still supported:

**Method-style calls supported:**
```rhai
e.absorb_kv("msg")           // As method on event map
absorb_kv(e, "msg")          // As function
```

### Behavior

The function performs these steps:

#### 1. Extract and Validate Field

- Get value of the specified field
- If field doesn't exist → return result with `status = "missing_field"`
- If field is not a string → return result with `status = "not_string"`

#### 2. Parse with Remainder Tracking

- Split text by separator (whitespace by default, or custom separator)
- For each token:
  - **Contains KV separator** (`=` by default): Parse as `key=value` pair
  - **Doesn't contain KV separator**: Keep as unparsed text

```rhai
"Payment timeout order=1234 gateway=stripe duration=5s"
// Tokens: ["Payment", "timeout", "order=1234", "gateway=stripe", "duration=5s"]
// Parsed data: {order: "1234", gateway: "stripe", duration: "5s"}
// Unparsed: ["Payment", "timeout"]
```

#### 3. Merge Parsed Pairs into Event

- Each parsed key-value pair is inserted into the event
- **Overwrites existing fields** with same key (like `merge()`) by default
- Set `overwrite: false` to preserve existing fields when conflicts occur

#### 4. Update Source Field

Unless `keep_source` is enabled, the source field is updated according to:

**Unparsed tokens remain:**
- Join unparsed tokens with single space
- Update field with this remainder
```rhai
e.msg = "Payment timeout order=1234"
e.absorb_kv("msg")
// → e.msg = "Payment timeout"
```

**All tokens were pairs:**
- Delete field entirely
```rhai
e.data = "user=alice status=active"
e.absorb_kv("data")
// → e.data deleted (field removed from event)
```

When `keep_source` is `true`, the source field is never modified; use `res.remainder` if you need the cleaned text.

#### 5. Return Result

- Returns `AbsorbResult` so scripts can inspect `status`, `data`, and `remainder`
- `status == "applied"` when at least one pair was merged
- Non-`"applied"` statuses indicate why nothing changed

### Examples

#### Basic Usage

```rhai
e.msg = "Payment timeout order=1234 gateway=stripe duration=5s"
let res = e.absorb_kv("msg")

// After:
// e.msg = "Payment timeout"
// e.order = "1234"
// e.gateway = "stripe"
// e.duration = "5s"
// res.status == "applied"
// res.data == #{ order: "1234", gateway: "stripe", duration: "5s" }
// res.remainder == "Payment timeout"
```

#### All Tokens Are Pairs

When every token is a key-value pair, the field is deleted:

```rhai
e.data = "user=alice status=active count=42"
let res = e.absorb_kv("data")

// After:
// e.data deleted (no longer exists)
// e.user = "alice"
// e.status = "active"
// e.count = "42"
// res.removed_source == true
// res.remainder == ()
```

#### No Pairs Found

If no key-value pairs are found, the field remains unchanged:

```rhai
e.msg = "This is just plain text without any pairs"
let res = e.absorb_kv("msg")

// After:
// e.msg = "This is just plain text without any pairs" (unchanged)
// res.status == "empty"
// res.data == #{}
```

#### Custom Separators

Parse with custom token and KV separators:

```rhai
e.tags = "env:prod,region:us-west,tier:web"
let res = e.absorb_kv("tags", #{ sep: ",", kv_sep: ":" })

// After:
// e.tags deleted (all tokens were KV pairs)
// e.env = "prod"
// e.region = "us-west"
// e.tier = "web"
// res.status == "applied"
```

#### Custom Separator with Mixed Content

When mixing plain tokens and KV pairs, format is preserved:

```rhai
e.categories = "news,sports,user:alice,region:us-west"
let res = e.absorb_kv("categories", #{ sep: ",", kv_sep: ":" })

// After:
// e.categories = "news,sports" (comma-separated, format preserved!)
// e.user = "alice"
// e.region = "us-west"
// res.remainder == "news,sports"
```

#### Whitespace Separator with Custom KV Separator

Use `sep: ()` in the options map to specify whitespace separator with custom KV separator:

```rhai
e.labels = "env:prod region:us tier:web"
let res = e.absorb_kv("labels", #{ sep: (), kv_sep: ":" })

// After:
// e.labels deleted
// e.env = "prod"
// e.region = "us"
// e.tier = "web"
```

#### Keeping the Source Field

Prevent destructive updates by enabling `keep_source`:

```rhai
e.msg = "Payment timeout order=1234"
let res = e.absorb_kv("msg", #{ keep_source: true })

// After:
// e.msg stays "Payment timeout order=1234"
// e.order == "1234"
// res.remainder == "Payment timeout"
```

#### Avoiding Overwrites

Preserve existing fields by disabling overwrite:

```rhai
e.order = "legacy"
e.msg = "order=1234 duration=5s"
let res = e.absorb_kv("msg", #{ overwrite: false })

// After:
// e.order is still "legacy" (not overwritten)
// e.duration == "5s" (new field added)
// res.data == #{ order: "1234", duration: "5s" } (shows all parsed data)
```

`res.data` always reports what was parsed, even if `overwrite: false` prevents conflicting keys from being written, so inspect the event map when you need to know which fields actually changed.

#### Conditional Logic

Use the return value for conditional processing:

```rhai
// Try KV first, fall back to JSON if no pairs
let res = e.absorb_kv("payload")
if res.status != "applied" {
    e.merge(e.payload.parse_json())
}
```

```rhai
// Only process events with KV data
let res = e.absorb_kv("msg")
if res.status == "applied" {
    print("Found structured data")
}
```

### Edge Cases

#### Field Doesn't Exist

No error; result reports `status = "missing_field"`:

```rhai
let res = e.absorb_kv("missing_field")
assert(res.status == "missing_field")
```

#### Field Is Not a String

No error; result reports `status = "not_string"`:

```rhai
e.count = 42
let res = e.absorb_kv("count")
assert(res.status == "not_string")
```

#### Empty or Whitespace-Only String

`status = "empty"` and the field is deleted (unless `keep_source` is set):

```rhai
e.msg = ""
let res = e.absorb_kv("msg")
assert(res.status == "empty")

e.msg = "   "
let res2 = e.absorb_kv("msg")
assert(res2.status == "empty")
```

#### Key with Empty Value

Empty values are preserved:

```rhai
e.msg = "error= code=500"
let res = e.absorb_kv("msg")

// After:
// e.msg deleted
// e.error = ""  (empty string)
// e.code = "500"
// res.data.error == ""
```

#### Key with No Value Separator

Tokens without the KV separator are kept as unparsed text:

```rhai
e.msg = "prefix key=value suffix"
let res = e.absorb_kv("msg")

// After:
// e.msg = "prefix suffix"
// e.key = "value"
// res.remainder == "prefix suffix"
```

#### Conflicting Keys (Overwrites)

By default absorb **overwrites existing fields** (same behavior as `merge()`), but `overwrite: false` preserves existing values:

```rhai
e.status = "pending"
e.msg = "Processing status=active"

// Default: overwrites existing
e.absorb_kv("msg")
assert(e.status == "active")        // overwritten

// Reset for second example
e.status = "pending"
e.msg = "Processing status=active"

// With overwrite: false, keeps existing
let res = e.absorb_kv("msg", #{ overwrite: false })
assert(res.data.status == "active") // parsed data available
assert(e.status == "pending")       // unchanged - existing preserved
```

#### Special Characters and Unicode

Handles Unicode and special characters in both keys and values:

```rhai
e.msg = "user=alice™ emoji=🎉 price=$99.99"
let res = e.absorb_kv("msg")

// After:
// e.msg deleted
// e.user = "alice™"
// e.emoji = "🎉"
// e.price = "$99.99"
// res.data.price == "$99.99"
```

### Comparison with Manual Approach

#### Before: Manual Parse + Merge

```rhai
e.msg = "Payment timeout order=1234 gateway=stripe"

// Step 1: Parse
let kv = e.msg.parse_kv()  // {order: "1234", gateway: "stripe"}

// Step 2: Merge
e.merge(kv)

// Step 3: Clean up (complex!)
// Problem: e.msg still = "Payment timeout order=1234 gateway=stripe"
// Need manual string manipulation:
e.msg = e.msg.before("order=").strip()  // Fragile! What if order appears in text?
// Or complex regex replacement...
```

#### After: Single Absorb Call

```rhai
e.msg = "Payment timeout order=1234 gateway=stripe"
e.absorb_kv("msg")

// Done!
// e.msg = "Payment timeout"
// e.order = "1234", e.gateway = "stripe"
```

### Implementation Notes

#### Join Separator for Unparsed Tokens

Unparsed tokens are **joined using the same separator that was used for splitting**.

**Rationale:**
- Preserves format fidelity (comma-separated stays comma-separated)
- Enables round-tripping and further processing
- Whitespace mode still normalizes to single space (expected behavior)

**Rules:**
- **Whitespace mode** (`sep = ()`): Join with single space
- **Custom separator** (`sep = ","`, `":"`, etc.): Join with same separator
- **Token processing**: Tokens are trimmed before classification; empty tokens are filtered out

**Example with custom separator:**
```rhai
e.tags = "important,urgent,user=alice,priority=high"
e.absorb_kv("tags", #{ sep: ",", kv_sep: "=" })

// Unparsed: ["important", "urgent"]
// Joined with comma (same as split separator):
// e.tags = "important,urgent"
```

**Example with whitespace:**
```rhai
e.msg = "Error   occurred    code=500"
e.absorb_kv("msg")

// Unparsed: ["Error", "occurred"]
// Joined with single space (normalized):
// e.msg = "Error occurred"
```

#### Token Processing and Normalization

Before classifying tokens as KV pairs or remainder, each token undergoes processing:

**Steps:**
1. **Split** by separator (whitespace or custom string)
2. **Trim** each token (remove leading/trailing whitespace)
3. **Filter** empty tokens (from consecutive separators like `"tag1,,tag3"`)
4. **Classify** as KV pair (contains `kv_sep`) or remainder
5. **Join** remainder using same separator

**Edge cases handled:**

```rhai
// Leading/trailing separators
e.data = ",tag1,tag2,owner=alice,"
e.absorb_kv("data", #{ sep: ",", kv_sep: "=" })
// Split: ["", "tag1", "tag2", "owner=alice", ""]
// After trim+filter: ["tag1", "tag2", "owner=alice"]
// Result: e.data = "tag1,tag2"

// Inconsistent spacing with custom separator
e.tags = "error, warning, code=500"
e.absorb_kv("tags", #{ sep: ",", kv_sep: "=" })
// Split: ["error", " warning", " code=500"]
// After trim: ["error", "warning", "code=500"]
// Classified: KV={code:"500"}, remainder=["error","warning"]
// Result: e.tags = "error,warning"

// Whitespace mode normalizes all whitespace
e.msg = "Error\n\toccurred\t\tuser=alice"
e.absorb_kv("msg")
// Split by any whitespace: ["Error", "occurred", "user=alice"]
// Remainder joined with single space: "Error occurred"
```

#### Error Handling

Follows Kelora's resilient error handling philosophy:

- **Never throws exceptions** in resilient mode
- Invalid field types → `status = "not_string"`, no error
- Missing fields → `status = "missing_field"`, no error
- Empty results → `status = "empty"`, not an error
- Parsers that fail (e.g., logfmt quotes, JSON syntax) return `status = "parse_error"` and populate `error` with the failure message

This ensures `absorb_kv()` can be used safely in pipelines without breaking on unexpected data.

#### Performance

The function performs a single pass through the text:
1. One split operation
2. One pass to classify tokens (pair vs. unparsed)
3. Insertion into event map (O(1) per key)
4. One join for unparsed tokens

Overall: O(n) where n is the number of tokens.

## Future Extensions

The absorb pattern can be extended to other formats:

### absorb_json()

Parse JSON from field, merge into event, delete field:

```rhai
e.payload = '{"user":"alice","action":"login","timestamp":1234567890}'
let res = e.absorb_json("payload")

// After:
// e.payload deleted
// e.user = "alice"
// e.action = "login"
// e.timestamp = 1234567890
// res.status == "applied"
// res.data == #{ user: "alice", action: "login", timestamp: 1234567890 }
// res.remainder == ()  (always () for JSON)
```

**Options:** Supports the shared options map. `keep_source` lets you retain the original JSON string, and `overwrite` controls merge conflicts. `sep` / `kv_sep` are ignored.

**Behavior differences from absorb_kv():**
- JSON parsing is all-or-nothing (no "unparsed text")
- Field always deleted on successful parse
- Parse failure → `status = "parse_error"`, field unchanged, and `res.error` carries the parser message

### absorb_logfmt()

Parse logfmt from field, merge into event, clean field:

```rhai
e.msg = 'prefix user="alice" status=active suffix'
let res = e.absorb_logfmt("msg")

// After:
// e.msg = "prefix suffix"
// e.user = "alice"
// e.status = "active"
// res.status == "applied"
// res.data == #{ user: "alice", status: "active" }
// res.remainder == "prefix suffix"
```

**Options:** Honors the same options map. `sep`/`kv_sep` customize token parsing, while `keep_source` and `overwrite` behave identically to `absorb_kv()`.

**Similar to absorb_kv()** but uses logfmt parser which handles quoted values.

### absorb_url_params()

Parse URL query parameters from field, merge into event, delete field:

```rhai
e.query = "foo=bar&baz=qux&limit=10"
let res = e.absorb_url_params("query")

// After:
// e.query deleted
// e.foo = "bar"
// e.baz = "qux"
// e.limit = "10"
// res.status == "applied"
// res.data == #{ foo: "bar", baz: "qux", limit: "10" }
// res.remainder == ()  (always () for URL params)
```

**Options:** Shares the same options map. `keep_source` preserves the original query string, `overwrite` guards existing fields, and tokenization options are ignored.

**All-or-nothing parsing** like JSON - entire string is the query string.
- Parse failure → `status = "parse_error"`, field unchanged, and `res.error` carries the parser message.

### Format-Specific Behavior Summary

| Format | Unparsed Text Behavior | Field Deletion |
|--------|------------------------|----------------|
| **KV** | Kept in field | Only if all tokens are pairs |
| **Logfmt** | Kept in field | Only if entire string is logfmt |
| **JSON** | N/A (all-or-nothing) | Always on success |
| **URL params** | N/A (all-or-nothing) | Always on success |

## See Also

- [parse_kv()](../reference/cli-reference.md#parse_kv) - Parse KV pairs without modifying source
- [merge()](../reference/cli-reference.md#merge) - Merge maps into events
- [Error Handling](../concepts/error-handling.md) - Kelora's error handling philosophy
- [Rhai Functions](../reference/rhai-functions.md) - Complete function reference
