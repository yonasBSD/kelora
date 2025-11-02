#![no_main]

use kelora::parsers::JsonlParser;
use kelora::pipeline::EventParser;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        // Parsing errors are fine; we only care about panics or UB.
        let parser = JsonlParser::new();
        let _ = parser.parse(input);
    }
});
