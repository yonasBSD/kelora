# Absorb Functions

Absorb functions provide a streamlined way to extract structured data from event fields, merge it into the event, and clean up the source field - all in a single operation.

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

Parse key-value pairs from an event field, merge them into the event, and update the field with unparsed text.

### Signatures

```rhai
absorb_kv(field: string) -> bool
absorb_kv(field: string, sep: string, kv_sep: string) -> bool
absorb_kv(field: string, (), kv_sep: string) -> bool
```

**Parameters:**

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `field` | string | Field name to parse (e.g., `"msg"`) | Required |
| `sep` | string or `()` | Token separator; use `()` for whitespace | Whitespace |
| `kv_sep` | string | Key-value separator | `"="` |

**Returns:** `bool`
- `true` if any key-value pairs were found and merged
- `false` if no pairs found

**Method-style calls supported:**
```rhai
e.absorb_kv("msg")           // As method on event map
absorb_kv(e, "msg")          // As function
```

### Behavior

The function performs these steps:

#### 1. Extract and Validate Field

- Get value of the specified field
- If field doesn't exist → return `false` (no-op, no error)
- If field is not a string → return `false` (no-op, no error)

#### 2. Parse with Remainder Tracking

- Split text by separator (whitespace by default, or custom separator)
- For each token:
  - **Contains KV separator** (`=` by default): Parse as `key=value` pair
  - **Doesn't contain KV separator**: Keep as unparsed text

```rhai
"Payment timeout order=1234 gateway=stripe duration=5s"
// Tokens: ["Payment", "timeout", "order=1234", "gateway=stripe", "duration=5s"]
// Parsed: {order: "1234", gateway: "stripe", duration: "5s"}
// Unparsed: ["Payment", "timeout"]
```

#### 3. Merge Parsed Pairs into Event

- Each parsed key-value pair is inserted into the event
- **Overwrites existing fields** with same key (like `merge()`)

#### 4. Update Source Field

Two cases:

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

#### 5. Return Result

- Returns `true` if any pairs were parsed and merged
- Returns `false` if no pairs found

### Examples

#### Basic Usage

```rhai
e.msg = "Payment timeout order=1234 gateway=stripe duration=5s"
e.absorb_kv("msg")

// After:
// e.msg = "Payment timeout"
// e.order = "1234"
// e.gateway = "stripe"
// e.duration = "5s"
// Returns: true
```

#### All Tokens Are Pairs

When every token is a key-value pair, the field is deleted:

```rhai
e.data = "user=alice status=active count=42"
e.absorb_kv("data")

// After:
// e.data deleted (no longer exists)
// e.user = "alice"
// e.status = "active"
// e.count = "42"
// Returns: true
```

#### No Pairs Found

If no key-value pairs are found, the field remains unchanged:

```rhai
e.msg = "This is just plain text without any pairs"
e.absorb_kv("msg")

// After:
// e.msg = "This is just plain text without any pairs" (unchanged)
// Returns: false
```

#### Custom Separators

Parse with custom token and KV separators:

```rhai
e.tags = "env:prod,region:us-west,tier:web"
e.absorb_kv("tags", ",", ":")

// After:
// e.tags deleted
// e.env = "prod"
// e.region = "us-west"
// e.tier = "web"
// Returns: true
```

#### Whitespace Separator with Custom KV Separator

Use `()` to specify whitespace separator with custom KV separator:

```rhai
e.labels = "env:prod region:us tier:web"
e.absorb_kv("labels", (), ":")

// After:
// e.labels deleted
// e.env = "prod"
// e.region = "us"
// e.tier = "web"
```

#### Conditional Logic

Use the return value for conditional processing:

```rhai
// Try KV first, fall back to JSON if no pairs
if !e.absorb_kv("payload") {
    e.merge(e.payload.parse_json())
}
```

```rhai
// Only process events with KV data
if e.absorb_kv("msg") {
    print("Found structured data")
}
```

### Edge Cases

#### Field Doesn't Exist

No error, returns `false`:

```rhai
e.absorb_kv("missing_field")  // Returns false, no error
```

#### Field Is Not a String

No error, returns `false`:

```rhai
e.count = 42
e.absorb_kv("count")  // Returns false, no error
```

#### Empty or Whitespace-Only String

Field is deleted, returns `false`:

```rhai
e.msg = ""
e.absorb_kv("msg")  // Returns false, msg deleted

e.msg = "   "
e.absorb_kv("msg")  // Returns false, msg deleted
```

#### Key with Empty Value

Empty values are preserved:

```rhai
e.msg = "error= code=500"
e.absorb_kv("msg")

// After:
// e.msg deleted
// e.error = ""  (empty string)
// e.code = "500"
```

#### Key with No Value Separator

Tokens without the KV separator are kept as unparsed text:

```rhai
e.msg = "prefix key=value suffix"
e.absorb_kv("msg")

// After:
// e.msg = "prefix suffix"
// e.key = "value"
```

#### Conflicting Keys (Overwrites)

Absorb **overwrites existing fields** (same behavior as `merge()`):

```rhai
e.status = "pending"
e.msg = "Processing status=active"
e.absorb_kv("msg")

// After:
// e.msg = "Processing"
// e.status = "active"  (overwritten!)
```

#### Special Characters and Unicode

Handles Unicode and special characters in both keys and values:

```rhai
e.msg = "user=alice™ emoji=🎉 price=$99.99"
e.absorb_kv("msg")

// After:
// e.msg deleted
// e.user = "alice™"
// e.emoji = "🎉"
// e.price = "$99.99"
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

Unparsed tokens are **always joined with a single space**, regardless of the separator used for parsing.

**Rationale:**
- Simplest and most predictable behavior
- Works well for the common case (whitespace-separated logs)
- Avoids complexity of preserving original spacing

**Example:**
```rhai
e.tags = "important,urgent,user=alice,priority=high"
e.absorb_kv("tags", ",", "=")

// Unparsed: ["important", "urgent"]
// Joined with space (not comma):
// e.tags = "important urgent"
```

#### Error Handling

Follows Kelora's resilient error handling philosophy:

- **Never throws exceptions** in resilient mode
- Invalid field types → return `false`, no error
- Missing fields → return `false`, no error
- Empty results → return `false`, not an error

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
e.absorb_json("payload")

// After:
// e.payload deleted
// e.user = "alice"
// e.action = "login"
// e.timestamp = 1234567890
```

**Behavior differences from absorb_kv():**
- JSON parsing is all-or-nothing (no "unparsed text")
- Field always deleted on successful parse
- Parse failure → field unchanged, returns `false`

### absorb_logfmt()

Parse logfmt from field, merge into event, clean field:

```rhai
e.msg = 'prefix user="alice" status=active suffix'
e.absorb_logfmt("msg")

// After:
// e.msg = "prefix suffix"
// e.user = "alice"
// e.status = "active"
```

**Similar to absorb_kv()** but uses logfmt parser which handles quoted values.

### absorb_url_params()

Parse URL query parameters from field, merge into event, delete field:

```rhai
e.query = "foo=bar&baz=qux&limit=10"
e.absorb_url_params("query")

// After:
// e.query deleted
// e.foo = "bar"
// e.baz = "qux"
// e.limit = "10"
```

**All-or-nothing parsing** like JSON - entire string is the query string.

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
