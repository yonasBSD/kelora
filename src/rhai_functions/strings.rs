use crate::event::Event;
use crate::parsers::{CefParser, CombinedParser, LogfmtParser, SyslogParser};
use crate::pipeline::EventParser;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine as _;
use once_cell::sync::Lazy;
use rhai::{Array, Dynamic, Engine, Map};
use std::cell::RefCell;
use std::path::Path;
use url::Url;

/// Represents a captured message with its target stream
#[derive(Debug, Clone)]
pub enum CapturedMessage {
    Stdout(String),
    Stderr(String),
}

thread_local! {
    static CAPTURED_PRINTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static CAPTURED_EPRINTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static CAPTURED_MESSAGES: RefCell<Vec<CapturedMessage>> = const { RefCell::new(Vec::new()) };
    static PARALLEL_MODE: RefCell<bool> = const { RefCell::new(false) };
    static SUPPRESS_SIDE_EFFECTS: RefCell<bool> = const { RefCell::new(false) };
}

const MAX_PARSE_LEN: usize = 1_048_576;

static LOGFMT_PARSER: Lazy<LogfmtParser> = Lazy::new(LogfmtParser::new);
static SYSLOG_PARSER: Lazy<SyslogParser> =
    Lazy::new(|| SyslogParser::new().expect("failed to initialize syslog parser"));
static CEF_PARSER: Lazy<CefParser> = Lazy::new(CefParser::new);
static COMBINED_PARSER: Lazy<CombinedParser> =
    Lazy::new(|| CombinedParser::new().expect("failed to initialize combined parser"));

fn event_to_map(event: Event) -> Map {
    let mut map = Map::new();
    for (key, value) in event.fields {
        map.insert(key.into(), value);
    }
    map
}

fn parse_event_with<P>(parser: &P, line: &str) -> Map
where
    P: EventParser,
{
    parser
        .parse(line)
        .map(event_to_map)
        .unwrap_or_else(|_| Map::new())
}

