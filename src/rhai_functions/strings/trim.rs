use rhai::Engine;
use std::collections::HashSet;

pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("strip", |text: &str| -> String { text.trim().to_string() });

    engine.register_fn("strip", |text: &str, chars: &str| -> String {
        let chars_to_remove: HashSet<char> = chars.chars().collect();
        text.trim_matches(|c: char| chars_to_remove.contains(&c))
            .to_string()
    });

    engine.register_fn("lstrip", |text: &str| -> String {
        text.trim_start().to_string()
    });

    engine.register_fn("lstrip", |text: &str, chars: &str| -> String {
        let chars_to_remove: HashSet<char> = chars.chars().collect();
        text.trim_start_matches(|c: char| chars_to_remove.contains(&c))
            .to_string()
    });

    engine.register_fn("rstrip", |text: &str| -> String {
        text.trim_end().to_string()
    });

    engine.register_fn("rstrip", |text: &str, chars: &str| -> String {
        let chars_to_remove: HashSet<char> = chars.chars().collect();
        text.trim_end_matches(|c: char| chars_to_remove.contains(&c))
            .to_string()
    });

    engine.register_fn("clip", |text: &str| -> String {
        text.trim_start_matches(|c: char| !c.is_alphanumeric())
            .trim_end_matches(|c: char| !c.is_alphanumeric())
            .to_string()
    });

    engine.register_fn("lclip", |text: &str| -> String {
        text.trim_start_matches(|c: char| !c.is_alphanumeric())
            .to_string()
    });

    engine.register_fn("rclip", |text: &str| -> String {
        text.trim_end_matches(|c: char| !c.is_alphanumeric())
            .to_string()
    });
}
