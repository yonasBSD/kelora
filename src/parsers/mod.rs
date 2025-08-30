pub mod auto_detect;
pub mod cef;
pub mod combined;
pub mod csv;
pub mod jsonl;
pub mod line;
pub mod logfmt;
pub mod syslog;

#[allow(unused_imports)] // Used by lib.rs for format auto-detection
pub use auto_detect::detect_format;
pub use cef::CefParser;
pub use combined::CombinedParser;
pub use csv::CsvParser;
pub use jsonl::JsonlParser;
pub use line::LineParser;
pub use logfmt::LogfmtParser;
pub use syslog::SyslogParser;
