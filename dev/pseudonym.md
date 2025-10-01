Got it. Here’s the cleaned-up spec with the function renamed to **`pseudonym()`**, reflecting what it really does.

---

# Kelora `pseudonym()` — Minimal Spec

## Purpose

Produce deterministic pseudonyms (stable tokens) for identifiers using a secret if provided, or an ephemeral key if not.
This is **pseudonymization**, not anonymization: tokens remain linkable within a domain and depend on the secret.

---

## API (Rhai)

```rhai
pseudonym(value: string, domain: string) -> string
```

* `domain`: **required, non-empty** (e.g., `"kelora:v1:email"`, `"kelora:v1:ip"`).
  Used for **domain separation** (prevents cross-field linking, allows versioning).

---

## Behavior

* **Always produces a pseudonym.** Never pass-through.
* **Key source:**

  * If `KELORA_SECRET` is **set** (non-empty): derive a master key once with Argon2id → tokens **stable across runs**.
  * If unset: generate a **random ephemeral** 32-byte key once at startup → tokens **not stable across runs**.

---

## Algorithm

1. **Master key (once at startup)**

   * Env set:
     `master = Argon2id(secret=KELORA_SECRET, salt="kelora:v1:master", m=64MiB, t=3, p=1)`
   * Env absent:
     `master = 32 random bytes from CSPRNG` (ephemeral)
2. **Per-domain key** (cached):
   `k = HKDF-SHA256(ikm=master, info="kelora:v1:" + domain)`
3. **Token per call:**
   `tag = HMAC-SHA-256(key=k, data=domain || value)`
   `token = base64url_unpadded(tag)[0..24]` (fixed 24 chars)

---

## Logging (stderr, once at startup)

* Env set:
  `pseudonym: ON (stable; KELORA_SECRET)`
* Env absent:
  `pseudonym: ON (ephemeral; not stable)`

---

## Errors

* Empty domain → fatal: `pseudonym: domain must be non-empty`
* `KELORA_SECRET` present but empty → fatal: `KELORA_SECRET must not be empty`
* Init failures (Argon2/HKDF/HMAC) → fatal: `pseudonym init failed`

---

## Determinism & Rotation

* Same `(value, domain)` within a run → same token.
* Same `(value, domain, KELORA_SECRET)` across runs → same token.
* Changing `KELORA_SECRET` or bumping `domain` (e.g., `v1→v2`) → tokens intentionally change.

---

## Performance

* Argon2id once at startup (if env set): ~50–150 ms on modern CPUs.
* Ephemeral path: negligible startup.
* Per call: HMAC in microseconds.

---

## Acceptance Tests

1. Env absent: tokens differ across runs, match within one run.
2. Env set: tokens identical across machines for same `(value, domain, secret)`.
3. Domain separation: same secret, `pseudonym(v,"A") != pseudonym(v,"B")`.
4. Empty domain or empty secret → fatal error.

---

## Notes

* **This is pseudonymization, not anonymization.** Tokens are linkable within a domain.
* To prevent cross-linking between datasets, include dataset/export info in the domain (e.g., `"kelora:v1:email:export-2025-10-01"`).
* Output length (24 chars) balances compactness with negligible collision risk.

