use rhai::{Dynamic, Engine};

pub fn register_functions(engine: &mut Engine) {
    // Column extraction functions
    engine.register_fn("col", |text: &str, selector: &str| -> String {
        extract_columns(text, selector, " ", " ")
    });

    engine.register_fn("col", |text: &str, selector: &str, sep: &str| -> String {
        extract_columns(text, selector, sep, " ")
    });

    engine.register_fn(
        "col",
        |text: &str, selector: &str, sep: &str, outsep: &str| -> String {
            extract_columns(text, selector, sep, outsep)
        },
    );

    // Integer column functions (up to 6 columns)
    engine.register_fn("col", |text: &str, col: i64| -> String {
        let selector = col.to_string();
        extract_columns(text, &selector, " ", " ")
    });

    engine.register_fn("col", |text: &str, col: i64, sep: &str| -> String {
        let selector = col.to_string();
        extract_columns(text, &selector, sep, " ")
    });

    engine.register_fn("col", |text: &str, col1: i64, col2: i64| -> String {
        let selector = format!("{},{}", col1, col2);
        extract_columns(text, &selector, " ", " ")
    });

    engine.register_fn(
        "col",
        |text: &str, col1: i64, col2: i64, sep: &str| -> String {
            let selector = format!("{},{}", col1, col2);
            extract_columns(text, &selector, sep, " ")
        },
    );

    engine.register_fn(
        "col",
        |text: &str, col1: i64, col2: i64, col3: i64| -> String {
            let selector = format!("{},{},{}", col1, col2, col3);
            extract_columns(text, &selector, " ", " ")
        },
    );

    engine.register_fn(
        "col",
        |text: &str, col1: i64, col2: i64, col3: i64, sep: &str| -> String {
            let selector = format!("{},{},{}", col1, col2, col3);
            extract_columns(text, &selector, sep, " ")
        },
    );

    engine.register_fn(
        "col",
        |text: &str, col1: i64, col2: i64, col3: i64, col4: i64| -> String {
            let selector = format!("{},{},{},{}", col1, col2, col3, col4);
            extract_columns(text, &selector, " ", " ")
        },
    );

    engine.register_fn(
        "col",
        |text: &str, col1: i64, col2: i64, col3: i64, col4: i64, sep: &str| -> String {
            let selector = format!("{},{},{},{}", col1, col2, col3, col4);
            extract_columns(text, &selector, sep, " ")
        },
    );

    engine.register_fn(
        "col",
        |text: &str, col1: i64, col2: i64, col3: i64, col4: i64, col5: i64| -> String {
            let selector = format!("{},{},{},{},{}", col1, col2, col3, col4, col5);
            extract_columns(text, &selector, " ", " ")
        },
    );

    engine.register_fn(
        "col",
        |text: &str, col1: i64, col2: i64, col3: i64, col4: i64, col5: i64, sep: &str| -> String {
            let selector = format!("{},{},{},{},{}", col1, col2, col3, col4, col5);
            extract_columns(text, &selector, sep, " ")
        },
    );

    engine.register_fn(
        "col",
        |text: &str, col1: i64, col2: i64, col3: i64, col4: i64, col5: i64, col6: i64| -> String {
            let selector = format!("{},{},{},{},{},{}", col1, col2, col3, col4, col5, col6);
            extract_columns(text, &selector, " ", " ")
        },
    );

    engine.register_fn(
        "col",
        |text: &str,
         col1: i64,
         col2: i64,
         col3: i64,
         col4: i64,
         col5: i64,
         col6: i64,
         sep: &str|
         -> String {
            let selector = format!("{},{},{},{},{},{}", col1, col2, col3, col4, col5, col6);
            extract_columns(text, &selector, sep, " ")
        },
    );

    engine.register_fn(
        "cols",
        |text: &str, selectors: rhai::Array| -> rhai::Array {
            selectors
                .into_iter()
                .filter_map(|s| s.into_string().ok())
                .map(|selector| Dynamic::from(extract_columns(text, &selector, " ", " ")))
                .collect()
        },
    );

    engine.register_fn(
        "cols",
        |text: &str, selectors: rhai::Array, sep: &str| -> rhai::Array {
            selectors
                .into_iter()
                .filter_map(|s| s.into_string().ok())
                .map(|selector| Dynamic::from(extract_columns(text, &selector, sep, " ")))
                .collect()
        },
    );

    engine.register_fn(
        "cols",
        |text: &str, selectors: rhai::Array, sep: &str, outsep: &str| -> rhai::Array {
            selectors
                .into_iter()
                .filter_map(|s| s.into_string().ok())
                .map(|selector| Dynamic::from(extract_columns(text, &selector, sep, outsep)))
                .collect()
        },
    );

    // Integer cols functions (up to 6 columns)
    engine.register_fn("cols", |text: &str, col: i64| -> rhai::Array {
        let selector = col.to_string();
        vec![Dynamic::from(extract_columns(text, &selector, " ", " "))]
    });

    engine.register_fn("cols", |text: &str, col: i64, sep: &str| -> rhai::Array {
        let selector = col.to_string();
        vec![Dynamic::from(extract_columns(text, &selector, sep, " "))]
    });

    engine.register_fn("cols", |text: &str, col1: i64, col2: i64| -> rhai::Array {
        vec![
            Dynamic::from(extract_columns(text, &col1.to_string(), " ", " ")),
            Dynamic::from(extract_columns(text, &col2.to_string(), " ", " ")),
        ]
    });

    engine.register_fn(
        "cols",
        |text: &str, col1: i64, col2: i64, sep: &str| -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), sep, " ")),
            ]
        },
    );

    engine.register_fn(
        "cols",
        |text: &str, col1: i64, col2: i64, col3: i64| -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col3.to_string(), " ", " ")),
            ]
        },
    );

    engine.register_fn(
        "cols",
        |text: &str, col1: i64, col2: i64, col3: i64, sep: &str| -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col3.to_string(), sep, " ")),
            ]
        },
    );

    engine.register_fn(
        "cols",
        |text: &str, col1: i64, col2: i64, col3: i64, col4: i64| -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col3.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col4.to_string(), " ", " ")),
            ]
        },
    );

    engine.register_fn(
        "cols",
        |text: &str, col1: i64, col2: i64, col3: i64, col4: i64, sep: &str| -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col3.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col4.to_string(), sep, " ")),
            ]
        },
    );

    engine.register_fn(
        "cols",
        |text: &str, col1: i64, col2: i64, col3: i64, col4: i64, col5: i64| -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col3.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col4.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col5.to_string(), " ", " ")),
            ]
        },
    );

    engine.register_fn(
        "cols",
        |text: &str,
         col1: i64,
         col2: i64,
         col3: i64,
         col4: i64,
         col5: i64,
         sep: &str|
         -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col3.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col4.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col5.to_string(), sep, " ")),
            ]
        },
    );

    engine.register_fn(
        "cols",
        |text: &str,
         col1: i64,
         col2: i64,
         col3: i64,
         col4: i64,
         col5: i64,
         col6: i64|
         -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col3.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col4.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col5.to_string(), " ", " ")),
                Dynamic::from(extract_columns(text, &col6.to_string(), " ", " ")),
            ]
        },
    );

    engine.register_fn(
        "cols",
        |text: &str,
         col1: i64,
         col2: i64,
         col3: i64,
         col4: i64,
         col5: i64,
         col6: i64,
         sep: &str|
         -> rhai::Array {
            vec![
                Dynamic::from(extract_columns(text, &col1.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col2.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col3.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col4.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col5.to_string(), sep, " ")),
                Dynamic::from(extract_columns(text, &col6.to_string(), sep, " ")),
            ]
        },
    );

    // Log analysis functions
    engine.register_fn("status_class", |status: i64| -> String {
        match status {
            100..=199 => "1xx".to_string(),
            200..=299 => "2xx".to_string(),
            300..=399 => "3xx".to_string(),
            400..=499 => "4xx".to_string(),
            500..=599 => "5xx".to_string(),
            _ => "unknown".to_string(),
        }
    });
}

