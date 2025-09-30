use rhai::Engine;
use sha2::{Digest, Sha256};
use xxhash_rust::xxh3::xxh3_64;

/// Fast non-cryptographic hash for bucketing/sampling
/// Uses xxh3_64 for performance
fn bucket_impl(value: &str) -> i64 {
    xxh3_64(value.as_bytes()) as i64
}

/// Apply a named hash algorithm to input
/// Supported: "sha256" (default), "sha1", "md5", "xxh3", "blake3"
fn hash_impl(value: &str, algo: &str) -> Result<String, Box<rhai::EvalAltResult>> {
    let algo_lower = algo.to_lowercase();
    match algo_lower.as_str() {
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(value.as_bytes());
            Ok(hex::encode(hasher.finalize()))
        }
        "sha1" => {
            let mut hasher = sha1::Sha1::new();
            hasher.update(value.as_bytes());
            Ok(hex::encode(hasher.finalize()))
        }
        "md5" => {
            let mut hasher = md5::Md5::new();
            hasher.update(value.as_bytes());
            Ok(hex::encode(hasher.finalize()))
        }
        "xxh3" => {
            let hash = xxh3_64(value.as_bytes());
            Ok(format!("{:016x}", hash))
        }
        "blake3" => {
            let hash = blake3::hash(value.as_bytes());
            Ok(hash.to_hex().to_string())
        }
        _ => Err(format!(
            "Unknown hash algorithm '{}'. Supported: sha256, sha1, md5, xxh3, blake3",
            algo
        )
        .into()),
    }
}

/// Wrapper for hash with default algorithm
fn hash_default_impl(value: &str) -> Result<String, Box<rhai::EvalAltResult>> {
    hash_impl(value, "sha256")
}

/// Generate random hex salt for error messages
fn generate_random_salt() -> String {
    use fastrand;
    let mut bytes = [0u8; 16];
    for byte in &mut bytes {
        *byte = fastrand::u8(..);
    }
    hex::encode(bytes)
}

