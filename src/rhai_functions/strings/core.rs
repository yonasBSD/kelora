use rhai::{Array, Dynamic, Engine};
use std::convert::TryFrom;

fn edit_distance_impl(lhs: &str, rhs: &str) -> i64 {
    if lhs == rhs {
        return 0;
    }

    let lhs_chars: Vec<char> = lhs.chars().collect();
    let rhs_chars: Vec<char> = rhs.chars().collect();
    let len_lhs = lhs_chars.len();
    let len_rhs = rhs_chars.len();

    if len_lhs == 0 {
        return i64::try_from(len_rhs).unwrap_or(i64::MAX);
    }
    if len_rhs == 0 {
        return i64::try_from(len_lhs).unwrap_or(i64::MAX);
    }

    let mut prev: Vec<usize> = (0..=len_rhs).collect();
    let mut curr: Vec<usize> = vec![0; len_rhs + 1];

    for (i, &lhs_ch) in lhs_chars.iter().enumerate() {
        curr[0] = i + 1;

        for (j, &rhs_ch) in rhs_chars.iter().enumerate() {
            let cost = usize::from(lhs_ch != rhs_ch);
            let insertion = curr[j] + 1;
            let deletion = prev[j + 1] + 1;
            let substitution = prev[j] + cost;
            curr[j + 1] = insertion.min(deletion).min(substitution);
        }

        std::mem::swap(&mut prev, &mut curr);
    }

    i64::try_from(prev[len_rhs]).unwrap_or(i64::MAX)
}

pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("contains", |text: &str, pattern: &str| {
        text.contains(pattern)
    });

    engine.register_fn("to_int", |text: &str| -> rhai::Dynamic {
        text.parse::<i64>()
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT)
    });

    engine.register_fn("to_float", |text: &str| -> rhai::Dynamic {
        text.parse::<f64>()
            .map(Dynamic::from)
            .unwrap_or(Dynamic::UNIT)
    });

    engine.register_fn("or_empty", |text: &str| -> rhai::Dynamic {
        if text.is_empty() {
            Dynamic::UNIT
        } else {
            Dynamic::from(text.to_string())
        }
    });

    engine.register_fn("or_empty", |_unit: ()| -> rhai::Dynamic { Dynamic::UNIT });

    engine.register_fn("or_empty", |arr: rhai::Array| -> rhai::Dynamic {
        if arr.is_empty() {
            Dynamic::UNIT
        } else {
            Dynamic::from(arr)
        }
    });

    engine.register_fn("or_empty", |map: rhai::Map| -> rhai::Dynamic {
        if map.is_empty() {
            Dynamic::UNIT
        } else {
            Dynamic::from(map)
        }
    });

    engine.register_fn("lower", |text: &str| -> String { text.to_lowercase() });

    engine.register_fn("upper", |text: &str| -> String { text.to_uppercase() });

    engine.register_fn("is_digit", |text: &str| -> bool {
        !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
    });

    engine.register_fn("count", |text: &str, pattern: &str| -> i64 {
        if pattern.is_empty() {
            return 0;
        }
        text.matches(pattern).count() as i64
    });

    engine.register_fn("edit_distance", edit_distance_impl);

    engine.register_fn("join", |separator: &str, items: Array| -> String {
        items
            .into_iter()
            .filter_map(|item| item.into_string().ok())
            .collect::<Vec<String>>()
            .join(separator)
    });

    engine.register_fn("join", |items: Array, separator: &str| -> String {
        items
            .into_iter()
            .filter_map(|item| item.into_string().ok())
            .collect::<Vec<String>>()
            .join(separator)
    });
}
