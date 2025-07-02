use rhai::Engine;

pub mod strings;
pub mod tracking;
pub mod columns;

pub fn register_all_functions(engine: &mut Engine) {
    strings::register_functions(engine);
    tracking::register_functions(engine);
    columns::register_functions(engine);
}