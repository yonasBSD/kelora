# Kelora Built-in Patterns & Helpers

## Purpose

Provide a tiny, baked-in “patterns stdlib” for common matching and extraction tasks without adding new flags or a module system. Keep it KISS: a few globals that work in every Rhai stage.

## Scope (Goals / Non-Goals)

* **Goals**

  * Zero-setup regex constants for common things (URLs, emails, IPs, etc.).
  * One-liner helper to **fan out** regex matches (`emit_matches`).
  * Simple boolean helper for **filtering** (`has_match`).
  * Available **globally** in all Rhai evaluation contexts (filter, exec, map, etc.).
* **Non-Goals**

  * No new CLI flags, subcommands, or “import” syntax.
  * No namespacing/module system.
  * No exotic patterns (IBANs, ARNs, etc.). Keep the baked set small and conservative.

---

## Public Surface

### Regex constants (strings)

Registered as **globals** (no namespace):

* `RE_URL` — `r"(?i)\bhttps?://[^\s\"'<>]+"`
* `RE_EMAIL` — `r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b"`
* `RE_IPV4` — `r"\b(?:\d{1,3}\.){3}\d{1,3}\b"`
* `RE_IPV6` — a conservative compressed/expanded IPv6 matcher
  `r"\b(?:[A-F0-9]{1,4}:){2,7}[A-F0-9]{1,4}\b(?i)"`
* `RE_FQDN` — `r"\b(?:(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)\.)+[a-z]{2,}\b"`
* `RE_PATH_UNIX` — `r"(?:(?:/|~)[^\s\"'<>]+)"`
* `RE_PATH_WIN` — `r"\b[A-Za-z]:\\[^\s\"'<>]+"`
* `RE_UUID` — `r"\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b(?i)"`
* `RE_ERR` — conservative error tokens
  `r"\b(?:panic|fatal|error|exception|traceback|stack\s*overflow)\b(?i)"`

> Notes:
>
> * These are intentionally **simple** and “good enough for logs.” They are not RFC-complete.
> * Case-insensitive flags are embedded via `(?i)` to avoid engine-level switches.

### Helper functions

Registered as **globals**:

1. `has_match(s: string, re: string) -> bool`
   Sugar for: `s.extract_re(re) != ""`.

2. `emit_matches(s: string, re: string, key: string) -> int`

   * Extracts all matches with `extract_all_re(s, re)`.
   * Builds `[{ key: match }, …]`.
   * Fans out with `emit_each(items)`.
   * **Suppresses the original event** (per `emit_each` semantics).
   * Returns the number of emitted events.

3. `emit_matches(s: string, re: string, key: string, base: map) -> int`
   Same as above, but merges each `{ key: match }` into `base` and calls `emit_each(items, base)`.

> No `extract_all_to_maps()` helper is shipped. When needed, users can do the two-liner inline:
>
> ```rhai
> let items = extract_all_re(s, RE_URL).map(|m| #{ url: m });
> emit_each(items);
> ```

---

## Semantics & Behavior

* **Availability:** All constants and helpers are registered into every Rhai `Engine` Kelora instantiates (filter/map/exec/etc.). No dependence on `--begin` or the `conf` map.
* **Purity:** Helpers are stateless and deterministic. They do not mutate global state.
* **Suppression:** `emit_matches` relies on `emit_each`’s contract to suppress the source event. This is deliberate and documented.
* **Performance:** Accept the small overhead of compiling regexes by pattern string. (Optional internal cache keyed by string is allowed later without API change.)
* **Safety:** Regexes are bounded (no catastrophic backtracking known for these shapes). If `extract_all_re` errors on an invalid regex, the error propagates as today.

---

## CLI Examples

### Grep-style keep/ignore

```bash
# Keep lines containing a URL
kelora -f line --filter 'has_match(e.line, RE_URL)'

# Ignore lines that look like FQDNs
kelora -f line --filter '!has_match(e.line, RE_FQDN)'
```

### Extract values only (emails from raw lines)

```bash
kelora -f line \
  -e 'emit_matches(e.line, RE_EMAIL, "email")' \
  -k email -b
```

### Extract with context (URLs from JSON field, keep user + ts)

```bash
kelora -f json \
  -e 'let base = #{user: e.user_id, ts: e.ts};
      emit_matches(e.msg, RE_URL, "url", base)' \
  -F json
```

### Multiple extractions (IPv4s and error tokens)

```bash
kelora -f line \
  -e 'emit_matches(e.line, RE_IPV4, "ipv4")' \
  -e 'emit_matches(e.line, RE_ERR,  "err")' \
  -k ipv4,err
```

---

## Configuration / Toggling

* **Env:** `KELORA_NO_STD=1` disables registering all built-ins (constants + helpers).
* **Flag (optional):** `--no-std` (global) mirrors the env var.
* If disabled, scripts must rely solely on user-provided regexes and helpers.

---

## Documentation

* **`--help-rhai` (or help screen):** Add a concise section “Built-in Regexes & Helpers” listing:

  * The constants (one-line descriptions).
  * `has_match(s,re)` and `emit_matches(s,re,key[,base])` with 2–3 short examples.
* **Manpage / README:** One pagelet showing parity with “klp built-ins” using these primitives.
* **Change log:** “Added minimal built-in regex constants and two helpers; no new flags.”

---

## Testing

* **Unit tests (Rhai):**

  * `has_match("visit https://a.b", RE_URL) == true`
  * `has_match("foo", RE_URL) == false`
  * `emit_matches("a b a", r"\ba\b", "tok")` returns `2`
* **Integration (CLI):**

  * Pipe known fixtures; assert counts/fields with `-F json | jq`.
  * Verify `--no-std` removes globals (scripts referencing them fail clearly).
* **Perf sanity:** Run against a 1–5 GB log; ensure no regressions >1–2% vs. baseline.

---

## Backward Compatibility

* No behavior change unless a script’s identifiers collide with new globals. Given the names, risk is minimal. If needed, users can disable via `--no-std` or rename their variables.

---

## Future-Proofing (Optional)

* If we ever need to tweak patterns, add an internal version tag (exposed as `PATTERNS_REV` string) for debugging only.
* Keep the baked set small. Point power users to a community “patterns-extra.rhai” for exotic needs.

