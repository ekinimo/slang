pub mod error;
pub mod parser;

// Re-export main types and functions
pub use self::error::ParserError;
pub use self::parser::{parse_program, LanguageParser};
