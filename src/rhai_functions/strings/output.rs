use rhai::{Dynamic, Engine};

use crate::rhai_functions::capture::{
    capture_eprint, capture_stderr, is_parallel_mode, is_suppress_side_effects,
};

pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("eprint", |message: Dynamic| {
        if is_suppress_side_effects() {
            return;
        }

        let msg = message.to_string();
        if is_parallel_mode() {
            capture_eprint(msg.clone());
            capture_stderr(msg);
        } else {
            eprintln!("{}", msg);
        }
    });
}
