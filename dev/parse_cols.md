Absolutely. Here’s the **amended spec** with the new array overload baked in, plus a clear note on how the string forms can delegate to the array core while still preserving the nice “verbatim rest” behavior.

# `parse_cols` — Final Spec

## Purpose

Declaratively map “column-ish” logs to a `Map`. No guessing, no regex engines inside—just explicit shapes.

## API (Rhai)

```rhai
// 1) Whitespace mode: collapse runs of space/tab
str.parse_cols(spec) -> Map

// 2) Custom-separator mode: literal `sep`, keep empty columns
str.parse_cols(spec, sep) -> Map

// 3) Pre-split columns: apply the same spec to an Array<String>
Array<String>.parse_cols(spec) -> Map
```

## Spec grammar

Spec is space-separated tokens:

* `name` — consume 1 column into field `name`.
* `name(n)` — consume `n ≥ 2` columns; join rule below.
* `-` — skip 1 column (no field).
* `-(n)` — skip `n ≥ 2` columns.
* `*name` — consume the **rest** into `name`. Must be **last and unique**. Always succeeds; if nothing remains → `()` (unit).

Invalid: `name(1)`, `-(1)`, multiple stars, star not last.
`name` matches `[A-Za-z_][A-Za-z0-9_]*`.

## Tokenization & join rules

* **Whitespace mode (`str.parse_cols(spec)`):**

  * Split on `[ \t]+`, collapse runs, trim ends, no empty columns.
  * `name(n)` joins with a **single space**.
  * `*name` captures the tail **verbatim** from the original line (preserve spacing/punctuation).

* **Custom-sep mode (`str.parse_cols(spec, sep)`):**

  * Split on **literal** `sep` (UTF-8, 1+ bytes). **Keep empties** (leading/trailing/consecutive separators produce empty columns).
  * `name(n)` joins with the **same `sep`** (preserves structure).
  * `*name` captures the tail **verbatim** from the original line (from first unconsumed byte to end).

* **Array mode (`Array<String>.parse_cols(spec)`):**

  * Treat the array as **the columns**; no further splitting.
  * Empty strings are real columns.
  * `name(n)` joins with a **single space** (there is no authoritative separator here).
  * `*name` joins the remainder with a **single space**. If remainder is empty → `()`.

## Output & empties

* Returns a Rhai `Map` with exactly the declared fields.
* Shortages (not enough columns for a fixed token): fill what you can; missing fields become `()`.
* `*name` on an empty remainder → `()` (distinguishes “absent” from `""`).

## Error handling (strict/resilient)

Fits Kelora’s duality:

* **Too few columns** (before `*name`):

  * **Strict**: error `parse_cols: expected ≥ {need_min} columns (got {have})`.
  * **Resilient**: produce partial map; missing = `()`; attach `_warn`.

* **Extra columns** (no `*name` provided):

  * **Strict**: error `parse_cols: {extra} unconsumed columns; add *field or skip with -`.
  * **Resilient**: ignore extras; attach `_warn` with count.

## Examples

```rhai
// Whitespace + skip + star (verbatim tail)
"2025-09-22 12:33:44 -- INFO hello   world"
  .parse_cols("ts(2) - level *msg");
// { ts:"2025-09-22 12:33:44", level:"INFO", msg:"hello   world" }

// Custom sep; preserve empties; join with same sep
"2025-09-22|12:34:56|INFO||done"
  .parse_cols("ts(2) level *msg", "|");
// { ts:"2025-09-22|12:34:56", level:"INFO", msg:"|done" }

// Pre-split via regex captures; array overload
let caps = e.line.extract_all_re(r#"(\S+)\s+(\S+)\s+\[(.*?)\]\s+(.*)"#);
// -> [ip, user, ts, rest]
let m = caps.parse_cols("ip user ts *msg");
// { ip:..., user:..., ts:..., msg:... }

// Shortage (resilient): missing action → ()
["2025-09-22","INFO","alice"].parse_cols("ts level user action");
// { ts:"2025-09-22", level:"INFO", user:"alice", action:() }

// Skip many with -(n)
"2025 a b c d e f INFO msg".parse_cols("ts -(5) level msg");
// { ts:"2025", level:"INFO", msg:"msg" }
```

## Implementation notes (how the overloads can share one core)

You can keep one tiny engine and have the string forms delegate to it—**without** losing verbatim tail:

* Build a core function that consumes:

  * `cols: Vec<&str>` (the tokens),
  * **optional** `byte_starts: Vec<usize>` (start offsets per token into the original line),
  * an enum for **join policy** (`WhitespaceSpace`, `SepLiteral(sep)`, or `ArraySpace`).

* For the **string** overloads, perform tokenization and collect `byte_starts`. When you hit `*name`, slice `&line[byte_starts[i]..]` so the tail is **verbatim**.

* For the **array** overload, call the same core with `byte_starts=None` and `ArraySpace` join policy; `*name` will join remainder with a single space (by spec).

This way, all three overloads share one parser, but the string ones still get byte-accurate `*rest`.

## Tests to lock behavior

1. Spec parse errors: bad names, multiple stars, star not last, `name(1)`, `-(1)`.
2. Whitespace collapsing vs. custom-sep empties.
3. `-(n)` skipping interleaved with `name(n)`.
4. `*name` verbatim tail for string variants; space-joined tail for array variant.
5. Strict vs. resilient for shortages and extras.
6. Multi-byte `sep` splitting (`" :: "`), indices correct for verbatim tail.
7. Star-empty → `()`.
