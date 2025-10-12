// patterns.rhai — curated regex helpers for Kelora pipelines.
// Usage:
//   kelora --include examples/patterns.rhai ...
//   e.message.has_pattern("url")
//   let urls = e.message.extract_patterns("url");
//   emit_patterns(e.line, "email", "email");

const PATTERN_DEFS = #{
    duration: [
        #{ re: r"\b\d+(?:\.\d+)?(?:ns|us|ms|s|m|h)\b", desc: "Short units like 12ms or 500us" },
        #{ re: r"(?i)\b\d+(?:\.\d+)?\s*(?:microsecond|millisecond|second|minute|hour|day|week|month|year)s?\b", desc: "Verbose durations such as 3 minutes" },
        #{ re: r"\b\d+h\d+m(?:\d+s)?\b", desc: "Compact combos like 1h30m15s" },
        #{ re: r"\b\d+m\d+s\b", desc: "Minute+second combos like 7m30s" },
    ],
    email: [
        #{ re: r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b", desc: "Email addresses" },
    ],
    error_token: [
        #{ re: r"(?i)\b(?:error|err|panic|critical|crit|alert|fatal|emerg|failed|failure|exception|abort|severe)\b", desc: "Error-ish keywords" },
    ],
    fail_signal: [
        #{ re: r"(?i)\b(?:err(?:or)?|fail(?:ure|ed|ing)?|den(?:y|ied)|invalid|time(?:out|d\s*out|-?\s*out)|timout|exception|blocked|expir(?:ed|ing|ation|e)?|reject(?:ed|ing|ion)?|unauthoriz(?:ed|ation|e)?|unauth|forbidden|corrupt(?:ed|ion)?|malform(?:ed|ation)?|disconnect(?:ed|ion)?|unreachable|violat(?:ed|ion|e)?|blacklist(?:ed|ing)?|crash(?:ed|ing)?|abort(?:ed|ing)?|panic|crit(?:ical)?|alert|fatal|emerg(?:ency)?)\b", desc: "Failure / denial keywords" },
    ],
    fqdn: [
        #{ re: r"(?i)\b(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+(?:[a-z]{2,63})\b", desc: "host.example.com" },
    ],
    function_call: [
        #{ re: r"\b[\w.]+\([^()\n]*\)", desc: "function(args) style call" },
    ],
    git_commit: [
        #{ re: r"(?i)\b[0-9a-f]{7,40}\b", desc: "Git commit hashes" },
    ],
    hex_color: [
        #{ re: r"#(?:[0-9a-fA-F]{3}){1,2}\b", desc: "#1A2B3C or #abc" },
    ],
    hex_number: [
        #{ re: r"\b0x[0-9a-fA-F]+\b", desc: "0xdeadbeef" },
    ],
    ipv4: [
        #{ re: r"\b(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}\b", desc: "IPv4 address" },
    ],
    ipv4_port: [
        #{ re: r"\b(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}:(?:6553[0-5]|655[0-2]\d|65[0-4]\d{2}|6[0-4]\d{3}|[1-5]\d{4}|[1-9]\d{0,3}|0)\b", desc: "IPv4 with TCP/UDP port" },
    ],
    ipv6: [
        #{ re: r##"(?xi)\b(?:
            (?:[0-9a-f]{1,4}:){7}[0-9a-f]{1,4}|
            (?:[0-9a-f]{1,4}:){1,7}:|
            :(?:[0-9a-f]{1,4}:){1,7}|
            (?:[0-9a-f]{1,4}:){1,6}:[0-9a-f]{1,4}|
            (?:[0-9a-f]{1,4}:){1,5}(?::[0-9a-f]{1,4}){1,2}|
            (?:[0-9a-f]{1,4}:){1,4}(?::[0-9a-f]{1,4}){1,3}|
            (?:[0-9a-f]{1,4}:){1,3}(?::[0-9a-f]{1,4}){1,4}|
            (?:[0-9a-f]{1,4}:){1,2}(?::[0-9a-f]{1,4}){1,5}|
            [0-9a-f]{1,4}:(?::[0-9a-f]{1,4}){1,6}|
            ::(?:ffff(?::0{1,4}){0,1}:)?(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}|
            (?:[0-9a-f]{1,4}:){1,4}:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}|
            ::
        )\b"##, desc: "IPv6 address (standard and compressed)" },
    ],
    iso_timestamp: [
        #{ re: r"\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?\b", desc: "ISO-8601 timestamp" },
    ],
    jwt: [
        #{ re: r"\beyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*\b", desc: "JWT token" },
    ],
    mac: [
        #{ re: r"\b(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}\b", desc: "MAC aa:bb:cc:dd:ee:ff" },
        #{ re: r"\b(?:[0-9A-Fa-f]{4}\.){2}[0-9A-Fa-f]{4}\b", desc: "Cisco style MAC" },
    ],
    md5: [
        #{ re: r"(?i)\b[a-f0-9]{32}\b", desc: "MD5 hash" },
    ],
    sha1: [
        #{ re: r"(?i)\b[a-f0-9]{40}\b", desc: "SHA-1 hash" },
    ],
    sha256: [
        #{ re: r"(?i)\b[a-f0-9]{64}\b", desc: "SHA-256 hash" },
    ],
    number: [
        #{ re: r"[+-]?(?:\d+\.\d+|\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?", desc: "Integer/float literal" },
    ],
    oauth: [
        #{ re: r"\bya29\.[0-9A-Za-z_-]+\b", desc: "Google OAuth token" },
    ],
    path_unix: [
        #{ re: r"(?:(?:/|~)[^\s\"'<>]+)", desc: "/var/log/app.log" },
    ],
    path_windows: [
        #{ re: r"(?i)\b(?:[A-Z]:\\|\\\\)[^\s\"'<>]+\b", desc: "C:\\Windows\\System32" },
    ],
    win_registry: [
        #{ re: r"\bHKEY_[A-Z_]+(?:\\[A-Za-z0-9_]+)+\b", desc: "Windows registry path" },
    ],
    sql_statement: [
        #{ re: r#""(?:(?:SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|TRUNCATE|GRANT|REVOKE|MERGE)\s+(?:(?:""|[^"])+))""#, desc: "Quoted SQL statement" },
    ],
    url: [
        #{ re: r"(?i)\b(?:[a-z][a-z0-9+.-]*://[^\s\"'<>]+)", desc: "http://example.com/path" },
    ],
    uuid: [
        #{ re: r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b", desc: "UUID/GUID" },
    ],
    version: [
        #{ re: r"\b[vV]?\d+\.\d+(?:\.\d+)?(?:-[A-Za-z0-9]+)?\b", desc: "Semantic version" },
    ],
};