fn split_semicolon_params(section: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = section.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            '\\' if in_quotes => {
                current.push(ch);
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ';' if !in_quotes => {
                if !current.trim().is_empty() {
                    parts.push(current.trim().to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

fn unescape_quoted_value(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                result.push(next);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn is_valid_http_token(token: &str) -> bool {
    !token.is_empty()
        && token.chars().all(|ch| {
            matches!(
                ch,
                'A'..='Z'
                    | 'a'..='z'
                    | '0'..='9'
                    | '!' | '#' | '$' | '%' | '&' | '\'' | '*'
                    | '+' | '-' | '.' | '^' | '_' | '`' | '|' | '~'
            )
        })
}

fn percent_decode_to_vec(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return None;
                }
                let hi = bytes[i + 1];
                let lo = bytes[i + 2];
                let value = (hex_value(hi)? << 4) | hex_value(lo)?;
                result.push(value);
                i += 3;
            }
            b => {
                result.push(b);
                i += 1;
            }
        }

        if result.len() > MAX_PARSE_LEN {
            return None;
        }
    }
    Some(result)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_url_impl(input: &str) -> Map {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_PARSE_LEN {
        return Map::new();
    }

    let (url_str, has_scheme) = if trimmed.contains("://") {
        (trimmed.to_string(), true)
    } else if trimmed.starts_with("//") {
        (format!("http:{}", trimmed), false)
    } else {
        return Map::new();
    };

    let parsed = match Url::parse(&url_str) {
        Ok(url) => url,
        Err(_) => return Map::new(),
    };

    if parsed.host().is_none() {
        return Map::new();
    }

    let mut result = Map::new();

    if has_scheme {
        result.insert("scheme".into(), Dynamic::from(parsed.scheme().to_string()));
    }

    if !parsed.username().is_empty() {
        result.insert("user".into(), Dynamic::from(parsed.username().to_string()));
    }

    if let Some(password) = parsed.password() {
        result.insert("pass".into(), Dynamic::from(password.to_string()));
    }

    if let Some(host) = parsed.host_str() {
        result.insert("host".into(), Dynamic::from(host.to_string()));
    }

    if let Some(port) = parsed.port() {
        result.insert("port".into(), Dynamic::from(port.to_string()));
    }

    let path = parsed.path().to_string();
    if !path.is_empty() {
        result.insert("path".into(), Dynamic::from(path));
    }

    if let Some(query) = parsed.query() {
        result.insert("query".into(), Dynamic::from(query.to_string()));

        let mut qmap = Map::new();
        for (key, value) in parsed.query_pairs() {
            let key_owned = key.into_owned();
            if !qmap.contains_key(key_owned.as_str()) {
                qmap.insert(key_owned.into(), Dynamic::from(value.into_owned()));
            }
        }
        if !qmap.is_empty() {
            result.insert("query_map".into(), Dynamic::from(qmap));
        }
    }

    if let Some(fragment) = parsed.fragment() {
        result.insert("fragment".into(), Dynamic::from(fragment.to_string()));
    }

    result
}

fn parse_path_impl(input: &str) -> Map {
    let mut map = Map::new();
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_PARSE_LEN {
        return map;
    }

    let path = Path::new(trimmed);

    map.insert("input".into(), Dynamic::from(trimmed.to_string()));
    map.insert("is_absolute".into(), Dynamic::from(path.is_absolute()));
    map.insert("is_relative".into(), Dynamic::from(path.is_relative()));
    map.insert("has_root".into(), Dynamic::from(path.has_root()));

    if let Some(parent) = path.parent() {
        let parent_str = parent.to_string_lossy().to_string();
        if !parent_str.is_empty() {
            map.insert("parent".into(), Dynamic::from(parent_str));
        }
    }

    if let Some(file_name) = path.file_name() {
        map.insert(
            "file_name".into(),
            Dynamic::from(file_name.to_string_lossy().to_string()),
        );
    }

    if let Some(stem) = path.file_stem() {
        map.insert(
            "stem".into(),
            Dynamic::from(stem.to_string_lossy().to_string()),
        );
    }

    if let Some(ext) = path.extension() {
        map.insert(
            "extension".into(),
            Dynamic::from(ext.to_string_lossy().to_string()),
        );
    }

    let mut components_array = Array::new();
    let mut prefix_value: Option<String> = None;
    let mut root_value: Option<String> = None;

    for component in path.components() {
        use std::path::Component;

        let display = component.as_os_str().to_string_lossy().to_string();
        match component {
            Component::Prefix(prefix) => {
                let value = prefix.as_os_str().to_string_lossy().to_string();
                if prefix_value.is_none() {
                    prefix_value = Some(value.clone());
                }
                components_array.push(Dynamic::from(value));
            }
            Component::RootDir => {
                if root_value.is_none() {
                    root_value = Some(display.clone());
                }
                components_array.push(Dynamic::from(display));
            }
            Component::CurDir | Component::ParentDir | Component::Normal(_) => {
                components_array.push(Dynamic::from(display));
            }
        }
    }

    if !components_array.is_empty() {
        map.insert("components".into(), Dynamic::from(components_array));
    }

    if let Some(prefix) = prefix_value {
        map.insert("prefix".into(), Dynamic::from(prefix));
    }

    if let Some(root) = root_value {
        map.insert("root".into(), Dynamic::from(root));
    }

    map
}

fn parse_email_impl(input: &str) -> Map {
    fn is_allowed_unquoted_local_char(ch: char) -> bool {
        matches!(
            ch,
            'A'..='Z'
                | 'a'..='z'
                | '0'..='9'
                | '!' | '#' | '$' | '%' | '&' | '\'' | '*'
                | '+' | '-' | '/' | '=' | '?' | '^' | '_' | '`' | '{' | '|' | '}'
                | '~'
        )
    }

    fn parse_quoted_local(local: &str) -> Option<String> {
        if !local.starts_with('"') || !local.ends_with('"') || local.len() < 2 {
            return None;
        }

        let mut result = String::with_capacity(local.len() - 2);
        let mut chars = local[1..local.len() - 1].chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(escaped) = chars.next() {
                    result.push(escaped);
                } else {
                    return None;
                }
            } else if ch == '"' {
                return None;
            } else {
                result.push(ch);
            }
        }
        Some(result)
    }

    fn parse_unquoted_local(local: &str) -> Option<String> {
        if local.is_empty() || local.starts_with('.') || local.ends_with('.') {
            return None;
        }

        let mut prev_dot = false;
        for ch in local.chars() {
            if ch == '.' {
                if prev_dot {
                    return None;
                }
                prev_dot = true;
                continue;
            }

            if ch.is_ascii() && is_allowed_unquoted_local_char(ch) {
                prev_dot = false;
                continue;
            }

            return None;
        }

        if prev_dot {
            return None;
        }

        Some(local.to_string())
    }

    fn is_valid_domain(domain: &str) -> bool {
        if domain.is_empty()
            || domain.len() > MAX_PARSE_LEN
            || domain.starts_with('.')
            || domain.ends_with('.')
        {
            return false;
        }

        for label in domain.split('.') {
            if label.is_empty()
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
            {
                return false;
            }
        }
        true
    }

    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_PARSE_LEN {
        return Map::new();
    }

    let mut splitter = trimmed.split('@');
    let local_raw = splitter.next().unwrap_or("");
    let domain = match splitter.next() {
        Some(value) => value,
        None => return Map::new(),
    };

    if splitter.next().is_some() {
        return Map::new();
    }

    if domain.is_empty() || local_raw.is_empty() {
        return Map::new();
    }

    if !is_valid_domain(domain) {
        return Map::new();
    }

    let local = if local_raw.starts_with('"') {
        parse_quoted_local(local_raw)
    } else {
        if local_raw.contains(char::is_whitespace) {
            return Map::new();
        }
        parse_unquoted_local(local_raw)
    };

    let local = match local {
        Some(value) => value,
        None => return Map::new(),
    };

    let mut map = Map::new();
    map.insert("local".into(), Dynamic::from(local));
    map.insert("domain".into(), Dynamic::from(domain.to_string()));
    map
}

fn extract_version_token(ua: &str, ua_lower: &str, token: &str) -> Option<String> {
    let token_lower = token.to_lowercase();
    let start = ua_lower.find(&token_lower)? + token_lower.len();
    let mut end = ua.len();
    for (idx, ch) in ua[start..].char_indices() {
        if !matches!(ch, '0'..='9' | 'A'..='Z' | 'a'..='z' | '.' | '_' | '-') {
            end = start + idx;
            break;
        }
    }
    if end == start {
        None
    } else {
        Some(ua[start..end].to_string())
    }
}

fn capture_version_after(
    ua: &str,
    ua_lower: &str,
    token: &str,
    replace_underscores: bool,
) -> Option<String> {
    let token_lower = token.to_lowercase();
    let start = ua_lower.find(&token_lower)? + token_lower.len();
    let mut end = ua.len();
    for (idx, ch) in ua[start..].char_indices() {
        if !matches!(ch, '0'..='9' | 'A'..='Z' | 'a'..='z' | '.' | '_' ) {
            end = start + idx;
            break;
        }
    }
    if end == start {
        None
    } else {
        let mut value = ua[start..end].to_string();
        if replace_underscores {
            value = value.replace('_', ".");
        }
        Some(value)
    }
}

fn parse_user_agent_impl(input: &str) -> Map {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_PARSE_LEN {
        return Map::new();
    }

    let ua_lower = trimmed.to_lowercase();
    let mut result = Map::new();

    let mut agent_family: Option<String> = None;
    let mut agent_version: Option<String> = None;

    let candidate_agents: &[(&str, &str)] = &[
        ("curl", "curl/"),
        ("wget", "wget/"),
        ("okhttp", "okhttp/"),
        ("Go-http-client", "go-http-client/"),
        ("Edge", "edge/"),
        ("Edge", "edg/"),
        ("Firefox", "firefox/"),
        ("Chrome", "chrome/"),
        ("Safari", "version/"),
    ];

    for (family, token) in candidate_agents {
        if let Some(version) = extract_version_token(trimmed, &ua_lower, token) {
            if *family == "Safari"
                && (!ua_lower.contains("safari/") || ua_lower.contains("chrome/")) {
                    continue;
                }
            agent_family = Some(family.to_string());
            agent_version = Some(version);
            break;
        }
    }

    if agent_family.is_none() {
        if ua_lower.contains("mozilla/") {
            agent_family = Some("Mozilla".to_string());
        } else if ua_lower.contains("okhttp") {
            agent_family = Some("okhttp".to_string());
        }
    }

    if let Some(family) = agent_family.clone() {
        result.insert("agent_family".into(), Dynamic::from(family));
    }
    if let Some(version) = agent_version.clone() {
        if !version.is_empty() {
            result.insert("agent_version".into(), Dynamic::from(version));
        }
    }

    let mut os_family: Option<String> = None;
    let mut os_version: Option<String> = None;

    if ua_lower.contains("windows nt ") {
        if let Some(version) = capture_version_after(trimmed, &ua_lower, "windows nt ", false) {
            os_family = Some("Windows".to_string());
            os_version = Some(version);
        }
    } else if ua_lower.contains("android ") {
        if let Some(version) = capture_version_after(trimmed, &ua_lower, "android ", false) {
            os_family = Some("Android".to_string());
            os_version = Some(version);
        } else {
            os_family = Some("Android".to_string());
        }
    } else if ua_lower.contains("cpu iphone os ") {
        if let Some(version) = capture_version_after(trimmed, &ua_lower, "cpu iphone os ", true) {
            os_family = Some("iOS".to_string());
            os_version = Some(version);
        }
    } else if ua_lower.contains("iphone os ") {
        if let Some(version) = capture_version_after(trimmed, &ua_lower, "iphone os ", true) {
            os_family = Some("iOS".to_string());
            os_version = Some(version);
        }
    } else if ua_lower.contains("cpu os ") && ua_lower.contains("ipad") {
        if let Some(version) = capture_version_after(trimmed, &ua_lower, "cpu os ", true) {
            os_family = Some("iOS".to_string());
            os_version = Some(version);
        }
    } else if ua_lower.contains("mac os x ") {
        if let Some(version) = capture_version_after(trimmed, &ua_lower, "mac os x ", true) {
            os_family = Some("macOS".to_string());
            os_version = Some(version);
        } else {
            os_family = Some("macOS".to_string());
        }
    } else if ua_lower.contains("linux") {
        os_family = Some("Linux".to_string());
    }

    if let Some(family) = os_family.clone() {
        result.insert("os_family".into(), Dynamic::from(family));
    }
    if let Some(version) = os_version.clone() {
        if !version.is_empty() {
            result.insert("os_version".into(), Dynamic::from(version));
        }
    }

    let mut device: Option<String> = None;
    let lower = ua_lower.as_str();
    if lower.contains("bot")
        || lower.contains("spider")
        || lower.contains("crawler")
        || lower.contains("googlebot")
        || lower.contains("bingbot")
        || matches!(
            agent_family.as_deref(),
            Some("curl" | "wget" | "okhttp" | "Go-http-client")
        )
    {
        device = Some("Bot".to_string());
    } else if lower.contains("ipad") || lower.contains("tablet") {
        device = Some("Tablet".to_string());
    } else if lower.contains("mobile") || lower.contains("iphone") {
        device = Some("Mobile".to_string());
    } else if matches!(os_family.as_deref(), Some("Windows" | "macOS" | "Linux")) {
        device = Some("Desktop".to_string());
    }

    if let Some(device_value) = device {
        result.insert("device".into(), Dynamic::from(device_value));
    }

    if result.is_empty() {
        Map::new()
    } else {
        result
    }
}

fn parse_media_type_impl(input: &str) -> Map {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_PARSE_LEN || trimmed.contains(',') {
        return Map::new();
    }

    let mut iter = trimmed.splitn(2, ';');
    let type_subtype = iter.next().unwrap_or("").trim();
    if type_subtype.is_empty() {
        return Map::new();
    }

    let mut type_parts = type_subtype.splitn(2, '/');
    let r#type = type_parts.next().unwrap_or("").trim();
    let subtype = type_parts.next().unwrap_or("").trim();

    if !is_valid_http_token(r#type) || !is_valid_http_token(subtype) {
        return Map::new();
    }

    let type_lower = r#type.to_lowercase();
    let subtype_lower = subtype.to_lowercase();

    let mut result = Map::new();
    result.insert("type".into(), Dynamic::from(type_lower.clone()));
    result.insert("subtype".into(), Dynamic::from(subtype_lower.clone()));

    if let Some(dot_pos) = subtype_lower.find('.') {
        if dot_pos > 0 {
            let tree = &subtype_lower[..dot_pos];
            if !tree.is_empty() {
                result.insert("tree".into(), Dynamic::from(tree.to_string()));
            }
        }
    }

    if let Some(plus_pos) = subtype_lower.rfind('+') {
        if plus_pos + 1 < subtype_lower.len() {
            let suffix = &subtype_lower[plus_pos + 1..];
            if !suffix.is_empty() && is_valid_http_token(suffix) {
                result.insert("suffix".into(), Dynamic::from(suffix.to_string()));
            }
        }
    }

    let mut params = Map::new();
    if let Some(rest) = iter.next() {
        for param in split_semicolon_params(rest) {
            let mut kv = param.splitn(2, '=');
            let key = kv.next().unwrap_or("").trim();
            let value_raw = kv.next().unwrap_or("").trim();
            if key.is_empty() || !is_valid_http_token(key) {
                continue;
            }
            let key_lower = key.to_lowercase();
            if params.contains_key(key_lower.as_str()) {
                continue;
            }

            let value =
                if value_raw.starts_with('"') && value_raw.ends_with('"') && value_raw.len() >= 2 {
                    unescape_quoted_value(&value_raw[1..value_raw.len() - 1])
                } else {
                    value_raw.to_string()
                };

            params.insert(key_lower.into(), Dynamic::from(value));
        }
    }

    result.insert("params".into(), Dynamic::from(params));
    result
}

fn parse_content_disposition_impl(input: &str) -> Map {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_PARSE_LEN {
        return Map::new();
    }

    let mut iter = trimmed.splitn(2, ';');
    let disposition = iter.next().unwrap_or("").trim();
    if disposition.is_empty() || !is_valid_http_token(disposition) {
        return Map::new();
    }

    let mut params = Map::new();
    let mut filename_regular: Option<String> = None;
    let mut filename_star: Option<String> = None;

    if let Some(rest) = iter.next() {
        for param in split_semicolon_params(rest) {
            let mut kv = param.splitn(2, '=');
            let key = kv.next().unwrap_or("").trim();
            if key.is_empty() {
                continue;
            }
            let key_lower = key.to_lowercase();
            let raw_value = kv.next().unwrap_or("").trim();

            if !params.contains_key(key_lower.as_str()) {
                let value = if raw_value.starts_with('"')
                    && raw_value.ends_with('"')
                    && raw_value.len() >= 2
                {
                    unescape_quoted_value(&raw_value[1..raw_value.len() - 1])
                } else {
                    raw_value.to_string()
                };
                params.insert(key_lower.clone().into(), Dynamic::from(value.clone()));

                if key_lower == "filename" && filename_regular.is_none() {
                    filename_regular = Some(value);
                }
            }

            if key_lower == "filename*" && filename_star.is_none() {
                let value = if raw_value.starts_with('"')
                    && raw_value.ends_with('"')
                    && raw_value.len() >= 2
                {
                    &raw_value[1..raw_value.len() - 1]
                } else {
                    raw_value
                };

                let apostrophe = '\'';
                let parts: Vec<&str> = value.splitn(3, apostrophe).collect();
                if parts.len() == 3 {
                    if let Some(decoded) = percent_decode_to_vec(parts[2]) {
                        let text = if parts[0].eq_ignore_ascii_case("utf-8") {
                            match String::from_utf8(decoded) {
                                Ok(value) => value,
                                Err(err) => String::from_utf8_lossy(err.as_bytes()).into_owned(),
                            }
                        } else {
                            String::from_utf8_lossy(&decoded).into_owned()
                        };
                        filename_star = Some(text);
                    }
                }
            }
        }
    }

    let mut result = Map::new();
    result.insert(
        "disposition".into(),
        Dynamic::from(disposition.to_lowercase()),
    );
    result.insert("params".into(), Dynamic::from(params));

    if let Some(name) = filename_star.or(filename_regular) {
        if !name.is_empty() {
            result.insert("filename".into(), Dynamic::from(name));
        }
    }

    result
}

fn is_base64url_char(ch: char) -> bool {
    matches!(ch, 'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '=')
}

fn decode_jwt_segment(segment: &str) -> Option<Vec<u8>> {
    if segment.len() > MAX_PARSE_LEN {
        return None;
    }
    match URL_SAFE_NO_PAD.decode(segment.as_bytes()) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            let mut padded = segment.to_string();
            while padded.len() % 4 != 0 {
                padded.push('=');
                if padded.len() > MAX_PARSE_LEN {
                    return None;
                }
            }
            URL_SAFE.decode(padded.as_bytes()).ok()
        }
    }
}

