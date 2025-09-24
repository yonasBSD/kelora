Heck yes—methods it is. Here’s a tight draft you can drop into Kelora without waking the feature-creep goblin.

# Micro-Search Methods (Rhai)

## Surface API (Rhai)

On **String** values:

* `str.like(pattern: string) -> bool`
  Glob matching: `*` = any sequence, `?` = any single char. Case-sensitive.
* `str.ilike(pattern: string) -> bool`
  Same as `like` but case-insensitive (Unicode simple fold).
* `str.match(pattern: string) -> bool`
  Regex “find” (not anchored). Case-sensitive.
* `str.imatch(pattern: string) -> bool`
  Regex “find”, case-insensitive.

On **maps/events**:

* `e.has(key: string) -> bool`
  Top-level existence check (sugar for `"key" in e`). Keeps beginners out of `"field" in e` syntax.

### Examples

```bash
# substring/glob
kelora -f json --filter 'e.msg.ilike("*timeout*")'

# regex, case-insensitive
kelora -f json --filter 'e.msg.imatch("user\\s+not\\s+found")'

# field presence + simple glob
kelora -f logfmt --filter 'e.has("user") && e.user.like("alice*")'
```

---

## Semantics (precise but boring)

### `like` / `ilike` (glob)

* Supported wildcards: `*` (0+ chars), `?` (exactly 1 char).
* **No** `[]` character classes (keep it KISS and fast).
* `like`: byte-wise compare; `ilike`: Unicode case-folded (Rust `to_lowercase()` on both haystack + pattern once).
* Pattern is matched against the **entire** string (implicit `^...$` semantics).

### `match` / `imatch` (regex)

* Engine: `regex` crate.
* Uses **search** (find) semantics, not anchored. Users can write `^…$` if they want anchoring.
* `imatch` compiles with case-insensitive flag (equivalent to `(?i)`).

### `e.has(key)`

* Checks only **top-level** keys on the event/map.
* Equivalent to `"key" in e`, but clearer to read.
* For nested: users should continue to use `e.has_path("a.b[0].c")` (already documented).

---

## Rust API (binding to Rhai)

```rust
use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::sync::Mutex;
use rhai::{Engine, Map, Dynamic};

// ---------- Regex cache ----------
static RE_CACHE: Lazy<Mutex<HashMap<(String, bool), Regex>>> =
    Lazy::new(|| Mutex::new(HashMap::with_capacity(128)));

fn re_find(s: &str, pat: &str, case_insensitive: bool) -> bool {
    let key = (pat.to_string(), case_insensitive);
    let re = {
        let mut cache = RE_CACHE.lock().unwrap();
        if let Some(r) = cache.get(&key) { r.clone() } else {
            let r = RegexBuilder::new(&pat)
                .case_insensitive(case_insensitive)
                .build()
                .unwrap_or_else(|_| Regex::new("$^").unwrap()); // never matches on invalid regex
            cache.insert(key.clone(), r.clone());
            r
        }
    };
    re.find(s).is_some()
}

// ---------- Glob (*, ?) ----------
fn glob_like(s: &str, pat: &str, case_insensitive: bool) -> bool {
    fn norm(x: &str, ci: bool) -> std::borrow::Cow<'_, str> {
        if ci { x.to_lowercase().into() } else { x.into() }
    }
    let s = norm(s, case_insensitive);
    let p = norm(pat, case_insensitive);

    // Simple iterative matcher, supports * and ?
    let (mut si, mut pi, mut star_pi, mut star_si) = (0usize, 0usize, None, 0usize);
    let sb = s.as_bytes();
    let pb = p.as_bytes();

    while si < sb.len() {
        if pi < pb.len() && (pb[pi] == b'?' || pb[pi] == sb[si]) {
            si += 1; pi += 1;
        } else if pi < pb.len() && pb[pi] == b'*' {
            star_pi = Some(pi);
            star_si = si;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_si += 1;
            si = star_si;
        } else {
            return false;
        }
    }
    while pi < pb.len() && pb[pi] == b'*' { pi += 1; }
    pi == pb.len()
}

// ---------- Rhai-callable wrappers ----------
fn str_like(s: &str, pat: &str) -> bool { glob_like(s, pat, false) }
fn str_ilike(s: &str, pat: &str) -> bool { glob_like(s, pat, true) }
fn str_match(s: &str, re: &str) -> bool { re_find(s, re, false) }
fn str_imatch(s: &str, re: &str) -> bool { re_find(s, re, true) }
fn map_has(m: &Map, key: &str) -> bool { m.contains_key(key) }

// ---------- Registration ----------
pub fn register_search_methods(engine: &mut Engine) {
    engine.register_fn("like", str_like);
    engine.register_fn("ilike", str_ilike);
    engine.register_fn("match", str_match);
    engine.register_fn("imatch", str_imatch);
    engine.register_fn("has", map_has);
}
// Rhai treats functions as methods when the first argument’s type matches the receiver,
// so `"abc".like("*")` and `e.has("field")` work.
```

