use rhai::{Dynamic, Engine, EvalAltResult, ImmutableString, Map};
use std::cell::Cell;
use std::collections::HashSet;

thread_local! {
    static ABSORB_STRICT: Cell<bool> = const { Cell::new(false) };
}

pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("absorb_kv", absorb_kv_default);
    engine.register_fn("absorb_kv", absorb_kv_with_options);
}

pub fn set_absorb_strict(strict: bool) {
    ABSORB_STRICT.with(|flag| flag.set(strict));
}

fn is_absorb_strict() -> bool {
    ABSORB_STRICT.with(|flag| flag.get())
}

fn absorb_kv_default(event: &mut Map, field: &str) -> Result<Map, Box<EvalAltResult>> {
    finalize_result(absorb_kv_impl(event, field, None))
}

fn absorb_kv_with_options(
    event: &mut Map,
    field: &str,
    options: Map,
) -> Result<Map, Box<EvalAltResult>> {
    finalize_result(absorb_kv_impl(event, field, Some(&options)))
}

fn finalize_result(result: AbsorbResult) -> Result<Map, Box<EvalAltResult>> {
    if result.status == AbsorbStatus::InvalidOption && is_absorb_strict() {
        let message = result
            .error
            .clone()
            .unwrap_or_else(|| "invalid absorb option".to_string());
        return Err(format!("absorb_kv: {}", message).into());
    }

    Ok(result.into_map())
}

fn absorb_kv_impl(event: &mut Map, field: &str, options: Option<&Map>) -> AbsorbResult {
    let opts = match AbsorbOptions::from_map(options) {
        Ok(opts) => opts,
        Err(err) => return AbsorbResult::invalid_option(err),
    };

    let field_value = match event.get(field) {
        Some(value) => value.clone(),
        None => return AbsorbResult::new(AbsorbStatus::MissingField),
    };

    let immutable = match field_value.try_cast::<ImmutableString>() {
        Some(value) => value,
        None => return AbsorbResult::new(AbsorbStatus::NotString),
    };

    let text = immutable.into_owned();
    let mut tokens = opts.separator.split_tokens(&text);
    let had_tokens = !tokens.is_empty();
    let mut remainder_tokens: Vec<String> = Vec::new();
    let mut parsed_pairs: Vec<(String, String)> = Vec::new();

    for token in tokens.drain(..) {
        let token_str = token.as_str();
        if let Some(idx) = token_str.find(&opts.kv_sep) {
            let key = token_str[..idx].trim();
            let value = token_str[idx + opts.kv_sep.len()..].trim();

            if key.is_empty() {
                remainder_tokens.push(token);
                continue;
            }

            parsed_pairs.push((key.to_string(), value.to_string()));
        } else {
            remainder_tokens.push(token);
        }
    }

    let remainder = opts.separator.join_tokens(&remainder_tokens);
    let mut result = AbsorbResult::new(if parsed_pairs.is_empty() {
        AbsorbStatus::Empty
    } else {
        AbsorbStatus::Applied
    });
    result.remainder = remainder.clone();
    result.data = build_data_map(&parsed_pairs);

    if parsed_pairs.is_empty() {
        if !opts.keep_source && remainder.is_none() && !had_tokens && event.remove(field).is_some()
        {
            result.removed_source = true;
        }

        return result;
    }

    let mut wrote = false;
    let preexisting_keys = if opts.overwrite {
        None
    } else {
        Some(
            event
                .keys()
                .map(|key| key.to_string())
                .collect::<HashSet<String>>(),
        )
    };

    for (key, value) in &parsed_pairs {
        if !opts.overwrite {
            if let Some(existing) = &preexisting_keys {
                if existing.contains(key) {
                    continue;
                }
            }

            event.insert(key.clone().into(), Dynamic::from(value.clone()));
            wrote = true;
            continue;
        }

        event.insert(key.clone().into(), Dynamic::from(value.clone()));
        wrote = true;
    }

    result.written = wrote;

    if !opts.keep_source {
        match remainder {
            Some(ref text) => {
                event.insert(field.into(), Dynamic::from(text.clone()));
            }
            None => {
                if event.remove(field).is_some() {
                    result.removed_source = true;
                }
            }
        }
    }

    result
}

fn build_data_map(pairs: &[(String, String)]) -> Map {
    let mut data = Map::new();
    for (key, value) in pairs {
        data.insert(key.clone().into(), Dynamic::from(value.clone()));
    }
    data
}

