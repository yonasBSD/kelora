use rhai::Engine;

pub mod arrays;
pub mod columns;
pub mod datetime;
pub mod docs;
pub mod encoding;
pub mod init;
pub mod maps;
pub mod math;
pub mod process;
pub mod random;
pub mod safety;
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
    process::register_functions(engine);
    random::register_functions(engine);
    datetime::register_functions(engine);
    window::register_functions(engine);
    safety::register_functions(engine);
    init::register_functions(engine);
    encoding::register_functions(engine);
}