**Notes**

* `Regex` is `Clone`, cloning is cheap; cache keeps compiled forms. Start with a plain `HashMap`; if you want, swap to a tiny LRU later.
* Invalid regexes return `false` without aborting (consistent with resilient mode); in `--strict` you might prefer surfacing the error—your call.

---

## Tests (unit + integration)

### Unit (Rhai engine)

```rust
#[test]
fn test_like_basic() {
    let mut eng = Engine::new();
    register_search_methods(&mut eng);
    assert!(eng.eval::<bool>(r#""hello".like("he*o")"#).unwrap());
    assert!(!eng.eval::<bool>(r#""hello".like("he?lo!")"#).unwrap());
}

#[test]
fn test_ilike_unicode() {
    let mut eng = Engine::new();
    register_search_methods(&mut eng);
    assert!(eng.eval::<bool>(r#""Straße".ilike("strasse")"#).unwrap()); // simple fold pass
}

#[test]
fn test_match_find() {
    let mut eng = Engine::new();
    register_search_methods(&mut eng);
    assert!(eng.eval::<bool>(r#""user not found".match("not\\s+f")"#).unwrap());
    assert!(!eng.eval::<bool>(r#""abc".match("^z")"#).unwrap());
}

#[test]
fn test_imatch_flag() {
    let mut eng = Engine::new();
    register_search_methods(&mut eng);
    assert!(eng.eval::<bool>(r#""Timeout".imatch("timeout")"#).unwrap());
}

#[test]
fn test_map_has() {
    let mut eng = Engine::new();
    register_search_methods(&mut eng);
    let ok = eng.eval::<bool>(r#"let e = #{user: "alice"}; e.has("user")"#).unwrap();
    assert!(ok);
}
```

### CLI smoke (integration)

```
# glob
echo '{"msg":"read timeout"}' \
 | kelora -f json --filter 'e.msg.ilike("*timeout*")' -J

# regex
echo '{"msg":"user not found"}' \
 | kelora -f json --filter 'e.msg.imatch("user\\s+not\\s+found")' -J

# field presence
echo 'level=info msg="started" user=alice' \
 | kelora -f logfmt --filter 'e.has("user") && e.user.like("ali*")'
```

---

## Docs (one tiny box)

**Quick Searching**

* Substring/glob: `e.msg.like("error*")`, case-insensitive: `ilike`
* Regex search: `e.msg.match("timeout|failed")`, case-insensitive: `imatch`
* Field presence: `e.has("user")` (top-level). For nested use `e.has_path("user.name")`.

---

## Why this stays KISS

* No new flag, no new DSL. Power lives in `--filter`.
* Clear names, method style, zero gotchas.
* Fast paths: `like/ilike` avoid regex cost; regex is cached.

If you nod, I can convert this into a PR-ready patch layout (module file, registration call in your `engine.rs`, tests under `tests/`).