#[derive(Debug, Clone)]
struct AbsorbResult {
    status: AbsorbStatus,
    data: Map,
    written: bool,
    remainder: Option<String>,
    removed_source: bool,
    error: Option<String>,
}

impl AbsorbResult {
    fn new(status: AbsorbStatus) -> Self {
        Self {
            status,
            data: Map::new(),
            written: false,
            remainder: None,
            removed_source: false,
            error: None,
        }
    }

    fn invalid_option(err: OptionsError) -> Self {
        Self {
            status: AbsorbStatus::InvalidOption,
            data: Map::new(),
            written: false,
            remainder: None,
            removed_source: false,
            error: Some(err.message),
        }
    }

    fn into_map(self) -> Map {
        let mut map = Map::new();
        map.insert("status".into(), Dynamic::from(self.status.as_str()));
        map.insert("data".into(), Dynamic::from(self.data));
        map.insert("written".into(), Dynamic::from(self.written));
        match self.remainder {
            Some(text) => {
                map.insert("remainder".into(), Dynamic::from(text));
            }
            None => {
                map.insert("remainder".into(), Dynamic::UNIT);
            }
        }
        map.insert("removed_source".into(), Dynamic::from(self.removed_source));
        match self.error {
            Some(err) => {
                map.insert("error".into(), Dynamic::from(err));
            }
            None => {
                map.insert("error".into(), Dynamic::UNIT);
            }
        }
        map
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AbsorbStatus {
    Applied,
    MissingField,
    NotString,
    Empty,
    #[allow(dead_code)]
    ParseError,
    InvalidOption,
}

impl AbsorbStatus {
    fn as_str(&self) -> &'static str {
        match self {
            AbsorbStatus::Applied => "applied",
            AbsorbStatus::MissingField => "missing_field",
            AbsorbStatus::NotString => "not_string",
            AbsorbStatus::Empty => "empty",
            AbsorbStatus::ParseError => "parse_error",
            AbsorbStatus::InvalidOption => "invalid_option",
        }
    }
}

#[derive(Debug, Clone)]
struct AbsorbOptions {
    separator: TokenSeparator,
    kv_sep: String,
    keep_source: bool,
    overwrite: bool,
}

impl Default for AbsorbOptions {
    fn default() -> Self {
        Self {
            separator: TokenSeparator::Whitespace,
            kv_sep: "=".to_string(),
            keep_source: false,
            overwrite: true,
        }
    }
}

impl AbsorbOptions {
    fn from_map(map: Option<&Map>) -> Result<Self, OptionsError> {
        let mut options = Self::default();

        if let Some(opts) = map {
            for (key, value) in opts.iter() {
                match key.as_str() {
                    "sep" => {
                        if value.is_unit() {
                            options.separator = TokenSeparator::Whitespace;
                        } else if let Some(sep) = value.clone().try_cast::<ImmutableString>() {
                            let sep = sep.into_owned();
                            if sep.is_empty() {
                                return Err(OptionsError::invalid_value(
                                    "sep",
                                    "must not be empty",
                                ));
                            }
                            options.separator = TokenSeparator::Literal(sep);
                        } else {
                            return Err(OptionsError::invalid_type("sep", "string or ()"));
                        }
                    }
                    "kv_sep" => {
                        if let Some(sep) = value.clone().try_cast::<ImmutableString>() {
                            let sep = sep.into_owned();
                            if sep.is_empty() {
                                return Err(OptionsError::invalid_value(
                                    "kv_sep",
                                    "must not be empty",
                                ));
                            }
                            options.kv_sep = sep;
                        } else {
                            return Err(OptionsError::invalid_type("kv_sep", "string"));
                        }
                    }
                    "keep_source" => {
                        if let Some(flag) = value.clone().try_cast::<bool>() {
                            options.keep_source = flag;
                        } else {
                            return Err(OptionsError::invalid_type("keep_source", "bool"));
                        }
                    }
                    "overwrite" => {
                        if let Some(flag) = value.clone().try_cast::<bool>() {
                            options.overwrite = flag;
                        } else {
                            return Err(OptionsError::invalid_type("overwrite", "bool"));
                        }
                    }
                    other => {
                        return Err(OptionsError::unknown(other));
                    }
                }
            }
        }

        Ok(options)
    }
}

#[derive(Debug, Clone)]
enum TokenSeparator {
    Whitespace,
    Literal(String),
}

impl TokenSeparator {
    fn split_tokens(&self, text: &str) -> Vec<String> {
        match self {
            TokenSeparator::Whitespace => text
                .split_whitespace()
                .map(|token| token.to_string())
                .collect(),
            TokenSeparator::Literal(sep) => text
                .split(sep)
                .map(|token| token.trim())
                .filter(|token| !token.is_empty())
                .map(|token| token.to_string())
                .collect(),
        }
    }

