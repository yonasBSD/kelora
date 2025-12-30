use rhai::Engine;

pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("slice", |s: &str, spec: &str| -> String {
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len() as i32;

        if len == 0 {
            return String::new();
        }

        let parts: Vec<&str> = spec.split(':').collect();

        let step = if parts.len() > 2 && !parts[2].trim().is_empty() {
            parts[2].trim().parse::<i32>().unwrap_or(1)
        } else {
            1
        };

        if step == 0 {
            return String::new();
        }

        let (default_start, default_end) = if step > 0 { (0, len) } else { (len - 1, -1) };

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
}
