🔐 Hashing & Anonymization Functions in Kelora

Kelora provides four distinct hashing and pseudonymization functions to support fast bucketing, deterministic grouping, and secure anonymization. Each function is explicitly designed for a different use case.

⸻

🧺 bucket(value: String) -> INT

Purpose:
Assigns a value to a numeric bucket using a fast, non-cryptographic hash.

Implementation:
	•	Uses xxh3_64 (fast, deterministic)
	•	Returns a 64-bit integer (INT)

Use Cases:
	•	Sampling: if bucket(user_id) % 10 == 0
	•	Sharding across workers or groups
	•	Grouping logs without revealing identity

Security Note:
❌ Not cryptographically secure.
✅ Suitable for internal grouping only.

⸻

🔢 hash(value: String, algo = "sha256") -> String

Purpose:
Applies a named hash algorithm to the input value and returns a hex-encoded string.

Supported Algorithms:
	•	"sha256" (default)
	•	"sha1"
	•	"md5"
	•	"xxh3" (as hex)
	•	"blake3"

Example:

let h1 = hash("hello");               // sha256 by default
let h2 = hash("value", "md5");

Use Cases:
	•	Fingerprinting values
	•	Explicit hash control
	•	Combining with user-provided salts

Security Note:
❌ Not salted by default — do not use for anonymization unless you prepend your own salt.

⸻

🔒 anonymize(value: String) -> String

Purpose:
Produces a secure, salted, irreversible hex string for anonymizing sensitive data.

Implementation:
	•	Computes sha256(KELORA_SALT + value)
	•	Returns lowercase hex string (64 chars)

Environment Requirement:
	•	Requires KELORA_SALT to be set (env or config)
	•	Fails with a clear error if missing, including a suggestion for a random salt:

        [kelora] error: `KELORA_SALT` is not set — required for `anonymize()` and `pseudonym()`.

        You must set a stable, secret salt to ensure secure and consistent anonymization.

        Suggested (randomized) example:
            export KELORA_SALT="ac47f90dcf6b4d2fa08cfa7b3725e2e3"

Once set, pseudonyms will remain consistent across runs.

Use Cases:
	•	Pseudonymizing user_id, email, ip, session_id
	•	Sharing logs safely without leaking identity
	•	Linkable but irreversible IDs

Security Note:
✅ Salted and cryptographically secure
✅ Suitable for compliance and data privacy

⸻

🪪 pseudonym(value: String, length: INT = 10) -> String

Purpose:
Generates a short, URL-safe, deterministic pseudonym ID using Blake3 and base62 encoding.

Implementation:
	•	Computes blake3(KELORA_SALT + value)
	•	Encodes result to base62
	•	Truncates to length characters

Output:
	•	Base62 string (e.g., "A7cxQZf2Tb")
	•	Length configurable (default: 10)

Environment Requirement:
	•	Requires KELORA_SALT to be set
	•	Fails clearly if not set

Use Cases:
	•	Short anonymous user identifiers
	•	Linking across logs without revealing data
	•	Safer than truncated raw hashes

Security Note:
✅ Salted and secure
✅ Optimized for brevity and readability
⚠️ Truncation reduces collision resistance — tune length accordingly

⸻

🧠 Summary Table

Function	Output	Secure	Salted	Use For…
bucket()	INT (u64)	❌	❌	Bucketing, sampling, grouping
hash()	String (hex)	✅/❌	❌	Explicit hashing, fingerprinting
anonymize()	String (hex)	✅	✅	PII anonymization, linkable IDs
pseudonym()	String (base62)	✅	✅	Short, readable pseudonyms


⸻

🔐 Salt Handling
	•	anonymize() and pseudonym() require a secret salt
	•	Set it via environment variable KELORA_SALT="a3f02c9e7b9d..."
	    or via a command line option (which can be put into the config file)


