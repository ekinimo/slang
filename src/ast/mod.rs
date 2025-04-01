pub mod indices;
pub mod pool;
pub mod primitives;

pub mod pretty_printer;

// Re-export main types for convenient usage
pub use self::indices::{AstIdx, FunIdx, NameIdx, ParamIdx};
pub use self::pool::AstPool;
pub use self::primitives::PrimitiveFunc;

#[derive(Debug, Clone, Copy)]
pub enum Ast {
    Integer(i64),
    ParamRef(ParamIdx),
    PrimitiveFunc(PrimitiveFunc),
    UserFunc(NameIdx),
    Call {
        func_idx: AstIdx,
        child_count: usize,
    },
    FunctionDef {
        name_idx: NameIdx,
        param_count: usize,
        body_idx: AstIdx,
    },
}