fn jwt_segment_to_map(segment: &str) -> Map {
    if let Some(bytes) = decode_jwt_segment(segment) {
        if bytes.len() <= MAX_PARSE_LEN {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let dynamic = crate::event::json_to_dynamic(&json);
                if let Some(map) = dynamic.try_cast::<Map>() {
                    return map;
                }
            }
        }
    }
    Map::new()
}

fn parse_jwt_impl(input: &str) -> Map {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_PARSE_LEN {
        return Map::new();
    }

    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Map::new();
    }

    if parts[0].is_empty() || parts[1].is_empty() {
        return Map::new();
    }

    if !parts[0].chars().all(is_base64url_char) || !parts[1].chars().all(is_base64url_char) {
        return Map::new();
    }

    let header_map = jwt_segment_to_map(parts[0]);
    let claims_map = jwt_segment_to_map(parts[1]);

    let signature_segment = if parts.len() == 3 { parts[2] } else { "" };

    let mut result = Map::new();
    result.insert("header".into(), Dynamic::from(header_map.clone()));
    result.insert("claims".into(), Dynamic::from(claims_map.clone()));
    result.insert(
        "signature_b64u".into(),
        Dynamic::from(signature_segment.to_string()),
    );

    if let Some(alg) = header_map
        .get("alg")
        .and_then(|v| v.clone().into_string().ok())
    {
        result.insert("alg".into(), Dynamic::from(alg));
    }
    if let Some(kid) = header_map
        .get("kid")
        .and_then(|v| v.clone().into_string().ok())
    {
        result.insert("kid".into(), Dynamic::from(kid));
    }
    if let Some(typ) = header_map
        .get("typ")
        .and_then(|v| v.clone().into_string().ok())
    {
        result.insert("typ".into(), Dynamic::from(typ));
    }

    result
}

