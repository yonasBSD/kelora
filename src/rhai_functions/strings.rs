use rhai::{Dynamic, Engine};
use std::cell::RefCell;

thread_local! {
    static CAPTURED_PRINTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static CAPTURED_EPRINTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static PARALLEL_MODE: RefCell<bool> = const { RefCell::new(false) };
    static SUPPRESS_SIDE_EFFECTS: RefCell<bool> = const { RefCell::new(false) };
}

/// Capture a print statement in thread-local storage for parallel processing
pub fn capture_print(message: String) {
    CAPTURED_PRINTS.with(|prints| {
        prints.borrow_mut().push(message);
    });
}

/// Capture an eprint statement in thread-local storage for parallel processing
pub fn capture_eprint(message: String) {
    CAPTURED_EPRINTS.with(|eprints| {
        eprints.borrow_mut().push(message);
    });
}

/// Get all captured prints and clear the buffer
pub fn take_captured_prints() -> Vec<String> {
    CAPTURED_PRINTS.with(|prints| std::mem::take(&mut *prints.borrow_mut()))
}

/// Get all captured eprints and clear the buffer
pub fn take_captured_eprints() -> Vec<String> {
    CAPTURED_EPRINTS.with(|eprints| std::mem::take(&mut *eprints.borrow_mut()))
}

/// Clear captured prints without returning them
pub fn clear_captured_prints() {
    CAPTURED_PRINTS.with(|prints| {
        prints.borrow_mut().clear();
    });
}

/// Clear captured eprints without returning them
pub fn clear_captured_eprints() {
    CAPTURED_EPRINTS.with(|eprints| {
        eprints.borrow_mut().clear();
    });
}

/// Set whether we're in parallel processing mode
pub fn set_parallel_mode(enabled: bool) {
    PARALLEL_MODE.with(|mode| {
        *mode.borrow_mut() = enabled;
    });
}

/// Check if we're in parallel processing mode
pub fn is_parallel_mode() -> bool {
    PARALLEL_MODE.with(|mode| *mode.borrow())
}

/// Set whether to suppress side effects (print, eprint, etc.)
pub fn set_suppress_side_effects(suppress: bool) {
    SUPPRESS_SIDE_EFFECTS.with(|flag| {
        *flag.borrow_mut() = suppress;
    });
}

/// Check if side effects should be suppressed
pub fn is_suppress_side_effects() -> bool {
    SUPPRESS_SIDE_EFFECTS.with(|flag| *flag.borrow())
}

/// Mask IP address for privacy (replace last N octets with 'X')
fn mask_ip_impl(ip: &str, octets_to_mask: usize) -> String {
    // IPv4 pattern validation
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return ip.to_string(); // Return unchanged if not valid IPv4
    }

    // Validate each octet is numeric
    for part in &parts {
        if part.parse::<u8>().is_err() {
            return ip.to_string(); // Return unchanged if not numeric
        }
    }

    let mut result = parts.clone();
    let mask_count = octets_to_mask.clamp(1, 4);

    // Replace last N octets with 'X'
    for item in result.iter_mut().skip(4 - mask_count) {
        *item = "X";
    }

    result.join(".")
}

/// Check if IP address is in private range
fn is_private_ip_impl(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false; // Not valid IPv4
    }

    // Parse octets
    let octets: Result<Vec<u8>, _> = parts.iter().map(|s| s.parse::<u8>()).collect();
    let octets = match octets {
        Ok(o) => o,
        Err(_) => return false,
    };

    // Check private ranges
    match octets[0] {
        10 => true,                                // 10.0.0.0/8
        172 => octets[1] >= 16 && octets[1] <= 31, // 172.16.0.0/12
        192 => octets[1] == 168,                   // 192.168.0.0/16
        127 => true,                               // 127.0.0.0/8 (loopback)
        _ => false,
    }
}

/// Parse key-value pairs from a string (like logfmt format)
///
/// # Arguments
/// * `text` - The input string to parse
/// * `sep` - Optional separator between key-value pairs (default: whitespace)
/// * `kv_sep` - Separator between key and value (default: "=")
///
/// # Returns
/// A Rhai Map containing the parsed key-value pairs
fn parse_kv_impl(text: &str, sep: Option<&str>, kv_sep: &str) -> rhai::Map {
    let mut map = rhai::Map::new();

    // Split by separator or whitespace
    let pairs: Vec<&str> = if let Some(separator) = sep {
        text.split(separator).collect()
    } else {
        // Split by any whitespace
        text.split_whitespace().collect()
    };

    for pair in pairs {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }

        // Find the key-value separator
        if let Some(kv_pos) = pair.find(kv_sep) {
            let key = pair[..kv_pos].trim();
            let value = pair[kv_pos + kv_sep.len()..].trim();

            if !key.is_empty() {
                map.insert(key.into(), rhai::Dynamic::from(value.to_string()));
            }
        }
        // If no separator found, treat as key with empty value
        else if !pair.is_empty() {
            map.insert(pair.into(), rhai::Dynamic::from(String::new()));
        }
    }

    map
}