fn _pattern_exists(name) {
    for key in PATTERN_DEFS.keys() {
        if key == name {
            return true;
        }
    }
    false
}

fn _pattern_defs(name) {
    if !_pattern_exists(name) {
        return [];
    }
    PATTERN_DEFS[name]
}

fn _def_group(def) {
    for key in def.keys() {
        if key == "group" {
            return def["group"];
        }
    }
    0
}

fn pattern_names() {
    let names = PATTERN_DEFS.keys();
    names.sort();
    names
}

fn pattern_defs(name) {
    _pattern_defs(name)
}

fn pattern_regexes(name) {
    let defs = _pattern_defs(name);
    let regexes = [];
    for def in defs {
        regexes.push(def.re);
    }
    regexes
}

fn pattern_first(name) {
    let defs = _pattern_defs(name);
    if defs.len() == 0 {
        return "";
    }
    defs[0].re
}

fn has_pattern(text, name) {
    let defs = _pattern_defs(name);
    if defs.len() == 0 {
        return false;
    }

    for def in defs {
        let group = _def_group(def);
        if group > 0 {
            if text.extract_re(def.re, group) != "" {
                return true;
            }
        } else if text.extract_re(def.re) != "" {
            return true;
        }
    }

    false
}

fn extract_pattern(text, name) {
    let defs = _pattern_defs(name);
    if defs.len() == 0 {
        return "";
    }

    for def in defs {
        let group = _def_group(def);
        let found = if group > 0 {
            text.extract_re(def.re, group)
        } else {
            text.extract_re(def.re)
        };

        if found != "" {
            return found;
        }
    }

    ""
}

fn extract_patterns(text, name) {
    let defs = _pattern_defs(name);
    let matches = [];

    for def in defs {
        let group = _def_group(def);
        let items = if group > 0 {
            text.extract_all_re(def.re, group)
        } else {
            text.extract_all_re(def.re)
        };

        if items.len() > 0 {
            for value in items {
                matches.push(value);
            }
        }
    }

    matches
}

fn emit_patterns(text, name, key) {
    emit_patterns_with_base(text, name, key, #{})
}

fn emit_patterns_with_base(text, name, key, base) {
    let values = extract_patterns(text, name);
    if values.len() == 0 {
        return 0;
    }

    let rows = [];
    for value in values {
        let row = base.clone();
        row[key] = value;
        rows.push(row);
    }

    emit_each(rows)
}