/// Secure, salted anonymization using SHA-256
/// Requires KELORA_SALT to be set
fn anonymize_impl(value: &str, salt: &str) -> Result<String, Box<rhai::EvalAltResult>> {
    if salt.is_empty() {
        let suggested_salt = generate_random_salt();
        return Err(format!(
            "`KELORA_SALT` is not set — required for `anonymize()` and `pseudonym()`.\n\
            \n\
            You must set a stable, secret salt to ensure secure and consistent anonymization.\n\
            \n\
            Suggested (randomized) example:\n\
                export KELORA_SALT=\"{}\"\n\
            \n\
            Once set, pseudonyms will remain consistent across runs.",
            suggested_salt
        )
        .into());
    }

    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(value.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// Short, URL-safe pseudonym using Blake3 and base62
/// Requires KELORA_SALT to be set
fn pseudonym_impl(
    value: &str,
    length: i64,
    salt: &str,
) -> Result<String, Box<rhai::EvalAltResult>> {
    if salt.is_empty() {
        let suggested_salt = generate_random_salt();
        return Err(format!(
            "`KELORA_SALT` is not set — required for `anonymize()` and `pseudonym()`.\n\
            \n\
            You must set a stable, secret salt to ensure secure and consistent anonymization.\n\
            \n\
            Suggested (randomized) example:\n\
                export KELORA_SALT=\"{}\"\n\
            \n\
            Once set, pseudonyms will remain consistent across runs.",
            suggested_salt
        )
        .into());
    }

    if length <= 0 {
        return Err("pseudonym() length must be positive".into());
    }

    // Hash with Blake3 (salted)
    let mut hasher = blake3::Hasher::new();
    hasher.update(salt.as_bytes());
    hasher.update(value.as_bytes());
    let hash = hasher.finalize();

    // Encode to base62 using hex representation as input
    let hex_str = hash.to_hex().to_string();
    let base62_str = base62::encode(u128::from_str_radix(&hex_str[..32], 16).unwrap_or(0));

    // Truncate to requested length
    let len = length as usize;
    if base62_str.len() >= len {
        Ok(base62_str[..len].to_string())
    } else {
        Ok(base62_str)
    }
}

/// Wrapper for pseudonym with default length
fn pseudonym_default_impl(value: &str, salt: &str) -> Result<String, Box<rhai::EvalAltResult>> {
    pseudonym_impl(value, 10, salt)
}

/// Register hashing functions with the Rhai engine
/// Salt parameter comes from ProcessingConfig (CLI --salt or KELORA_SALT env var)
pub fn register_functions(engine: &mut Engine, salt: Option<String>) {
    let salt_str = salt.unwrap_or_default();

    // bucket() - fast non-cryptographic hash for bucketing/sampling
    engine.register_fn("bucket", bucket_impl);

    // hash() - multi-algorithm hashing
    engine.register_fn("hash", hash_default_impl);
    engine.register_fn("hash", hash_impl);

    // anonymize() - salted SHA-256
    let salt_for_anonymize = salt_str.clone();
    engine.register_fn("anonymize", move |value: &str| {
        anonymize_impl(value, &salt_for_anonymize)
    });

    // pseudonym() - salted Blake3 + base62
    let salt_for_pseudonym = salt_str.clone();
    engine.register_fn("pseudonym", move |value: &str| {
        pseudonym_default_impl(value, &salt_for_pseudonym)
    });

    let salt_for_pseudonym_len = salt_str;
    engine.register_fn("pseudonym", move |value: &str, length: i64| {
        pseudonym_impl(value, length, &salt_for_pseudonym_len)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket() {
        let result1 = bucket_impl("test");
        let result2 = bucket_impl("test");
        let result3 = bucket_impl("other");

        // Same input should produce same hash
        assert_eq!(result1, result2);
        // Different input should (probably) produce different hash
        assert_ne!(result1, result3);
    }

    #[test]
    fn test_hash_sha256() {
        let result = hash_impl("hello", "sha256").unwrap();
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_hash_sha1() {
        let result = hash_impl("hello", "sha1").unwrap();
        assert_eq!(result, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_hash_md5() {
        let result = hash_impl("hello", "md5").unwrap();
        assert_eq!(result, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_hash_xxh3() {
        let result = hash_impl("hello", "xxh3").unwrap();
        // xxh3 is deterministic, just verify it's a valid hex string
        assert_eq!(result.len(), 16);
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_blake3() {
        let result = hash_impl("hello", "blake3").unwrap();
        assert_eq!(
            result,
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f"
        );
    }

    #[test]
    fn test_hash_unknown_algo() {
        let result = hash_impl("hello", "unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_default() {
        let result = hash_default_impl("hello").unwrap();
        // Should default to sha256
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_anonymize_with_salt() {
        let salt = "test_salt";
        let result1 = anonymize_impl("user123", salt).unwrap();
        let result2 = anonymize_impl("user123", salt).unwrap();
        let result3 = anonymize_impl("user456", salt).unwrap();

        // Same input with same salt should be deterministic
        assert_eq!(result1, result2);
        // Different input should produce different hash
        assert_ne!(result1, result3);
        // Should be a valid SHA-256 hex string (64 chars)
        assert_eq!(result1.len(), 64);
        assert!(result1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_anonymize_without_salt() {
        let result = anonymize_impl("user123", "");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("KELORA_SALT"));
        assert!(err_msg.contains("export KELORA_SALT="));
    }

    #[test]
    fn test_pseudonym_with_salt() {
        let salt = "test_salt";
        let result1 = pseudonym_impl("user123", 10, salt).unwrap();
        let result2 = pseudonym_impl("user123", 10, salt).unwrap();
        let result3 = pseudonym_impl("user456", 10, salt).unwrap();

        // Same input with same salt should be deterministic
        assert_eq!(result1, result2);
        // Different input should produce different pseudonym
        assert_ne!(result1, result3);
        // Should be the requested length
        assert_eq!(result1.len(), 10);
        // Should only contain base62 characters
        assert!(result1
            .chars()
            .all(|c| c.is_ascii_alphanumeric() && c != 'O' && c != 'I'));
    }

    #[test]
    fn test_pseudonym_default_length() {
        let salt = "test_salt";
        let result = pseudonym_default_impl("user123", salt).unwrap();
        assert_eq!(result.len(), 10); // Default length
    }

    #[test]
    fn test_pseudonym_different_lengths() {
        let salt = "test_salt";
        let result5 = pseudonym_impl("user123", 5, salt).unwrap();
        let result20 = pseudonym_impl("user123", 20, salt).unwrap();

        assert_eq!(result5.len(), 5);
        assert_eq!(result20.len(), 20);
        // Shorter should be prefix of longer
        assert!(result20.starts_with(&result5));
    }

    #[test]
    fn test_pseudonym_without_salt() {
        let result = pseudonym_impl("user123", 10, "");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("KELORA_SALT"));
    }

    #[test]
    fn test_pseudonym_invalid_length() {
        let salt = "test_salt";
        let result = pseudonym_impl("user123", 0, salt);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("positive"));
    }

    #[test]
    fn test_different_salts_produce_different_results() {
        let result1 = anonymize_impl("user123", "salt1").unwrap();
        let result2 = anonymize_impl("user123", "salt2").unwrap();
        assert_ne!(result1, result2);

        let result3 = pseudonym_impl("user123", 10, "salt1").unwrap();
        let result4 = pseudonym_impl("user123", 10, "salt2").unwrap();
        assert_ne!(result3, result4);
    }

    #[test]
    fn test_rhai_integration() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine, Some("test_salt".to_string()));

        // Test bucket
        let result: i64 = engine.eval(r#"bucket("test")"#).unwrap();
        assert_eq!(result, bucket_impl("test"));

        // Test hash with default
        let result: String = engine.eval(r#"hash("hello")"#).unwrap();
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );

        // Test hash with algo
        let result: String = engine.eval(r#"hash("hello", "md5")"#).unwrap();
        assert_eq!(result, "5d41402abc4b2a76b9719d911017c592");

        // Test anonymize
        let result: String = engine.eval(r#"anonymize("user123")"#).unwrap();
        assert_eq!(result.len(), 64);

        // Test pseudonym with default length
        let result: String = engine.eval(r#"pseudonym("user123")"#).unwrap();
        assert_eq!(result.len(), 10);

        // Test pseudonym with custom length
        let result: String = engine.eval(r#"pseudonym("user123", 15)"#).unwrap();
        assert_eq!(result.len(), 15);
    }

    #[test]
    fn test_rhai_without_salt() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine, None);

        // bucket and hash should work without salt
        let _: i64 = engine.eval(r#"bucket("test")"#).unwrap();
        let _: String = engine.eval(r#"hash("test")"#).unwrap();

        // anonymize should fail without salt
        let result = engine.eval::<String>(r#"anonymize("user123")"#);
        assert!(result.is_err());

        // pseudonym should fail without salt
        let result = engine.eval::<String>(r#"pseudonym("user123")"#);
        assert!(result.is_err());
    }
}
