use rhai::{Dynamic, Engine};
use std::cell::RefCell;

thread_local! {
    static CAPTURED_PRINTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static CAPTURED_EPRINTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static PARALLEL_MODE: RefCell<bool> = const { RefCell::new(false) };
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

    // Custom eprint function that captures output in parallel mode
    engine.register_fn("eprint", |message: rhai::Dynamic| {
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
                        captures.get(1).map(|m| m.as_str()).unwrap_or("").to_string()
                    } else {
                        captures.get(0).map(|m| m.as_str()).unwrap_or("").to_string()
                    }
                } else {
                    String::new()
                }
            }
            Err(_) => String::new(), // Invalid regex returns empty string
        }
    });

    engine.register_fn("extract_all_re", |text: &str, pattern: &str| -> rhai::Array {
        match regex::Regex::new(pattern) {
            Ok(re) => {
                let mut results = rhai::Array::new();
                for captures in re.captures_iter(text) {
                    if captures.len() > 1 {
                        // Multiple capture groups - return array of groups
                        let groups: rhai::Array = captures
                            .iter()
                            .skip(1) // Skip full match (index 0)
                            .filter_map(|m| m.map(|match_| Dynamic::from(match_.as_str().to_string())))
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
    });

    engine.register_fn("split_re", |text: &str, pattern: &str| -> rhai::Array {
        match regex::Regex::new(pattern) {
            Ok(re) => re
                .split(text)
                .map(|s| Dynamic::from(s.to_string()))
                .collect(),
            Err(_) => vec![Dynamic::from(text.to_string())], // Invalid regex returns original string
        }
    });

    engine.register_fn("replace_re", |text: &str, pattern: &str, replacement: &str| -> String {
        match regex::Regex::new(pattern) {
            Ok(re) => re.replace_all(text, replacement).to_string(),
            Err(_) => text.to_string(), // Invalid regex returns original string
        }
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
        assert_eq!(result[0].clone().into_string().unwrap(), "one,two;three:four");
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
            .eval_with_scope(&mut scope, r##"emails.replace_re("(\\w+)@(\\w+\\.\\w+)", "[$1 at $2]")"##)
            .unwrap();
        assert_eq!(result, "Contact [alice at example.com] or [bob at test.org]");
        
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
}