    fn join_tokens(&self, tokens: &[String]) -> Option<String> {
        if tokens.is_empty() {
            return None;
        }

        let joined = match self {
            TokenSeparator::Whitespace => tokens.join(" "),
            TokenSeparator::Literal(sep) => tokens.join(sep),
        };

        Some(joined)
    }
}

#[derive(Debug, Clone)]
struct OptionsError {
    message: String,
}

impl OptionsError {
    fn unknown(key: &str) -> Self {
        Self {
            message: format!("unknown absorb option: {}", key),
        }
    }

    fn invalid_type(key: &str, expected: &str) -> Self {
        Self {
            message: format!(
                "invalid absorb option type for {}: expected {}",
                key, expected
            ),
        }
    }

    fn invalid_value(key: &str, message: &str) -> Self {
        Self {
            message: format!("invalid value for absorb option {}: {}", key, message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_string(value: &str) -> Dynamic {
        Dynamic::from(value.to_string())
    }

    #[test]
    fn absorb_kv_basic_merge() {
        set_absorb_strict(false);
        let mut event = Map::new();
        event.insert(
            "msg".into(),
            map_string("Payment timeout order=1234 gateway=stripe"),
        );

        let result = absorb_kv_impl(&mut event, "msg", None);
        assert_eq!(result.status, AbsorbStatus::Applied);
        assert!(result.written);
        assert_eq!(result.remainder.as_deref(), Some("Payment timeout"));
        assert_eq!(event.get("order").unwrap().to_string(), "1234");
        assert_eq!(event.get("gateway").unwrap().to_string(), "stripe");
    }

    #[test]
    fn absorb_kv_keep_source_preserves_field() {
        set_absorb_strict(false);
        let mut event = Map::new();
        event.insert("msg".into(), map_string("prefix user=alice suffix"));

        let mut options = Map::new();
        options.insert("keep_source".into(), Dynamic::from(true));

        let result = absorb_kv_impl(&mut event, "msg", Some(&options));
        assert_eq!(result.status, AbsorbStatus::Applied);
        assert_eq!(
            event.get("msg").unwrap().to_string(),
            "prefix user=alice suffix"
        );
        assert_eq!(result.remainder.as_deref(), Some("prefix suffix"));
    }

    #[test]
    fn absorb_kv_overwrite_false_skips_existing() {
        set_absorb_strict(false);
        let mut event = Map::new();
        event.insert("status".into(), map_string("pending"));
        event.insert("msg".into(), map_string("Processing status=active"));

        let mut options = Map::new();
        options.insert("overwrite".into(), Dynamic::from(false));

        let result = absorb_kv_impl(&mut event, "msg", Some(&options));
        assert_eq!(result.status, AbsorbStatus::Applied);
        assert!(!result.written);
        assert_eq!(event.get("status").unwrap().to_string(), "pending");
        assert_eq!(result.data.get("status").unwrap().to_string(), "active");
    }

    #[test]
    fn absorb_kv_invalid_option_sets_status() {
        set_absorb_strict(false);
        let mut event = Map::new();
        event.insert("msg".into(), map_string("user=alice"));

        let mut options = Map::new();
        options.insert("keep_sorce".into(), Dynamic::from(true));

        let result = absorb_kv_impl(&mut event, "msg", Some(&options));
        assert_eq!(result.status, AbsorbStatus::InvalidOption);
        assert_eq!(
            result.error.as_deref(),
            Some("unknown absorb option: keep_sorce")
        );
    }

    #[test]
    fn absorb_kv_empty_string_removes_field() {
        set_absorb_strict(false);
        let mut event = Map::new();
        event.insert("msg".into(), map_string("   "));

        let result = absorb_kv_impl(&mut event, "msg", None);
        assert_eq!(result.status, AbsorbStatus::Empty);
        assert!(result.remainder.is_none());
        assert!(!event.contains_key("msg"));
        assert!(result.removed_source);
    }
}