fn parse_syslog_impl(line: &str) -> Map {
    parse_event_with(&*SYSLOG_PARSER, line)
}

fn parse_cef_impl(line: &str) -> Map {
    parse_event_with(&*CEF_PARSER, line)
}

fn parse_logfmt_impl(line: &str) -> Map {
    parse_event_with(&*LOGFMT_PARSER, line)
}

fn parse_combined_impl(line: &str) -> Map {
    parse_event_with(&*COMBINED_PARSER, line)
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

/// Capture a message in the ordered message system for parallel processing
pub fn capture_message(message: CapturedMessage) {
    CAPTURED_MESSAGES.with(|messages| {
        messages.borrow_mut().push(message);
    });
}

/// Capture a stdout message in the ordered system
pub fn capture_stdout(message: String) {
    capture_message(CapturedMessage::Stdout(message));
}

/// Capture a stderr message in the ordered system  
pub fn capture_stderr(message: String) {
    capture_message(CapturedMessage::Stderr(message));
}

/// Get all captured messages in order and clear the buffer
pub fn take_captured_messages() -> Vec<CapturedMessage> {
    CAPTURED_MESSAGES.with(|messages| std::mem::take(&mut *messages.borrow_mut()))
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
            // Use both old capture system (for compatibility) and new ordered system
            capture_eprint(msg.clone());
            capture_stderr(msg);
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

    // Structured parsing helpers
    engine.register_fn("parse_url", parse_url_impl);
    engine.register_fn("parse_path", parse_path_impl);
    engine.register_fn("parse_email", parse_email_impl);
    engine.register_fn("parse_user_agent", parse_user_agent_impl);
    engine.register_fn("parse_media_type", parse_media_type_impl);
    engine.register_fn("parse_content_disposition", parse_content_disposition_impl);
    engine.register_fn("parse_jwt", parse_jwt_impl);
    engine.register_fn("parse_syslog", parse_syslog_impl);
    engine.register_fn("parse_cef", parse_cef_impl);
    engine.register_fn("parse_logfmt", parse_logfmt_impl);
    engine.register_fn("parse_combined", parse_combined_impl);

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

    // Overloaded variant for method syntax: array.join(separator)
    engine.register_fn("join", |items: rhai::Array, separator: &str| -> String {
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

    // Unflattening functions - reconstruct nested structures from flat keys

    // Default unflatten() - uses underscore separator with smart heuristics
    engine.register_fn("unflatten", |map: rhai::Map| -> rhai::Map {
        unflatten_map(map, "_")
    });

    // unflatten(separator) - specify separator with smart heuristics
    engine.register_fn(
        "unflatten",
        |map: rhai::Map, separator: &str| -> rhai::Map { unflatten_map(map, separator) },
    );
}

/// Unflatten a map by reconstructing nested structures from flat keys
/// Uses smart heuristics to determine when to create arrays vs objects
fn unflatten_map(flat_map: Map, separator: &str) -> Map {
    let mut result = Map::new();

    // First pass: analyze all keys to determine container types
    let mut key_analysis = std::collections::HashMap::new();
    for flat_key in flat_map.keys() {
        let parts: Vec<&str> = flat_key.split(separator).collect();
        analyze_key_path(&parts, &mut key_analysis, separator);
    }

    // Second pass: build the nested structure
    for (flat_key, value) in flat_map {
        let parts: Vec<&str> = flat_key.split(separator).collect();
        if !parts.is_empty() {
            set_nested_value(&mut result, &parts, value, &key_analysis, separator);
        }
    }

    result
}

/// Analyze a key path to determine what type of containers should be created
fn analyze_key_path(
    parts: &[&str],
    analysis: &mut std::collections::HashMap<String, ContainerType>,
    separator: &str,
) {
    let mut current_path = String::new();

    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            current_path.push_str(separator);
        }
        current_path.push_str(part);

        // Look at the next part to determine what container type this should be
        if i + 1 < parts.len() {
            let next_part = parts[i + 1];
            let container_type = if is_array_index(next_part) {
                ContainerType::Array
            } else {
                ContainerType::Object
            };

            // If we've seen this path before, check for conflicts
            match analysis.get(&current_path) {
                Some(existing_type) => {
                    if *existing_type != container_type {
                        // Conflict: array index and non-array key for same parent
                        // Default to object in case of conflict
                        analysis.insert(current_path.clone(), ContainerType::Object);
                    }
                }
                None => {
                    analysis.insert(current_path.clone(), container_type);
                }
            }
        }
    }
}

/// Check if a string represents an array index (pure number)
fn is_array_index(s: &str) -> bool {
    s.parse::<usize>().is_ok()
}

/// Container type for reconstruction
#[derive(Debug, Clone, Copy, PartialEq)]
enum ContainerType {
    Array,
    Object,
}

/// Set a nested value in the result structure
fn set_nested_value(
    container: &mut Map,
    parts: &[&str],
    value: Dynamic,
    analysis: &std::collections::HashMap<String, ContainerType>,
    separator: &str,
) {
    set_nested_value_with_path(container, parts, value, analysis, separator, &[]);
}

/// Set a nested value in the result structure with full path context
fn set_nested_value_with_path(
    container: &mut Map,
    parts: &[&str],
    value: Dynamic,
    analysis: &std::collections::HashMap<String, ContainerType>,
    separator: &str,
    parent_path: &[&str],
) {
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        // Leaf value
        container.insert(parts[0].into(), value);
        return;
    }

    let current_key = parts[0];
    let remaining_parts = &parts[1..];

    // Determine what kind of container we need to create/access
    // Build the full path to the current container
    let mut full_path = parent_path.to_vec();
    full_path.push(current_key);
    let lookup_key = full_path.join(separator);

    let container_type = analysis
        .get(&lookup_key)
        .copied()
        .unwrap_or(ContainerType::Object);

    match container_type {
        ContainerType::Object => {
            // Ensure we have a Map for this key
            let nested_map = container
                .entry(current_key.into())
                .or_insert_with(|| Dynamic::from(Map::new()));

            if let Some(mut map) = nested_map.clone().try_cast::<Map>() {
                let mut new_path = parent_path.to_vec();
                new_path.push(current_key);
                set_nested_value_with_path(
                    &mut map,
                    remaining_parts,
                    value,
                    analysis,
                    separator,
                    &new_path,
                );
                *nested_map = Dynamic::from(map);
            }
        }
        ContainerType::Array => {
            // Ensure we have an Array for this key
            let nested_array = container
                .entry(current_key.into())
                .or_insert_with(|| Dynamic::from(Array::new()));

            if let Some(mut array) = nested_array.clone().try_cast::<Array>() {
                let mut new_path = parent_path.to_vec();
                new_path.push(current_key);
                set_array_value_with_path(
                    &mut array,
                    remaining_parts,
                    value,
                    analysis,
                    separator,
                    &new_path,
                );
                *nested_array = Dynamic::from(array);
            }
        }
    }
}

