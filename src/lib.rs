pub mod ast;
pub mod checker;
pub mod compiler;
pub mod interpreter;
pub mod parser;
pub mod value;

// Re-export main types for convenient usage
pub use ast::indices::{AstIdx, FunIdx, NameIdx, ParamIdx};
pub use ast::pool::AstPool;
pub use ast::Ast;
pub use compiler::executor::CompiledFunctions;
pub use compiler::function::CompiledFunction;
pub use interpreter::repl::Interpreter;
pub use parser::error::ParserError;
pub use parser::parser::{parse_program, LanguageParser};
pub use value::Value;
