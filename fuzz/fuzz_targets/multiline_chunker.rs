#![no_main]

use kelora::config::{InputFormat, MultilineConfig, MultilineStrategy};
use kelora::pipeline::{Chunker, MultilineChunker};
use libfuzzer_sys::fuzz_target;

const MAX_PATTERN_LEN: usize = 128;
const MAX_FORMAT_LEN: usize = 64;
const MAX_LINE_LEN: usize = 1024;
const MAX_LINES: usize = 64;

fuzz_target!(|data: &[u8]| {
    if data.len() < 7 {
        return;
    }

    let strategy_tag = data[0];
    let format_tag = data[1];
    let control = data[2];
    let chrono_len = data[3] as usize;
    let start_len = data[4] as usize;
    let end_len = data[5] as usize;
    let flush_policy = data[6];
    let mut idx = 7;

    let chrono_hint = take_string(data, &mut idx, chrono_len, MAX_FORMAT_LEN);
    let start_pattern = take_string(data, &mut idx, start_len, MAX_PATTERN_LEN);
    let end_pattern = take_string(data, &mut idx, end_len, MAX_PATTERN_LEN);

    let strategy = build_strategy(strategy_tag, chrono_hint, start_pattern, end_pattern);
    let config = MultilineConfig { strategy };
    let input_format = pick_input_format(format_tag);

    let mut chunker = match MultilineChunker::new(config, input_format) {
        Ok(c) => c,
        Err(_) => return,
    };

    let remaining = &data[idx..];
    let mut lines = build_lines(remaining, control);
    if lines.is_empty() {
        lines.push(String::new());
    }

    for (i, line) in lines.into_iter().enumerate() {
        let _ = chunker.feed_line(line);

        if flush_policy & 0x1 == 0x1 && i % 3 == 0 {
            let _ = chunker.flush();
        }

        if flush_policy & 0x2 == 0x2 {
            let _ = chunker.has_pending();
        }
    }

    let _ = chunker.flush();
});

fn build_strategy(
    tag: u8,
    chrono_hint: Option<String>,
    start_pattern: Option<String>,
    end_pattern: Option<String>,
) -> MultilineStrategy {
    match tag % 4 {
        0 => MultilineStrategy::All,
        1 => MultilineStrategy::Indent,
        2 => {
            let chrono_format = chrono_hint.filter(|s| !s.trim().is_empty());
            MultilineStrategy::Timestamp { chrono_format }
        }
        _ => {
            let start_input = start_pattern.unwrap_or_default();
            let start = if start_input.trim().is_empty() {
                r"^.+$".to_string()
            } else {
                start_input
            };

            let end = end_pattern.filter(|s| !s.trim().is_empty());

            MultilineStrategy::Regex { start, end }
        }
    }
}

fn pick_input_format(tag: u8) -> InputFormat {
    match tag % 5 {
        0 => InputFormat::Raw,
        1 => InputFormat::Json,
        2 => InputFormat::Line,
        3 => InputFormat::Syslog,
        _ => InputFormat::Logfmt,
    }
}

fn build_lines(bytes: &[u8], control: u8) -> Vec<String> {
    if bytes.is_empty() {
        return Vec::new();
    }

    let text = if control & 0x4 == 0x4 {
        // Force ASCII subset to exercise indentation detection
        bytes.iter().map(|b| (b % 95 + 32) as char).collect::<String>()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };

    let mut lines = Vec::new();

    for chunk in text.split_inclusive('\n') {
        if lines.len() == MAX_LINES {
            break;
        }
        lines.push(truncate(chunk, MAX_LINE_LEN));
    }

    if lines.is_empty() && !text.is_empty() {
        lines.push(truncate(&text, MAX_LINE_LEN));
    }

    // Optionally add a synthesized timestamp-heavy line so the detector gets hints
    if control & 0x8 == 0x8 {
        lines.push("2025-01-02 03:04:05 host app: synthetic\n".to_string());
    }

    lines
}

fn take_string(data: &[u8], idx: &mut usize, requested: usize, max_len: usize) -> Option<String> {
    if *idx >= data.len() {
        return None;
    }

    let remaining = data.len() - *idx;
    if remaining == 0 {
        return None;
    }

    let actual = requested.min(remaining).min(max_len);
    if actual == 0 {
        return None;
    }

    let slice = &data[*idx..*idx + actual];
    *idx += actual;

    Some(String::from_utf8_lossy(slice).into_owned())
}

fn truncate(input: &str, max_len: usize) -> String {
    input.chars().take(max_len).collect()
}