/// Set a value in an array structure with full path context
fn set_array_value_with_path(
    array: &mut Array,
    parts: &[&str],
    value: Dynamic,
    analysis: &std::collections::HashMap<String, ContainerType>,
    separator: &str,
    parent_path: &[&str],
) {
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        // Leaf value - parts[0] should be an index
        if let Ok(index) = parts[0].parse::<usize>() {
            // Extend array if necessary
            while array.len() <= index {
                array.push(Dynamic::UNIT);
            }
            array[index] = value;
        }
        return;
    }

    let current_index_str = parts[0];
    let remaining_parts = &parts[1..];

    if let Ok(index) = current_index_str.parse::<usize>() {
        // Extend array if necessary
        while array.len() <= index {
            array.push(Dynamic::UNIT);
        }

        // Determine what kind of container the next level needs
        let mut full_path = parent_path.to_vec();
        full_path.push(current_index_str);
        let lookup_key = full_path.join(separator);
        let container_type = analysis
            .get(&lookup_key)
            .copied()
            .unwrap_or(ContainerType::Object);

        match container_type {
            ContainerType::Object => {
                // Ensure we have a Map at this index
                if array[index].is_unit() {
                    array[index] = Dynamic::from(Map::new());
                }

                if let Some(mut map) = array[index].clone().try_cast::<Map>() {
                    let mut new_path = parent_path.to_vec();
                    new_path.push(current_index_str);
                    set_nested_value_with_path(
                        &mut map,
                        remaining_parts,
                        value,
                        analysis,
                        separator,
                        &new_path,
                    );
                    array[index] = Dynamic::from(map);
                }
            }
            ContainerType::Array => {
                // Ensure we have an Array at this index
                if array[index].is_unit() {
                    array[index] = Dynamic::from(Array::new());
                }

                if let Some(mut nested_array) = array[index].clone().try_cast::<Array>() {
                    let mut new_path = parent_path.to_vec();
                    new_path.push(current_index_str);
                    set_array_value_with_path(
                        &mut nested_array,
                        remaining_parts,
                        value,
                        analysis,
                        separator,
                        &new_path,
                    );
                    array[index] = Dynamic::from(nested_array);
                }
            }
        }
    }
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
    fn test_parse_url_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "url",
            "https://user:pass@example.com:8443/path/to/page?foo=bar&baz=qux#frag",
        );

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_url(url)"#)
            .unwrap();

        assert_eq!(
            result.get("scheme").unwrap().clone().into_string().unwrap(),
            "https"
        );
        assert_eq!(
            result.get("user").unwrap().clone().into_string().unwrap(),
            "user"
        );
        assert_eq!(
            result.get("pass").unwrap().clone().into_string().unwrap(),
            "pass"
        );
        assert_eq!(
            result.get("host").unwrap().clone().into_string().unwrap(),
            "example.com"
        );
        assert_eq!(
            result.get("port").unwrap().clone().into_string().unwrap(),
            "8443"
        );
        assert_eq!(
            result.get("path").unwrap().clone().into_string().unwrap(),
            "/path/to/page"
        );
        assert_eq!(
            result.get("query").unwrap().clone().into_string().unwrap(),
            "foo=bar&baz=qux"
        );
        assert_eq!(
            result
                .get("fragment")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "frag"
        );
        let query_map = result
            .get("query_map")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            query_map.get("foo").unwrap().clone().into_string().unwrap(),
            "bar"
        );
        assert_eq!(
            query_map.get("baz").unwrap().clone().into_string().unwrap(),
            "qux"
        );

        scope.push("schemeless", "//example.com/path");
        let schemeless: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_url(schemeless)"#)
            .unwrap();
        assert!(!schemeless.contains_key("scheme"));
        assert_eq!(
            schemeless
                .get("host")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "example.com"
        );

        scope.push("dup", "https://example.com/?id=1&id=2");
        let dup_map: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_url(dup)"#)
            .unwrap();
        let dup_query = dup_map
            .get("query_map")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            dup_query.get("id").unwrap().clone().into_string().unwrap(),
            "1"
        );

        scope.push("invalid", "/just/a/path");
        let invalid: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_url(invalid)"#)
            .unwrap();
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_parse_path_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("path", "logs/app.log");

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_path(path)"#)
            .unwrap();

        assert_eq!(
            result.get("input").unwrap().clone().into_string().unwrap(),
            "logs/app.log"
        );
        assert_eq!(result.get("is_absolute").unwrap().as_bool().unwrap(), false);
        assert_eq!(result.get("is_relative").unwrap().as_bool().unwrap(), true);
        assert_eq!(result.get("has_root").unwrap().as_bool().unwrap(), false);
        assert_eq!(
            result.get("parent").unwrap().clone().into_string().unwrap(),
            "logs"
        );
        assert_eq!(
            result
                .get("file_name")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "app.log"
        );
        assert_eq!(
            result.get("stem").unwrap().clone().into_string().unwrap(),
            "app"
        );
        assert_eq!(
            result
                .get("extension")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "log"
        );

        let components = result
            .get("components")
            .unwrap()
            .clone()
            .into_array()
            .unwrap();
        let component_strings: Vec<String> = components
            .into_iter()
            .map(|item| item.into_string().unwrap())
            .collect();
        assert_eq!(component_strings, vec!["logs", "app.log"]);
    }

    #[test]
    fn test_parse_email_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("email", "user.name+tag@example.co.uk");

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_email(email)"#)
            .unwrap();

        assert_eq!(
            result.get("local").unwrap().clone().into_string().unwrap(),
            "user.name+tag"
        );
        assert_eq!(
            result.get("domain").unwrap().clone().into_string().unwrap(),
            "example.co.uk"
        );
        assert_eq!(result.len(), 2);

        scope.push("quoted", "\"a b\"@xn--exmpl-hra.com");
        let quoted: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_email(quoted)"#)
            .unwrap();
        assert_eq!(
            quoted.get("local").unwrap().clone().into_string().unwrap(),
            "a b"
        );
        assert_eq!(
            quoted.get("domain").unwrap().clone().into_string().unwrap(),
            "xn--exmpl-hra.com"
        );

        scope.push("invalid", "missing-at.example.com");
        let invalid: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_email(invalid)"#)
            .unwrap();
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_parse_user_agent_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "ua",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
        );

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_user_agent(ua)"#)
            .unwrap();

        assert_eq!(
            result
                .get("agent_family")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "Chrome"
        );
        assert_eq!(
            result
                .get("agent_version")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "114.0.0.0"
        );
        assert_eq!(
            result
                .get("os_family")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "macOS"
        );
        assert_eq!(
            result
                .get("os_version")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "10.15.7"
        );
        assert_eq!(
            result.get("device").unwrap().clone().into_string().unwrap(),
            "Desktop"
        );

        scope.push("bot", "curl/8.1.0");
        let bot: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_user_agent(bot)"#)
            .unwrap();
        assert_eq!(
            bot.get("agent_family")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "curl"
        );
        assert_eq!(
            bot.get("device").unwrap().clone().into_string().unwrap(),
            "Bot"
        );
    }

    #[test]
    fn test_parse_media_type_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "mt",
            "Application/vnd.api+JSON; charset=\"utf-8\"; version=1",
        );

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_media_type(mt)"#)
            .unwrap();

        assert_eq!(
            result.get("type").unwrap().clone().into_string().unwrap(),
            "application"
        );
        assert_eq!(
            result
                .get("subtype")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "vnd.api+json"
        );
        assert_eq!(
            result.get("tree").unwrap().clone().into_string().unwrap(),
            "vnd"
        );
        assert_eq!(
            result.get("suffix").unwrap().clone().into_string().unwrap(),
            "json"
        );
        let params = result
            .get("params")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            params
                .get("charset")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "utf-8"
        );
        assert_eq!(
            params
                .get("version")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "1"
        );

        scope.push("invalid_mt", "textplain");
        let invalid: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_media_type(invalid_mt)"#)
            .unwrap();
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_parse_content_disposition_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "cd",
            "attachment; filename=\"resume.pdf\"; filename*=utf-8''r%C3%A9sum%C3%A9.pdf",
        );

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_content_disposition(cd)"#)
            .unwrap();

        assert_eq!(
            result
                .get("disposition")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "attachment"
        );
        assert_eq!(
            result
                .get("filename")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "résumé.pdf"
        );

        let params = result
            .get("params")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert!(params.contains_key("filename"));
        assert!(params.contains_key("filename*"));

        scope.push("bad_cd", "attachment");
        let bad: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_content_disposition(bad_cd)"#)
            .unwrap();
        assert!(!bad.is_empty());

        scope.push("empty_cd", "");
        let empty: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_content_disposition(empty_cd)"#)
            .unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_parse_jwt_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "jwt",
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
             eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWV9.\
             SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        );

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_jwt(jwt)"#)
            .unwrap();

        assert_eq!(
            result.get("alg").unwrap().clone().into_string().unwrap(),
            "HS256"
        );
        assert_eq!(
            result.get("typ").unwrap().clone().into_string().unwrap(),
            "JWT"
        );
        assert_eq!(
            result
                .get("signature_b64u")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        );

        let claims = result
            .get("claims")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            claims.get("sub").unwrap().clone().into_string().unwrap(),
            "1234567890"
        );
        assert!(claims.get("admin").unwrap().clone().as_bool().unwrap());

        scope.push("invalid_jwt", "ab$cd.efg");
        let invalid: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_jwt(invalid_jwt)"#)
            .unwrap();
        assert!(invalid.is_empty());
    }

    #[test]
    fn test_parse_syslog_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "line",
            "<34>1 2023-10-11T22:14:15.003Z server01 app - - - Test message",
        );

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_syslog(line)"#)
            .unwrap();

        assert_eq!(result.get("pri").unwrap().as_int().unwrap(), 34);
        assert_eq!(result.get("facility").unwrap().as_int().unwrap(), 4);
        assert_eq!(result.get("severity").unwrap().as_int().unwrap(), 2);
        assert_eq!(
            result.get("host").unwrap().clone().into_string().unwrap(),
            "server01"
        );
        assert_eq!(
            result.get("prog").unwrap().clone().into_string().unwrap(),
            "app"
        );
        assert_eq!(
            result.get("msg").unwrap().clone().into_string().unwrap(),
            "Test message"
        );
    }

    #[test]
    fn test_parse_cef_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "line",
            "CEF:0|Security|threatmanager|1.0|100|worm successfully stopped|10|src=10.0.0.1 dst=2.1.2.2 spt=1232",
        );

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_cef(line)"#)
            .unwrap();

        assert_eq!(
            result.get("vendor").unwrap().clone().into_string().unwrap(),
            "Security"
        );
        assert_eq!(
            result
                .get("product")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "threatmanager"
        );
        assert_eq!(
            result.get("src").unwrap().clone().into_string().unwrap(),
            "10.0.0.1"
        );
        assert_eq!(result.get("spt").unwrap().as_int().unwrap(), 1232);
    }

    #[test]
    fn test_parse_logfmt_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push("line", "level=info message=hello count=5");

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_logfmt(line)"#)
            .unwrap();

        assert_eq!(
            result.get("level").unwrap().clone().into_string().unwrap(),
            "info"
        );
        assert_eq!(result.get("count").unwrap().as_int().unwrap(), 5);
    }

    #[test]
    fn test_parse_combined_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();
        scope.push(
            "line",
            "192.168.1.1 - user [25/Dec/1995:10:00:00 +0000] \"GET /index.html HTTP/1.0\" 200 1234 \"http://example.com\" \"Mozilla\"",
        );

        let result: rhai::Map = engine
            .eval_with_scope(&mut scope, r#"parse_combined(line)"#)
            .unwrap();

        assert_eq!(
            result.get("ip").unwrap().clone().into_string().unwrap(),
            "192.168.1.1"
        );
        assert_eq!(result.get("status").unwrap().as_int().unwrap(), 200);
        assert_eq!(
            result.get("method").unwrap().clone().into_string().unwrap(),
            "GET"
        );
        assert_eq!(
            result.get("path").unwrap().clone().into_string().unwrap(),
            "/index.html"
        );
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
        assert!(result);

        let result: bool = engine
            .eval_with_scope(&mut scope, r#"mixed.is_digit()"#)
            .unwrap();
        assert!(!result);

        let result: bool = engine
            .eval_with_scope(&mut scope, r#"empty.is_digit()"#)
            .unwrap();
        assert!(!result);

        let result: bool = engine
            .eval_with_scope(&mut scope, r#"letters.is_digit()"#)
            .unwrap();
        assert!(!result);
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

        // Test original syntax: separator.join(array)
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

        // Test new method syntax: array.join(separator)
        let result: String = engine
            .eval_with_scope(&mut scope, r#"["a", "b", "c"].join(",")"#)
            .unwrap();
        assert_eq!(result, "a,b,c");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"["hello", "world"].join(" ")"#)
            .unwrap();
        assert_eq!(result, "hello world");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"["one"].join("-")"#)
            .unwrap();
        assert_eq!(result, "one");

        let result: String = engine
            .eval_with_scope(&mut scope, r#"[].join(",")"#)
            .unwrap();
        assert_eq!(result, "");

        // Test method syntax with mixed types (non-strings filtered out)
        let result: String = engine
            .eval_with_scope(&mut scope, r#"["a", 123, "b"].join(",")"#)
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

    #[test]
    fn test_unflatten_function() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();

        // Test basic object unflattening with default separator (underscore)
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{
                    "user_name": "alice",
                    "user_age": "30",
                    "user_settings_theme": "dark"
                };
                flat.unflatten()
            "##,
            )
            .unwrap();

        // Check nested structure
        let user_map = result
            .get("user")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            user_map.get("name").unwrap().clone().into_string().unwrap(),
            "alice"
        );
        assert_eq!(
            user_map.get("age").unwrap().clone().into_string().unwrap(),
            "30"
        );

        let settings_map = user_map
            .get("settings")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            settings_map
                .get("theme")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "dark"
        );

        // Test array unflattening with numeric indices
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{
                    "items_0_name": "first",
                    "items_1_name": "second",
                    "items_2_name": "third"
                };
                flat.unflatten()
            "##,
            )
            .unwrap();

        let items_array = result
            .get("items")
            .unwrap()
            .clone()
            .try_cast::<rhai::Array>()
            .unwrap();
        assert_eq!(items_array.len(), 3);

        let first_item = items_array[0].clone().try_cast::<rhai::Map>().unwrap();
        assert_eq!(
            first_item
                .get("name")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "first"
        );

        let second_item = items_array[1].clone().try_cast::<rhai::Map>().unwrap();
        assert_eq!(
            second_item
                .get("name")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "second"
        );

        // Test mixed array and object structures
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{
                    "users_0_name": "alice",
                    "users_0_roles_0": "admin",
                    "users_0_roles_1": "user",
                    "users_1_name": "bob",
                    "users_1_roles_0": "user"
                };
                flat.unflatten()
            "##,
            )
            .unwrap();

        let users_array = result
            .get("users")
            .unwrap()
            .clone()
            .try_cast::<rhai::Array>()
            .unwrap();
        assert_eq!(users_array.len(), 2);

        let alice = users_array[0].clone().try_cast::<rhai::Map>().unwrap();
        assert_eq!(
            alice.get("name").unwrap().clone().into_string().unwrap(),
            "alice"
        );

        let alice_roles = alice
            .get("roles")
            .unwrap()
            .clone()
            .try_cast::<rhai::Array>()
            .unwrap();
        assert_eq!(alice_roles.len(), 2);
        assert_eq!(alice_roles[0].clone().into_string().unwrap(), "admin");
        assert_eq!(alice_roles[1].clone().into_string().unwrap(), "user");

        // Test custom separator
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{
                    "user.name": "alice",
                    "user.settings.theme": "dark"
                };
                flat.unflatten(".")
            "##,
            )
            .unwrap();

        let user_map = result
            .get("user")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            user_map.get("name").unwrap().clone().into_string().unwrap(),
            "alice"
        );

        let settings_map = user_map
            .get("settings")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            settings_map
                .get("theme")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "dark"
        );

        // Test edge cases - empty map
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{};
                flat.unflatten()
            "##,
            )
            .unwrap();
        assert!(result.is_empty());

        // Test single level keys (no unflattening needed)
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{
                    "name": "alice",
                    "age": "30"
                };
                flat.unflatten()
            "##,
            )
            .unwrap();
        assert_eq!(
            result.get("name").unwrap().clone().into_string().unwrap(),
            "alice"
        );
        assert_eq!(
            result.get("age").unwrap().clone().into_string().unwrap(),
            "30"
        );
    }

    #[test]
    fn test_unflatten_array_edge_cases() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();

        // Test sparse arrays (gaps in indices)
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{
                    "items_0": "first",
                    "items_2": "third",
                    "items_5": "sixth"
                };
                flat.unflatten()
            "##,
            )
            .unwrap();

        let items_array = result
            .get("items")
            .unwrap()
            .clone()
            .try_cast::<rhai::Array>()
            .unwrap();
        assert_eq!(items_array.len(), 6); // Should extend to highest index + 1
        assert_eq!(items_array[0].clone().into_string().unwrap(), "first");
        assert!(items_array[1].is_unit()); // Gap filled with unit
        assert_eq!(items_array[2].clone().into_string().unwrap(), "third");
        assert!(items_array[3].is_unit()); // Gap
        assert!(items_array[4].is_unit()); // Gap
        assert_eq!(items_array[5].clone().into_string().unwrap(), "sixth");

        // Test array with non-numeric keys mixed in (should default to object)
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{
                    "mixed_0": "zero",
                    "mixed_name": "alice",
                    "mixed_1": "one"
                };
                flat.unflatten()
            "##,
            )
            .unwrap();

        // Should be treated as object due to mixed keys
        let mixed_map = result
            .get("mixed")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            mixed_map.get("0").unwrap().clone().into_string().unwrap(),
            "zero"
        );
        assert_eq!(
            mixed_map
                .get("name")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "alice"
        );
        assert_eq!(
            mixed_map.get("1").unwrap().clone().into_string().unwrap(),
            "one"
        );
    }

    #[test]
    fn test_unflatten_deep_nesting() {
        let mut engine = rhai::Engine::new();
        register_functions(&mut engine);

        let mut scope = Scope::new();

        // Test deeply nested structures
        let result: rhai::Map = engine
            .eval_with_scope(
                &mut scope,
                r##"
                let flat = #{
                    "app_config_database_host": "localhost",
                    "app_config_database_port": "5432",
                    "app_config_cache_redis_url": "redis://localhost",
                    "app_config_cache_ttl": "3600",
                    "app_features_0_name": "auth",
                    "app_features_0_enabled": "true",
                    "app_features_1_name": "logging",
                    "app_features_1_enabled": "false"
                };
                flat.unflatten()
            "##,
            )
            .unwrap();

        // Navigate the nested structure
        let app_map = result
            .get("app")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        let config_map = app_map
            .get("config")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();

        // Check database config
        let db_map = config_map
            .get("database")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            db_map.get("host").unwrap().clone().into_string().unwrap(),
            "localhost"
        );
        assert_eq!(
            db_map.get("port").unwrap().clone().into_string().unwrap(),
            "5432"
        );

        // Check cache config
        let cache_map = config_map
            .get("cache")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            cache_map.get("ttl").unwrap().clone().into_string().unwrap(),
            "3600"
        );
        let redis_map = cache_map
            .get("redis")
            .unwrap()
            .clone()
            .try_cast::<rhai::Map>()
            .unwrap();
        assert_eq!(
            redis_map.get("url").unwrap().clone().into_string().unwrap(),
            "redis://localhost"
        );

        // Check features array
        let features_array = app_map
            .get("features")
            .unwrap()
            .clone()
            .try_cast::<rhai::Array>()
            .unwrap();
        assert_eq!(features_array.len(), 2);

        let auth_feature = features_array[0].clone().try_cast::<rhai::Map>().unwrap();
        assert_eq!(
            auth_feature
                .get("name")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "auth"
        );
        assert_eq!(
            auth_feature
                .get("enabled")
                .unwrap()
                .clone()
                .into_string()
                .unwrap(),
            "true"
        );
    }
}
