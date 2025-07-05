use rhai::Engine;

pub mod arrays;
pub mod columns;
pub mod datetime;
pub mod maps;
pub mod math;
pub mod strings;
pub mod tracking;
pub mod window;

pub fn register_all_functions(engine: &mut Engine) {
    arrays::register_functions(engine);
    strings::register_functions(engine);
    tracking::register_functions(engine);
    columns::register_functions(engine);
    maps::register_functions(engine);
    math::register_functions(engine);
    datetime::register_functions(engine);
    window::register_functions(engine);
}