/// Extract columns from text using selector syntax
fn extract_columns(text: &str, selector: &str, sep: &str, outsep: &str) -> String {
    let columns = split_text(text, sep);
    let indices = parse_selector(selector, columns.len());

    let selected: Vec<String> = indices
        .into_iter()
        .filter_map(|i| columns.get(i).map(|s| s.to_string()))
        .collect();

    selected.join(outsep)
}

/// Split text by separator, handling whitespace specially
fn split_text<'a>(text: &'a str, sep: &str) -> Vec<&'a str> {
    if sep == " " {
        // Whitespace splitting - split on any whitespace and filter empty
        text.split_whitespace().collect()
    } else {
        // Regular splitting
        text.split(sep).collect()
    }
}

/// Parse selector string into column indices
fn parse_selector(selector: &str, total_cols: usize) -> Vec<usize> {
    let mut indices = Vec::new();

    // Handle comma-separated selectors
    for part in selector.split(',') {
        let part = part.trim();

        if part.contains(':') {
            // Range selector like "1:3" or "2:" or ":4"
            let range_indices = parse_range_selector(part, total_cols);
            indices.extend(range_indices);
        } else {
            // Single index selector
            if let Ok(idx) = part.parse::<i32>() {
                if let Some(pos) = normalize_index(idx, total_cols) {
                    indices.push(pos);
                }
            }
        }
    }

    indices
}

