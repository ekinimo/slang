use crate::CompiledFunction;

#[derive(Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Bool(bool),
    Char(char),
    Fun(CompiledFunction),
}

impl core::fmt::Debug for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Value::Unit => write!(f, "() : ()"),
            Value::Int(i) => write!(f, "{i} : Int"),
            Value::Bool(i) => write!(f, "{i} : Bool"),
            Value::Char(i) => write!(f, "{i} : Char"),
            Value::Fun(_compiled_function) => write!(f, "Function"),
        }
    }
}
