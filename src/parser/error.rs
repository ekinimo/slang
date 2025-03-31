use pest::Parser;
use thiserror::Error;
use pest::error::Error as PestError;
use pest::Span;

use super::parser::Rule;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Syntax error: {0}")]
    PestError(#[from] PestError<Rule>),

    #[error("Undefined parameter '{0}' - all variables must be defined as function parameters")]
    UndefinedParameter(String),

    #[error("Invalid operator '{0}' - only +, -, *, / are supported")]
    InvalidOperator(String),

    #[error("Empty expression found where a value was expected")]
    EmptyExpression,

    #[error("Unexpected syntax element: {0:?}")]
    UnexpectedRule(Rule),

    #[error("Missing function name in function definition")]
    MissingIdentifier,

    #[error("Missing parameter list in function definition - use empty parentheses '()' for functions with no parameters")]
    MissingParameterList,

    #[error("Missing function body - function definition must contain an expression between curly braces")]
    MissingFunctionBody,

    #[error("Invalid binary expression - operators must have left and right operands")]
    InvalidBinaryExpr,

    #[error("Parsing error at line {line}, column {column}: {message}")]
    CustomError {
        line: usize,
        column: usize,
        message: String,
    },
}

pub type Result<T> = std::result::Result<T, ParserError>;

/// Helper function to create rich error messages with source context
pub fn error_with_location(input: &str, span: Span, message: &str) -> ParserError {
    let line_col = span.start_pos().line_col();
    let error_line = input.lines().nth(line_col.0 - 1).unwrap_or("");
    let pointer = " ".repeat(line_col.1 - 1) + "^";

    let detailed_message = format!(
        "{}\nIn line {}:\n{}\n{}\n",
        message, line_col.0, error_line, pointer
    );

    ParserError::CustomError {
        line: line_col.0,
        column: line_col.1,
        message: detailed_message,
    }
}
