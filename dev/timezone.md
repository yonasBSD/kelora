## Timestamp Parsing & Formatting in Kelora

---

### 🌍 Overview

This spec introduces **clear, separate controls** for:

1️⃣ **Parsing** timestamps (input stage, affects event data)

2️⃣ **Formatting** timestamps (output stage, display-only in `default` formatter)

---

### ⚙️ 1️⃣ Parsing Option

#### 🏷️ `--input-tz <tz>`

**Purpose:**
Set the assumed timezone for parsing **naive input timestamps** (timestamps without explicit timezone info).

* Default: `UTC`. This means if `--input-tz` is not specified, we now assume `UTC` (not local time, like we did before).
  This is to avoid consistency problems when logs are parsed on servers in multiple locations.
* Special value: `local` → system local timezone, `utc` → UTC
* Example values: `UTC`, `utc`, `local`, `Europe/Berlin`, `America/New_York`

✅ Affects:

* Parsing of timestamps during input stage
* Promotion to `FieldValue::DateTime`
* Time-based filters like `--since` / `--until`
* `parse_timestamp()` in Rhai scripts

❌ Does **not** affect display timezone (use `-z` / `-Z` for that)
❌ Does **not** modify explicitly timezone-stamped input

---

### ⚙️ 2️⃣ Formatting Options (Display-Only)

#### 🏷️ `--format-ts field1,field2,...`

Explicitly format **selected fields** in `default` output.

* For each field:

  * If `FieldValue::DateTime`: convert to **local time**, RFC3339 format
  * If `FieldValue::String`: attempt `parse_timestamp()`; if successful, convert to local RFC3339
  * Else: leave as-is

✅ Affects only **default output formatter** (human-readable display)
❌ Does **not** modify event data or structured outputs (jsonl, csv, logfmt, etc.)

---

#### 🏷️ `-z`

Auto-format **all known timestamp fields**:

* Includes promoted `ts` and all `FieldValue::DateTime` fields

✅ Convert to **local time**, RFC3339
✅ Display-only; no change to event data or structured outputs

---

#### 🏷️ `-Z`

Same as `-z`, but:

✅ Convert to **UTC**, RFC3339
✅ Display-only; no change to event data or structured outputs

---

### 🧠 Summary of Scope

| Option        | Stage      | Affects Event Data? | Affects Structured Output?  | Affects Display? |
| ------------- | ---------- | ------------------- | --------------------------- | ---------------- |
| `--input-tz`  | Parsing    | ✅ Yes               | ✅ Yes (original timestamps) | ❌ No             |
| `--format-ts` | Formatting | ❌ No                | ❌ No                        | ✅ Yes (default)  |
| `-z` / `-Z`   | Formatting | ❌ No                | ❌ No                        | ✅ Yes (default)  |

---

### 🌍 Example Usage

```bash
kelora logs.jsonl --input-tz Europe/Berlin --format-ts created_at,updated_at
```

➡ Parse naive timestamps as Europe/Berlin; format `created_at` + `updated_at` as local RFC3339 (display-only).

```bash
kelora logs.jsonl --input-tz UTC -z
```

➡ Parse naive timestamps as UTC; format all known timestamp fields as local RFC3339 (display-only).

```bash
kelora logs.jsonl --input-tz local -Z
```

➡ Parse naive timestamps as system local time; format all known timestamp fields as UTC RFC3339 (display-only).

---

### 💥 CLI Help Draft (Explicit)

```
--input-tz <tz>         Assume timezone for input timestamps without timezone info (default: UTC).
                        Use 'local' for system local time.
                        Examples: 'UTC', 'local', 'Europe/Berlin'.

--format-ts <fields>    Comma-separated list of fields to format as local RFC3339.
                        Only affects default output; does not modify event data.

-z                      Auto-format all known timestamp fields as local RFC3339.
                        Only affects default output; does not modify event data.

-Z                      Auto-format all known timestamp fields as UTC RFC3339.
                        Only affects default output; does not modify event data.
```

---

### ⚠️ Explicit Non-Goals

❌ No change to event data by `--format-ts`, `-z`, `-Z`
❌ No change to structured outputs like JSONL, CSV, logfmt
✅ Parsing timezones is **only** controlled by `--input-tz`