pub fn register_functions(engine: &mut Engine) {
    // Note: print() function is now handled via engine.on_print() in engine.rs

    // Custom eprint function that captures output in parallel mode and respects suppression
    engine.register_fn("eprint", |message: rhai::Dynamic| {
        if is_suppress_side_effects() {
            // Suppress all eprint output
            return;
        }

        let msg = message.to_string();
        if is_parallel_mode() {
            capture_eprint(msg);
        } else {
            eprintln!("{}", msg);
        }
    });

    // Existing string functions from engine.rs
    engine.register_fn("contains", |text: &str, pattern: &str| {
        text.contains(pattern)
    });

    engine.register_fn("matches", |text: &str, pattern: &str| {
        regex::Regex::new(pattern)
            .map(|re| re.is_match(text))
            .unwrap_or(false)
    });

    engine.register_fn("to_int", |text: &str| -> rhai::Dynamic {
        text.parse::<i64>()
            .map(Dynamic::from)
            .unwrap_or(Dynamic::from(0i64))
    });

    engine.register_fn("to_float", |text: &str| -> rhai::Dynamic {
        text.parse::<f64>()
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT)
    });

    engine.register_fn("slice", |s: &str, spec: &str| -> String {
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len() as i32;

        if len == 0 {
            return String::new();
        }

        let parts: Vec<&str> = spec.split(':').collect();

        // Parse step first
        let step = if parts.len() > 2 && !parts[2].trim().is_empty() {
            parts[2].trim().parse::<i32>().unwrap_or(1)
        } else {
            1
        };

        if step == 0 {
            return String::new();
        }

        // Determine defaults based on step direction
        let (default_start, default_end) = if step > 0 { (0, len) } else { (len - 1, -1) };

        // Parse start
        let start = if !parts.is_empty() && !parts[0].trim().is_empty() {
            let mut s = parts[0].trim().parse::<i32>().unwrap_or(default_start);
            if s < 0 {
                s += len;
            }
            if step > 0 {
                s.clamp(0, len)
            } else {
                s.clamp(0, len - 1)
            }
        } else {
            default_start
        };

        // Parse end
        let end = if parts.len() > 1 && !parts[1].trim().is_empty() {
            let mut e = parts[1].trim().parse::<i32>().unwrap_or(default_end);
            if e < 0 {
                e += len;
            }
            if step > 0 {
                e.clamp(0, len)
            } else {
                e.clamp(-1, len - 1)
            }
        } else {
            default_end
        };

        let mut result = String::new();
        let mut i = start;

        if step > 0 {
            while i < end {
                if i >= 0 && i < len {
                    result.push(chars[i as usize]);
                }
                i += step;
            }
        } else {
            while i > end {
                if i >= 0 && i < len {
                    result.push(chars[i as usize]);
                }
                i += step;
            }
        }

        result
    });

    // String processing functions (literal string matching, not regex)
    engine.register_fn("after", |text: &str, substring: &str| -> String {
        if let Some(pos) = text.find(substring) {
            text[pos + substring.len()..].to_string()
        } else {
            String::new()
        }
    });

    engine.register_fn("before", |text: &str, substring: &str| -> String {
        if let Some(pos) = text.find(substring) {
            text[..pos].to_string()
        } else {
            String::new()
        }
    });

    engine.register_fn(
        "between",
        |text: &str, start_substring: &str, end_substring: &str| -> String {
            if let Some(start_pos) = text.find(start_substring) {
                let start_idx = start_pos + start_substring.len();
                let remainder = &text[start_idx..];

                if end_substring.is_empty() {
                    // Empty end substring means "to end of string"
                    remainder.to_string()
                } else if let Some(end_pos) = remainder.find(end_substring) {
                    remainder[..end_pos].to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        },
    );

    engine.register_fn("starting_with", |text: &str, prefix: &str| -> String {
        if text.starts_with(prefix) {
            text.to_string()
        } else {
            String::new()
        }
    });

    engine.register_fn("ending_with", |text: &str, suffix: &str| -> String {
        if text.ends_with(suffix) {
            text.to_string()
        } else {
            String::new()
        }
    });

    // Parse key-value pairs from a string (like logfmt)
    engine.register_fn("parse_kv", |text: &str| -> rhai::Map {
        parse_kv_impl(text, None, "=")
    });

    engine.register_fn("parse_kv", |text: &str, sep: &str| -> rhai::Map {
        parse_kv_impl(text, Some(sep), "=")
    });

    engine.register_fn(
        "parse_kv",
        |text: &str, sep: &str, kv_sep: &str| -> rhai::Map {
            parse_kv_impl(text, Some(sep), kv_sep)
        },
    );

    // Allow unit type for null separator
    engine.register_fn(
        "parse_kv",
        |text: &str, _sep: (), kv_sep: &str| -> rhai::Map { parse_kv_impl(text, None, kv_sep) },
    );

    // String case conversion functions
    engine.register_fn("lower", |text: &str| -> String { text.to_lowercase() });

    engine.register_fn("upper", |text: &str| -> String { text.to_uppercase() });

    // Python-style string methods
    engine.register_fn("is_digit", |text: &str| -> bool {
        !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
    });

    engine.register_fn("count", |text: &str, pattern: &str| -> i64 {
        if pattern.is_empty() {
            return 0;
        }
        text.matches(pattern).count() as i64
    });

    engine.register_fn("strip", |text: &str| -> String { text.trim().to_string() });

    engine.register_fn("strip", |text: &str, chars: &str| -> String {
        let chars_to_remove: std::collections::HashSet<char> = chars.chars().collect();
        text.trim_matches(|c: char| chars_to_remove.contains(&c))
            .to_string()
    });

    engine.register_fn("join", |separator: &str, items: rhai::Array| -> String {
        items
            .into_iter()
            .filter_map(|item| item.into_string().ok())
            .collect::<Vec<String>>()
            .join(separator)
    });

    // Regex string methods
    engine.register_fn("extract_re", |text: &str, pattern: &str| -> String {
        match regex::Regex::new(pattern) {
            Ok(re) => {
                if let Some(captures) = re.captures(text) {
                    // Return first captured group, or whole match if no groups
                    if captures.len() > 1 {
                        captures
                            .get(1)
                            .map(|m| m.as_str())
                            .unwrap_or("")
                            .to_string()
                    } else {
                        captures
                            .get(0)
                            .map(|m| m.as_str())
                            .unwrap_or("")
                            .to_string()
                    }
                } else {
                    String::new()
                }
            }
            Err(_) => String::new(), // Invalid regex returns empty string
        }
    });

    engine.register_fn(
        "extract_re",
        |text: &str, pattern: &str, group: i64| -> String {
            match regex::Regex::new(pattern) {
                Ok(re) => {
                    if let Some(captures) = re.captures(text) {
                        let group_idx = if group < 0 {
                            // Negative indices not supported, default to 0
                            0
                        } else {
                            group as usize
                        };
                        captures
                            .get(group_idx)
                            .map(|m| m.as_str())
                            .unwrap_or("")
                            .to_string()
                    } else {
                        String::new()
                    }
                }
                Err(_) => String::new(), // Invalid regex returns empty string
            }
        },
    );

    engine.register_fn(
        "extract_all_re",
        |text: &str, pattern: &str| -> rhai::Array {
            match regex::Regex::new(pattern) {
                Ok(re) => {
                    let mut results = rhai::Array::new();
                    for captures in re.captures_iter(text) {
                        if captures.len() > 1 {
                            // Multiple capture groups - return array of groups
                            let groups: rhai::Array = captures
                                .iter()
                                .skip(1) // Skip full match (index 0)
                                .filter_map(|m| {
                                    m.map(|match_| Dynamic::from(match_.as_str().to_string()))
                                })
                                .collect();
                            results.push(Dynamic::from(groups));
                        } else {
                            // No capture groups - return the full match
                            if let Some(full_match) = captures.get(0) {
                                results.push(Dynamic::from(full_match.as_str().to_string()));
                            }
                        }
                    }
                    results
                }
                Err(_) => rhai::Array::new(), // Invalid regex returns empty array
            }
        },
    );

    engine.register_fn(
        "extract_all_re",
        |text: &str, pattern: &str, group: i64| -> rhai::Array {
            match regex::Regex::new(pattern) {
                Ok(re) => {
                    let mut results = rhai::Array::new();
                    let group_idx = if group < 0 {
                        // Negative indices not supported, default to 0
                        0
                    } else {
                        group as usize
                    };

                    for captures in re.captures_iter(text) {
                        if let Some(group_match) = captures.get(group_idx) {
                            results.push(Dynamic::from(group_match.as_str().to_string()));
                        }
                    }
                    results
                }
                Err(_) => rhai::Array::new(), // Invalid regex returns empty array
            }
        },
    );

    engine.register_fn("split_re", |text: &str, pattern: &str| -> rhai::Array {
        match regex::Regex::new(pattern) {
            Ok(re) => re
                .split(text)
                .map(|s| Dynamic::from(s.to_string()))
                .collect(),
            Err(_) => vec![Dynamic::from(text.to_string())], // Invalid regex returns original string
        }
    });

    engine.register_fn(
        "replace_re",
        |text: &str, pattern: &str, replacement: &str| -> String {
            match regex::Regex::new(pattern) {
                Ok(re) => re.replace_all(text, replacement).to_string(),
                Err(_) => text.to_string(), // Invalid regex returns original string
            }
        },
    );

    // Network/IP methods
    engine.register_fn("extract_ip", |text: &str| -> String {
        let ip_pattern = r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b";
        match regex::Regex::new(ip_pattern) {
            Ok(re) => {
                re.find(text)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(String::new)
            }
            Err(_) => String::new(),
        }
    });

    engine.register_fn("extract_ips", |text: &str| -> rhai::Array {
        let ip_pattern = r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b";
        match regex::Regex::new(ip_pattern) {
            Ok(re) => re
                .find_iter(text)
                .map(|m| Dynamic::from(m.as_str().to_string()))
                .collect(),
            Err(_) => rhai::Array::new(),
        }
    });

    engine.register_fn("mask_ip", |ip: &str| -> String {
        mask_ip_impl(ip, 1) // Default: mask last octet
    });

    engine.register_fn("mask_ip", |ip: &str, octets: i64| -> String {
        mask_ip_impl(ip, octets.clamp(1, 4) as usize) // Clamp between 1-4
    });

    engine.register_fn("is_private_ip", |ip: &str| -> bool {
        is_private_ip_impl(ip)
    });

    engine.register_fn("extract_url", |text: &str| -> String {
        let url_pattern = r##"https?://[^\s<>"]+[^\s<>".,;!?]"##;
        match regex::Regex::new(url_pattern) {
            Ok(re) => re
                .find(text)
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(String::new),
            Err(_) => String::new(),
        }
    });

    engine.register_fn("extract_domain", |text: &str| -> String {
        // Try URL first, then email domain
        let url_pattern = r##"https?://([^/\s<>"]+)"##;
        let email_pattern = r##"[a-zA-Z0-9._%+-]+@([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})"##;

        if let Ok(re) = regex::Regex::new(url_pattern) {
            if let Some(caps) = re.captures(text) {
                if let Some(domain) = caps.get(1) {
                    return domain.as_str().to_string();
                }
            }
        }

        if let Ok(re) = regex::Regex::new(email_pattern) {
            if let Some(caps) = re.captures(text) {
                if let Some(domain) = caps.get(1) {
                    return domain.as_str().to_string();
                }
            }
        }

        String::new()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::Scope;

    #[test]
    fn test_after_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "hello world test");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.after("world")"#)
            .unwrap();
        assert_eq!(result, " test");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.after("missing")"#)
            .unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_before_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "hello world test");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.before("world")"#)
            .unwrap();
        assert_eq!(result, "hello ");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.before("missing")"#)
            .unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_between_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "start[content]end");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.between("[", "]")"#)
            .unwrap();
        assert_eq!(result, "content");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.between("missing", "]")"#)
            .unwrap();
        assert_eq!(result, "");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.between("[", "missing")"#)
            .unwrap();
        assert_eq!(result, "");

        // Test empty end substring - should return everything after start
        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.between("[", "")"#)
            .unwrap();
        assert_eq!(result, "content]end");

        scope.push("log", "ERROR: connection failed");
        let result: String = engine
            .eval_with_scope(&mut scope, r#"log.between("ERROR: ", "")"#)
            .unwrap();
        assert_eq!(result, "connection failed");
    }

    #[test]
    fn test_starting_with_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "hello world");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.starting_with("hello")"#)
            .unwrap();
        assert_eq!(result, "hello world");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.starting_with("world")"#)
            .unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_ending_with_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "hello world");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.ending_with("world")"#)
            .unwrap();
        assert_eq!(result, "hello world");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.ending_with("hello")"#)
            .unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_parse_kv_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();

        // Test basic key=value parsing
        scope.push("text", "key1=value1 key2=value2");
        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_kv(text)"#)
            .unwrap();
        assert_eq!(
            result.get("key1").unwrap().clone().into_string().unwrap(),
            "value1"
        );
        assert_eq!(
            result.get("key2").unwrap().clone().into_string().unwrap(),
            "value2"
        );

        // Test with custom separator
        scope.push("text2", "key1=value1,key2=value2");
        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_kv(text2, ",")"#)
            .unwrap();
        assert_eq!(
            result.get("key1").unwrap().clone().into_string().unwrap(),
            "value1"
        );
        assert_eq!(
            result.get("key2").unwrap().clone().into_string().unwrap(),
            "value2"
        );

        // Test with custom key-value separator
        scope.push("text3", "key1:value1 key2:value2");
        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_kv(text3, (), ":")"#)
            .unwrap();
        assert_eq!(
            result.get("key1").unwrap().clone().into_string().unwrap(),
            "value1"
        );
        assert_eq!(
            result.get("key2").unwrap().clone().into_string().unwrap(),
            "value2"
        );

        // Test with quoted values (simple - no space handling inside quotes)
        scope.push("text4", r#"key1="quoted" key2=simple"#);
        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_kv(text4)"#)
            .unwrap();
        assert_eq!(
            result.get("key1").unwrap().clone().into_string().unwrap(),
            "\"quoted\""
        );
        assert_eq!(
            result.get("key2").unwrap().clone().into_string().unwrap(),
            "simple"
        );

        // Test with key without value
        scope.push("text5", "key1=value1 standalone key2=value2");
        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_kv(text5)"#)
            .unwrap();
        assert_eq!(
            result.get("key1").unwrap().clone().into_string().unwrap(),
            "value1"
        );
        assert_eq!(
            result
                .get("standalone")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            ""
        );
        assert_eq!(
            result.get("key2").unwrap().clone().into_string().unwrap(),
            "value2"
        );

        // Test edge cases
        scope.push("empty", "");
        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_kv(empty)"#)
            .unwrap();
        assert!(result.is_empty());

        scope.push("spaces", "  key1=value1   key2=value2  ");
        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_kv(spaces)"#)
            .unwrap();
        assert_eq!(
            result.get("key1").unwrap().clone().into_string().unwrap(),
            "value1"
        );
        assert_eq!(
            result.get("key2").unwrap().clone().into_string().unwrap(),
            "value2"
        );

        // Test with empty values
        scope.push("empty_vals", "key1= key2=value2");
        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_kv(empty_vals)"#)
            .unwrap();
        assert_eq!(
            result.get("key1").unwrap().clone().into_string().unwrap(),
            ""
        );
        assert_eq!(
            result.get("key2").unwrap().clone().into_string().unwrap(),
            "value2"
        );
    }

    #[test]
    fn test_lower_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "Hello World");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.lower()"#)
            .unwrap();
        assert_eq!(result, "hello world");

        scope.push("mixed", "MiXeD cAsE");
        let result: String = engine
            .eval_with_scope(&mut scope, r#"mixed.lower()"#)
            .unwrap();
        assert_eq!(result, "mixed case");
    }

    #[test]
    fn test_upper_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "Hello World");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.upper()"#)
            .unwrap();
        assert_eq!(result, "HELLO WORLD");

        scope.push("mixed", "MiXeD cAsE");
        let result: String = engine
            .eval_with_scope(&mut scope, r#"mixed.upper()"#)
            .unwrap();
        assert_eq!(result, "MIXED CASE");
    }

    #[test]
    fn test_is_digit_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("digits", "12345");
        scope.push("mixed", "123abc");
        scope.push("empty", "");
        scope.push("letters", "abcde");

        let result: bool = engine
            .eval_with_scope(&mut scope, r#"digits.is_digit()"#)
            .unwrap();
        assert_eq!(result, true);

        let result: bool = engine
            .eval_with_scope(&mut scope, r#"mixed.is_digit()"#)
            .unwrap();
        assert_eq!(result, false);

        let result: bool = engine
            .eval_with_scope(&mut scope, r#"empty.is_digit()"#)
            .unwrap();
        assert_eq!(result, false);

        let result: bool = engine
            .eval_with_scope(&mut scope, r#"letters.is_digit()"#)
            .unwrap();
        assert_eq!(result, false);
    }

    #[test]
    fn test_count_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "hello world hello");
        scope.push("empty", "");

        let result: i64 = engine
            .eval_with_scope(&mut scope, r#"text.count("hello")"#)
            .unwrap();
        assert_eq!(result, 2);

        let result: i64 = engine
            .eval_with_scope(&mut scope, r#"text.count("l")"#)
            .unwrap();
        assert_eq!(result, 5);

        let result: i64 = engine
            .eval_with_scope(&mut scope, r#"text.count("missing")"#)
            .unwrap();
        assert_eq!(result, 0);

        let result: i64 = engine
            .eval_with_scope(&mut scope, r#"empty.count("x")"#)
            .unwrap();
        assert_eq!(result, 0);

        let result: i64 = engine
            .eval_with_scope(&mut scope, r#"text.count("")"#)
            .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_strip_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "  hello world  ");
        scope.push("custom", "###hello world###");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"text.strip()"#)
            .unwrap();
        assert_eq!(result, "hello world");

        let result: String = engine
            .eval_with_scope(&mut scope, r##"custom.strip("#")"##)
            .unwrap();
        assert_eq!(result, "hello world");

        scope.push("mixed", "  ##hello world##  ");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"mixed.strip(" #")"##)
            .unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_join_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();

        let result: String = engine
            .eval_with_scope(&mut scope, r#"",".join(["a", "b", "c"])"#)
            .unwrap();
        assert_eq!(result, "a,b,c");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"" ".join(["hello", "world"])"#)
            .unwrap();
        assert_eq!(result, "hello world");

        let result: String = engine
            .eval_with_scope(&mut scope, r#""-".join(["one"])"#)
            .unwrap();
        assert_eq!(result, "one");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"",".join([])"#)
            .unwrap();
        assert_eq!(result, "");

        // Test with mixed types (non-strings filtered out)
        let result: String = engine
            .eval_with_scope(&mut scope, r#"",".join(["a", 123, "b"])"#)
            .unwrap();
        assert_eq!(result, "a,b");
    }

    #[test]
    fn test_extract_re_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "user=alice status=200");

        // Extract with capture group
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_re("user=(\\w+)")"##)
            .unwrap();
        assert_eq!(result, "alice");

        // Extract without capture group (returns full match)
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_re("\\d+")"##)
            .unwrap();
        assert_eq!(result, "200");

        // No match
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_re("missing")"##)
            .unwrap();
        assert_eq!(result, "");

        // Invalid regex
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_re("[")"##)
            .unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_extract_re_with_group_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "user=alice status=200 level=info");

        // Extract specific groups from complex pattern
        let result: String = engine
            .eval_with_scope(
                &mut scope,
                r##"text.extract_re("user=(\\w+).*status=(\\d+)", 0)"##,
            )
            .unwrap();
        assert_eq!(result, "user=alice status=200"); // Full match (group 0)

        let result: String = engine
            .eval_with_scope(
                &mut scope,
                r##"text.extract_re("user=(\\w+).*status=(\\d+)", 1)"##,
            )
            .unwrap();
        assert_eq!(result, "alice"); // First capture group

        let result: String = engine
            .eval_with_scope(
                &mut scope,
                r##"text.extract_re("user=(\\w+).*status=(\\d+)", 2)"##,
            )
            .unwrap();
        assert_eq!(result, "200"); // Second capture group

        // Out of bounds group (returns empty)
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_re("user=(\\w+)", 5)"##)
            .unwrap();
        assert_eq!(result, "");

        // Negative group index (defaults to 0)
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_re("user=(\\w+)", -1)"##)
            .unwrap();
        assert_eq!(result, "user=alice");
    }

    #[test]
    fn test_extract_all_re_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "a=1 b=2 c=3");

        // Extract all with capture groups
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"text.extract_all_re("(\\w+)=(\\d+)")"##)
            .unwrap();
        assert_eq!(result.len(), 3);

        // Check first match groups
        let first_match = result[0].clone().into_array().unwrap();
        assert_eq!(first_match[0].clone().into_string().unwrap(), "a");
        assert_eq!(first_match[1].clone().into_string().unwrap(), "1");

        // Extract all without capture groups (just matches)
        scope.push("numbers", "10 20 30 40");
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"numbers.extract_all_re("\\d+")"##)
            .unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].clone().into_string().unwrap(), "10");
        assert_eq!(result[3].clone().into_string().unwrap(), "40");

        // No matches
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"text.extract_all_re("missing")"##)
            .unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_extract_all_re_with_group_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "text",
            "user=alice status=200 user=bob status=404 user=charlie status=500",
        );

        // Extract all values from first capture group (usernames)
        let result: rhai::Array = engine
            .eval_with_scope(
                &mut scope,
                r##"text.extract_all_re("user=(\\w+).*?status=(\\d+)", 1)"##,
            )
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].clone().into_string().unwrap(), "alice");
        assert_eq!(result[1].clone().into_string().unwrap(), "bob");
        assert_eq!(result[2].clone().into_string().unwrap(), "charlie");

        // Extract all values from second capture group (status codes)
        let result: rhai::Array = engine
            .eval_with_scope(
                &mut scope,
                r##"text.extract_all_re("user=(\\w+).*?status=(\\d+)", 2)"##,
            )
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].clone().into_string().unwrap(), "200");
        assert_eq!(result[1].clone().into_string().unwrap(), "404");
        assert_eq!(result[2].clone().into_string().unwrap(), "500");

        // Extract all full matches (group 0)
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"text.extract_all_re("user=(\\w+)", 0)"##)
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].clone().into_string().unwrap(), "user=alice");
        assert_eq!(result[1].clone().into_string().unwrap(), "user=bob");
        assert_eq!(result[2].clone().into_string().unwrap(), "user=charlie");

        // Out of bounds group (returns empty array)
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"text.extract_all_re("user=(\\w+)", 5)"##)
            .unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_split_re_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "one,two;three:four");

        // Split by multiple delimiters
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"text.split_re("[,;:]")"##)
            .unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].clone().into_string().unwrap(), "one");
        assert_eq!(result[1].clone().into_string().unwrap(), "two");
        assert_eq!(result[2].clone().into_string().unwrap(), "three");
        assert_eq!(result[3].clone().into_string().unwrap(), "four");

        // Split by whitespace
        scope.push("spaced", "hello    world\ttab\nnewline");
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"spaced.split_re("\\s+")"##)
            .unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].clone().into_string().unwrap(), "hello");
        assert_eq!(result[1].clone().into_string().unwrap(), "world");

        // Invalid regex (returns original string)
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"text.split_re("[")"##)
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].clone().into_string().unwrap(),
            "one,two;three:four"
        );
    }

    #[test]
    fn test_replace_re_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "The year 2023 and 2024 are here");

        // Replace all years with "YEAR"
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.replace_re("\\d{4}", "YEAR")"##)
            .unwrap();
        assert_eq!(result, "The year YEAR and YEAR are here");

        // Replace with capture groups
        scope.push("emails", "Contact alice@example.com or bob@test.org");
        let result: String = engine
            .eval_with_scope(
                &mut scope,
                r##"emails.replace_re("(\\w+)@(\\w+\\.\\w+)", "[$1 at $2]")"##,
            )
            .unwrap();
        assert_eq!(
            result,
            "Contact [alice at example.com] or [bob at test.org]"
        );

        // No matches (returns original)
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.replace_re("nomatch", "replacement")"##)
            .unwrap();
        assert_eq!(result, "The year 2023 and 2024 are here");

        // Invalid regex (returns original)
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.replace_re("[", "replacement")"##)
            .unwrap();
        assert_eq!(result, "The year 2023 and 2024 are here");
    }

    #[test]
    fn test_extract_ip_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "Server 192.168.1.100 responded");

        // Extract single IP
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_ip()"##)
            .unwrap();
        assert_eq!(result, "192.168.1.100");

        // No IP found
        scope.push("no_ip", "No IP address here");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"no_ip.extract_ip()"##)
            .unwrap();
        assert_eq!(result, "");

        // Multiple IPs, returns first
        scope.push("multi", "From 10.0.0.1 to 172.16.0.1");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"multi.extract_ip()"##)
            .unwrap();
        assert_eq!(result, "10.0.0.1");
    }

    #[test]
    fn test_extract_ips_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "From 10.0.0.1 to 172.16.0.1 via 192.168.1.1");

        // Extract all IPs
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"text.extract_ips()"##)
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].clone().into_string().unwrap(), "10.0.0.1");
        assert_eq!(result[1].clone().into_string().unwrap(), "172.16.0.1");
        assert_eq!(result[2].clone().into_string().unwrap(), "192.168.1.1");

        // No IPs found
        scope.push("no_ips", "No IP addresses here");
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"no_ips.extract_ips()"##)
            .unwrap();
        assert_eq!(result.len(), 0);

        // Invalid IP-like patterns should be excluded
        scope.push("invalid", "300.400.500.600 and 192.168.1.1");
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r##"invalid.extract_ips()"##)
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].clone().into_string().unwrap(), "192.168.1.1");
    }

    #[test]
    fn test_mask_ip_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("ip", "192.168.1.100");

        // Default masking (last octet)
        let result: String = engine
            .eval_with_scope(&mut scope, r##"ip.mask_ip()"##)
            .unwrap();
        assert_eq!(result, "192.168.1.X");

        // Mask 2 octets
        let result: String = engine
            .eval_with_scope(&mut scope, r##"ip.mask_ip(2)"##)
            .unwrap();
        assert_eq!(result, "192.168.X.X");

        // Mask 3 octets
        let result: String = engine
            .eval_with_scope(&mut scope, r##"ip.mask_ip(3)"##)
            .unwrap();
        assert_eq!(result, "192.X.X.X");

        // Mask all 4 octets
        let result: String = engine
            .eval_with_scope(&mut scope, r##"ip.mask_ip(4)"##)
            .unwrap();
        assert_eq!(result, "X.X.X.X");

        // Invalid input (returns unchanged)
        scope.push("invalid", "not.an.ip.address");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"invalid.mask_ip()"##)
            .unwrap();
        assert_eq!(result, "not.an.ip.address");

        // Out of range values get clamped
        let result: String = engine
            .eval_with_scope(&mut scope, r##"ip.mask_ip(0)"##)
            .unwrap();
        assert_eq!(result, "192.168.1.X"); // Clamped to minimum 1

        let result: String = engine
            .eval_with_scope(&mut scope, r##"ip.mask_ip(10)"##)
            .unwrap();
        assert_eq!(result, "X.X.X.X"); // Clamped to maximum 4
    }

    #[test]
    fn test_is_private_ip_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();

        // Private IP ranges
        scope.push("private1", "10.0.0.1");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"private1.is_private_ip()"##)
            .unwrap();
        assert!(result);

        scope.push("private2", "172.16.0.1");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"private2.is_private_ip()"##)
            .unwrap();
        assert!(result);

        scope.push("private3", "192.168.1.1");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"private3.is_private_ip()"##)
            .unwrap();
        assert!(result);

        scope.push("loopback", "127.0.0.1");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"loopback.is_private_ip()"##)
            .unwrap();
        assert!(result);

        // Public IP addresses
        scope.push("public1", "8.8.8.8");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"public1.is_private_ip()"##)
            .unwrap();
        assert!(!result);

        scope.push("public2", "1.1.1.1");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"public2.is_private_ip()"##)
            .unwrap();
        assert!(!result);

        // Edge cases for 172.x.x.x range
        scope.push("edge1", "172.15.0.1");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"edge1.is_private_ip()"##)
            .unwrap();
        assert!(!result); // 172.15.x.x is not in private range

        scope.push("edge2", "172.32.0.1");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"edge2.is_private_ip()"##)
            .unwrap();
        assert!(!result); // 172.32.x.x is not in private range

        // Invalid IP addresses
        scope.push("invalid", "not.an.ip");
        let result: bool = engine
            .eval_with_scope(&mut scope, r##"invalid.is_private_ip()"##)
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn test_extract_url_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "Visit https://example.com/path for more info");

        // Extract URL
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_url()"##)
            .unwrap();
        assert_eq!(result, "https://example.com/path");

        // HTTP URL
        scope.push("http", "Go to http://test.org/page.html");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"http.extract_url()"##)
            .unwrap();
        assert_eq!(result, "http://test.org/page.html");

        // No URL found
        scope.push("no_url", "No URL in this text");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"no_url.extract_url()"##)
            .unwrap();
        assert_eq!(result, "");

        // Complex URL with parameters
        scope.push(
            "complex",
            "API endpoint: https://api.example.com/v1/users?page=2&limit=10",
        );
        let result: String = engine
            .eval_with_scope(&mut scope, r##"complex.extract_url()"##)
            .unwrap();
        assert_eq!(result, "https://api.example.com/v1/users?page=2&limit=10");

        // Multiple URLs (returns first)
        scope.push("multi", "Visit https://first.com or https://second.com");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"multi.extract_url()"##)
            .unwrap();
        assert_eq!(result, "https://first.com");
    }

    #[test]
    fn test_extract_domain_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("text", "Visit https://example.com/path for more info");

        // Extract domain from URL
        let result: String = engine
            .eval_with_scope(&mut scope, r##"text.extract_domain()"##)
            .unwrap();
        assert_eq!(result, "example.com");

        // Extract domain from email
        scope.push("email", "Contact us at support@test.org");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"email.extract_domain()"##)
            .unwrap();
        assert_eq!(result, "test.org");

        // URL takes precedence over email
        scope.push("both", "Visit https://example.com or email admin@test.org");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"both.extract_domain()"##)
            .unwrap();
        assert_eq!(result, "example.com");

        // No domain found
        scope.push("no_domain", "No domain in this text");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"no_domain.extract_domain()"##)
            .unwrap();
        assert_eq!(result, "");

        // Complex domain with subdomains
        scope.push("subdomain", "API: https://api.v2.example.com/endpoint");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"subdomain.extract_domain()"##)
            .unwrap();
        assert_eq!(result, "api.v2.example.com");

        // Domain with port (should be excluded)
        scope.push("port", "Connect to http://localhost:8080/api");
        let result: String = engine
            .eval_with_scope(&mut scope, r##"port.extract_domain()"##)
            .unwrap();
        assert_eq!(result, "localhost:8080");
    }
}