/// Parse range selector like "1:3" into indices
fn parse_range_selector(range: &str, total_cols: usize) -> Vec<usize> {
    let parts: Vec<&str> = range.split(':').collect();
    if parts.len() != 2 {
        return Vec::new();
    }

    let start = if parts[0].trim().is_empty() {
        0
    } else {
        match parts[0].trim().parse::<i32>() {
            Ok(idx) => match normalize_index(idx, total_cols) {
                Some(normalized) => normalized,
                None => return Vec::new(), // Out of bounds start index
            },
            Err(_) => return Vec::new(),
        }
    };

    let end = if parts[1].trim().is_empty() {
        total_cols
    } else {
        match parts[1].trim().parse::<i32>() {
            Ok(idx) => {
                let len = total_cols as i32;
                let normalized = if idx < 0 { len + idx } else { idx };
                if normalized < 0 {
                    return Vec::new();
                }
                normalized.min(len) as usize
            }
            Err(_) => return Vec::new(),
        }
    };

    if start >= end {
        return Vec::new();
    }

    (start..end.min(total_cols)).collect()
}

/// Convert potentially negative index to positive index
fn normalize_index(idx: i32, total_cols: usize) -> Option<usize> {
    let len = total_cols as i32;
    let normalized = if idx < 0 { len + idx } else { idx };

    if normalized >= 0 && normalized < len {
        Some(normalized as usize)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_columns_basic() {
        assert_eq!(extract_columns("alpha beta gamma", "0", " ", " "), "alpha");
        assert_eq!(extract_columns("alpha beta gamma", "1", " ", " "), "beta");
        assert_eq!(extract_columns("alpha beta gamma", "2", " ", " "), "gamma");
        assert_eq!(extract_columns("alpha beta gamma", "-1", " ", " "), "gamma");
    }

    #[test]
    fn test_extract_columns_range() {
        assert_eq!(
            extract_columns("alpha beta gamma delta", "1:3", " ", " "),
            "beta gamma"
        );
        assert_eq!(
            extract_columns("alpha beta gamma delta", "2:", " ", " "),
            "gamma delta"
        );
        assert_eq!(
            extract_columns("alpha beta gamma delta", ":2", " ", " "),
            "alpha beta"
        );
    }

    #[test]
    fn test_extract_columns_multiple() {
        assert_eq!(
            extract_columns("alpha beta gamma delta", "0,2", " ", " "),
            "alpha gamma"
        );
        assert_eq!(
            extract_columns("alpha beta gamma delta", "1,3", " ", " "),
            "beta delta"
        );
        assert_eq!(
            extract_columns("alpha beta gamma delta", "0,2,3", " ", " "),
            "alpha gamma delta"
        );
    }

    #[test]
    fn test_extract_columns_custom_separator() {
        assert_eq!(extract_columns("alpha|beta|gamma", "0", "|", " "), "alpha");
        assert_eq!(
            extract_columns("alpha|beta|gamma", "1:3", "|", "|"),
            "beta|gamma"
        );
        assert_eq!(
            extract_columns("alpha,beta,gamma", "0,2", ",", "-"),
            "alpha-gamma"
        );
    }

    #[test]
    fn test_extract_columns_out_of_bounds() {
        assert_eq!(extract_columns("alpha beta", "5", " ", " "), "");

        // This should only extract column 0 since column 5 doesn't exist
        let result = extract_columns("alpha beta", "0,5", " ", " ");
        assert_eq!(result, "alpha");

        let range_result = extract_columns("alpha beta", "5:7", " ", " ");
        assert_eq!(range_result, "");
    }

    #[test]
    fn test_split_text_whitespace() {
        let result = split_text("  alpha   beta   gamma  ", " ");
        assert_eq!(result, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_split_text_custom() {
        let result = split_text("alpha|beta||gamma", "|");
        assert_eq!(result, vec!["alpha", "beta", "", "gamma"]);
    }

    #[test]
    fn test_parse_selector_single() {
        assert_eq!(parse_selector("0", 3), vec![0]);
        assert_eq!(parse_selector("2", 3), vec![2]);
        assert_eq!(parse_selector("-1", 3), vec![2]);
    }

    #[test]
    fn test_parse_selector_range() {
        assert_eq!(parse_selector("1:3", 5), vec![1, 2]);
        assert_eq!(parse_selector("2:", 5), vec![2, 3, 4]);
        assert_eq!(parse_selector(":2", 5), vec![0, 1]);
    }

    #[test]
    fn test_parse_selector_multiple() {
        assert_eq!(parse_selector("0,2", 5), vec![0, 2]);
        assert_eq!(parse_selector("1,3,4", 5), vec![1, 3, 4]);
    }

    #[test]
    fn test_normalize_index() {
        assert_eq!(normalize_index(0, 3), Some(0));
        assert_eq!(normalize_index(2, 3), Some(2));
        assert_eq!(normalize_index(-1, 3), Some(2));
        assert_eq!(normalize_index(-3, 3), Some(0));
        assert_eq!(normalize_index(3, 3), None);
        assert_eq!(normalize_index(-4, 3), None);
    }

    #[test]
    fn test_col_function_integration() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = rhai::Scope::new();
        scope.push("line", "alpha beta gamma delta");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"line.col("0")"#)
            .unwrap();
        assert_eq!(result, "alpha");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"line.col("1:3")"#)
            .unwrap();
        assert_eq!(result, "beta gamma");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"line.col("0,2")"#)
            .unwrap();
        assert_eq!(result, "alpha gamma");
    }

    #[test]
    fn test_cols_function_integration() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = rhai::Scope::new();
        scope.push("line", "alpha beta gamma delta");

        // Test single selector
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r#"line.cols(["0"])"#)
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].clone().into_string().unwrap(), "alpha");

        // Test multiple selectors
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r#"line.cols(["0", "2"])"#)
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].clone().into_string().unwrap(), "alpha");
        assert_eq!(result[1].clone().into_string().unwrap(), "gamma");

        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r#"line.cols(["1:3", "-1"])"#)
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].clone().into_string().unwrap(), "beta gamma");
        assert_eq!(result[1].clone().into_string().unwrap(), "delta");
    }

    #[test]
    fn test_col_integer_functions() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = rhai::Scope::new();
        scope.push("line", "alpha beta gamma delta");

        // Single integer
        let result: String = engine
            .eval_with_scope(&mut scope, r#"line.col(0)"#)
            .unwrap();
        assert_eq!(result, "alpha");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"line.col(2)"#)
            .unwrap();
        assert_eq!(result, "gamma");

        // Multiple integers
        let result: String = engine
            .eval_with_scope(&mut scope, r#"line.col(0, 2)"#)
            .unwrap();
        assert_eq!(result, "alpha gamma");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"line.col(1, 2, 3)"#)
            .unwrap();
        assert_eq!(result, "beta gamma delta");

        // With custom separator
        scope.push("csv_line", "alpha,beta,gamma,delta");
        let result: String = engine
            .eval_with_scope(&mut scope, r#"csv_line.col(0, 2, ",")"#)
            .unwrap();
        assert_eq!(result, "alpha gamma");
    }

    #[test]
    fn test_cols_integer_functions() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = rhai::Scope::new();
        scope.push("line", "alpha beta gamma delta");

        // Single integer
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r#"line.cols(0)"#)
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].clone().into_string().unwrap(), "alpha");

        // Multiple integers
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r#"line.cols(0, 2)"#)
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].clone().into_string().unwrap(), "alpha");
        assert_eq!(result[1].clone().into_string().unwrap(), "gamma");

        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r#"line.cols(1, 2, 3)"#)
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].clone().into_string().unwrap(), "beta");
        assert_eq!(result[1].clone().into_string().unwrap(), "gamma");
        assert_eq!(result[2].clone().into_string().unwrap(), "delta");

        // With custom separator
        scope.push("csv_line", "alpha,beta,gamma,delta");
        let result: rhai::Array = engine
            .eval_with_scope(&mut scope, r#"csv_line.cols(0, 2, ",")"#)
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].clone().into_string().unwrap(), "alpha");
        assert_eq!(result[1].clone().into_string().unwrap(), "gamma");
    }
}
