#![no_main]

use kelora::parsers::RegexParser;
use kelora::pipeline::EventParser;
use libfuzzer_sys::fuzz_target;

const MAX_PATTERN_LEN: usize = 512;
const MAX_LINE_LEN: usize = 2048;

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 {
        return;
    }

    let flags = data[0];
    let strict = flags & 0x1 == 0x1;
    let inject_type = flags & 0x2 == 0x2;
    let structured = flags & 0x4 == 0x4;
    let expect_invalid = flags & 0x8 == 0x8;
    let type_choice = (flags >> 4) & 0x3;

    let mut idx = 1;

    if idx >= data.len() {
        return;
    }
    let mut pattern_len = data[idx] as usize;
    idx += 1;

    if idx >= data.len() {
        return;
    }
    let mut line_len_hint = data[idx] as usize;
    idx += 1;

    if idx >= data.len() {
        return;
    }

    let available = data.len() - idx;
    if available == 0 {
        return;
    }

    pattern_len = pattern_len.min(available);
    pattern_len = pattern_len.min(MAX_PATTERN_LEN);
    if pattern_len == 0 || idx + pattern_len > data.len() {
        return;
    }
    let pattern_bytes = &data[idx..idx + pattern_len];
    idx += pattern_len;

    if idx >= data.len() {
        return;
    }
    let remaining = data.len() - idx;
    if remaining == 0 {
        return;
    }
    line_len_hint = line_len_hint.min(remaining);
    let mut line_len = line_len_hint.min(MAX_LINE_LEN);
    if line_len == 0 {
        line_len = remaining.min(MAX_LINE_LEN);
    }
    if idx + line_len > data.len() {
        return;
    }
    let line_bytes = &data[idx..idx + line_len];

    let pattern_seed = match std::str::from_utf8(pattern_bytes) {
        Ok(s) => s,
        Err(_) => return,
    };
    let line_seed = match std::str::from_utf8(line_bytes) {
        Ok(s) => s,
        Err(_) => return,
    };

    let (pattern, line) = if structured {
        build_structured_case(pattern_seed, line_seed, type_choice as u8, expect_invalid)
    } else {
        (
            ensure_named_group(pattern_seed, inject_type, type_choice as u8),
            build_line_candidate(line_seed, pattern_seed),
        )
    };

    if pattern.is_empty() {
        return;
    }

    let mut parser = match RegexParser::new(&pattern) {
        Ok(p) => p,
        Err(_) => return,
    };
    parser = parser.with_strict(strict);

    let _ = parser.parse(&line);
});

fn ensure_named_group(pattern: &str, inject_type: bool, type_choice: u8) -> String {
    if pattern.contains("(?P<") {
        return truncate(pattern, MAX_PATTERN_LEN);
    }

    let field_name = sanitize_field_name(pattern);
    let type_suffix = if inject_type {
        match type_choice % 4 {
            0 => ":int",
            1 => ":float",
            2 => ":bool",
            _ => ":unknown",
        }
    } else {
        ""
    };

    let inner = if pattern.trim().is_empty() { ".*" } else { pattern };
    format!("(?P<{}{}>{})", field_name, type_suffix, truncate(inner, MAX_PATTERN_LEN))
}

fn build_line_candidate(line_seed: &str, pattern_seed: &str) -> String {
    if !line_seed.is_empty() {
        return truncate(line_seed, MAX_LINE_LEN);
    }

    let fallback: String = pattern_seed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace())
        .take(64)
        .collect();

    if fallback.is_empty() {
        "fallback".to_string()
    } else {
        fallback
    }
}

fn build_structured_case(
    pattern_seed: &str,
    line_seed: &str,
    type_choice: u8,
    produce_invalid: bool,
) -> (String, String) {
    let field_name = sanitize_field_name(pattern_seed);

    match type_choice % 4 {
        0 => {
            let value = if line_seed.trim().is_empty() {
                "default".to_string()
            } else {
                truncate(line_seed, MAX_LINE_LEN)
            };
            (format!(r"(?P<{}>.+)", field_name), value)
        }
        1 => {
            let digits = extract_digits(line_seed);
            let invalid = extract_letters(line_seed);
            let value = if produce_invalid { invalid } else { digits };
            (
                format!(r"(?P<{}:int>[[:alnum:]\+\-]+)", field_name),
                value,
            )
        }
        2 => {
            let float_value = build_float_value(line_seed);
            let invalid = format!("{}{}", extract_letters(line_seed), extract_letters(pattern_seed));
            let value = if produce_invalid {
                if invalid.is_empty() {
                    "NaNvalue".to_string()
                } else {
                    invalid
                }
            } else {
                float_value
            };
            (
                format!(r"(?P<{}:float>[[:alnum:]\+\-\.]+)", field_name),
                value,
            )
        }
        _ => {
            let valid_values = ["true", "false", "1", "0", "yes", "no", "TRUE", "FALSE"];
            let idx = line_seed.len() % valid_values.len();
            let bool_value = valid_values[idx].to_string();
            let invalid = if let Some(first) = extract_letters(line_seed).chars().next() {
                format!("maybe{}", first)
            } else {
                "maybe".to_string()
            };
            let value = if produce_invalid { invalid } else { bool_value };
            (
                format!(r"(?P<{}:bool>[[:alnum:]]+)", field_name),
                value,
            )
        }
    }
}

fn sanitize_field_name(input: &str) -> String {
    let mut name: String = input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(16)
        .collect();

    if name.is_empty() {
        name.push('f');
    }

    name
}

fn extract_digits(seed: &str) -> String {
    let digits: String = seed.chars().filter(|c| c.is_ascii_digit()).take(18).collect();
    if digits.is_empty() {
        "0".to_string()
    } else {
        digits
    }
}

fn extract_letters(seed: &str) -> String {
    let letters: String = seed
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .take(18)
        .collect();
    if letters.is_empty() {
        "abc".to_string()
    } else {
        letters
    }
}

fn build_float_value(seed: &str) -> String {
    let digits = extract_digits(seed);
    let split = digits.len().saturating_sub(1);
    let (int_part, frac_part) = digits.split_at(split);
    let int_part = if int_part.is_empty() { "0" } else { int_part };
    let frac_part = if frac_part.is_empty() { "0" } else { frac_part };
    format!("{}.{}", int_part, frac_part)
}

fn truncate(input: &str, max_len: usize) -> String {
    input.chars().take(max_len).collect()
}
