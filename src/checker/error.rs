use crate::NameIdx;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CheckerError {
    #[error("Function '{0}' not found")]
    UndefinedFunction(String),

    #[error("Function '{name}' expected {expected} arguments but got {actual}")]
    ArgumentCountMismatch {
        name: String,
        expected: usize,
        actual: usize,
    },

    #[error("Cannot call primitive function '{0}' with {1} arguments")]
    InvalidPrimitiveArgCount(String, usize),

    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type Result<T> = std::result::Result<T, CheckerError>;
